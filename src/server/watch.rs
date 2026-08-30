//! Filesystem watching and successful-build publication.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use jiff::Timestamp;
use miette::{Diagnostic, Report};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;
use tokio::sync::watch;

use super::{PublishedSite, next_generation};
use crate::{BuildMode, BuildOptions, build_at_with_publication, project::ProjectPaths};

const DEBOUNCE: Duration = Duration::from_millis(150);
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S.%3fZ";

/// Holds the watcher thread alive for the lifetime of the HTTP server.
pub(crate) struct SourceWatcher {
    _thread: thread::JoinHandle<()>,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not watch site sources below {path}")]
    Watch {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
    #[error("could not start the source-watcher thread")]
    Thread {
        #[source]
        source: std::io::Error,
    },
    #[error("could not observe a source change below {path}")]
    #[diagnostic(
        code(heine::server::watch),
        help(
            "save the file again or run a full build manually to make sure the generated site is current"
        )
    )]
    Observe {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
}

/// A watcher registered before the first build. Its receiver queues source
/// writes until the build coordinator is ready to process them.
pub(crate) struct PendingWatcher {
    root: PathBuf,
    watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
}

pub(crate) fn prepare(root: PathBuf) -> Result<PendingWatcher, Error> {
    let (events, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        // The receiver owns reporting and coalescing. A disconnected server
        // simply ends the watcher thread on its next receive.
        let _ = events.send(event);
    })
    .map_err(|source| Error::Watch {
        path: root.clone(),
        source,
    })?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|source| Error::Watch {
            path: root.clone(),
            source,
        })?;
    Ok(PendingWatcher {
        root,
        watcher,
        receiver,
    })
}

impl PendingWatcher {
    pub(crate) fn start(
        self,
        published: watch::Sender<PublishedSite>,
    ) -> Result<SourceWatcher, Error> {
        let thread = thread::Builder::new()
            .name("heine-source-watcher".into())
            .spawn(move || watch_loop(self.root, self.watcher, self.receiver, published))
            .map_err(|source| Error::Thread { source })?;
        Ok(SourceWatcher { _thread: thread })
    }
}

fn watch_loop(
    root: PathBuf,
    _watcher: impl Watcher,
    receiver: Receiver<notify::Result<Event>>,
    published: watch::Sender<PublishedSite>,
) {
    let mut generation = published.borrow().generation;
    while let Ok(event) = receiver.recv() {
        let mut changed_paths = match event {
            Ok(event) => changed_source_paths(&root, &event),
            Err(error) => {
                report_observation_error(&root, error);
                SourceChanges::default()
            }
        };
        if changed_paths.is_empty() {
            continue;
        }
        while let Ok(event) = receiver.recv_timeout(DEBOUNCE) {
            match event {
                Ok(event) => changed_paths.extend(changed_source_paths(&root, &event)),
                Err(error) => report_observation_error(&root, error),
            }
        }
        let changed_paths = changed_paths.finalize(&root);
        if changed_paths.is_empty() {
            continue;
        }
        match build_at_with_publication(&root, BuildOptions::new(BuildMode::Full)) {
            Ok(build) => {
                generation = next_generation(generation);
                let _ = published.send(PublishedSite::from_build(build, generation));
                println!("{}", rebuild_message(Timestamp::now(), &changed_paths));
            }
            Err(error) => eprintln!("{error:?}"),
        }
    }
}

fn changed_source_paths(root: &Path, event: &Event) -> SourceChanges {
    let Some(change) = SourceChange::from_event_kind(&event.kind) else {
        return SourceChanges::default();
    };
    let paths = ProjectPaths::new(root);
    event
        .paths
        .iter()
        .filter(|path| paths.is_build_input(path))
        .map(|path| path.strip_prefix(root).unwrap_or(path).to_path_buf())
        .fold(SourceChanges::default(), |mut changes, path| {
            changes.record(path, change);
            changes
        })
}

fn report_observation_error(root: &Path, source: notify::Error) {
    eprintln!(
        "{:?}",
        Report::new(Error::Observe {
            path: root.into(),
            source,
        })
    );
}

fn describe_changes(changes: &BTreeMap<PathBuf, SourceChange>) -> String {
    const DISPLAY_LIMIT: usize = 3;

    let mut displayed = changes
        .iter()
        .take(DISPLAY_LIMIT)
        .map(|(path, change)| format!("{change} {}", path.display()))
        .collect::<Vec<_>>();
    if changes.len() > DISPLAY_LIMIT {
        displayed.push(format!("and {} more", changes.len() - DISPLAY_LIMIT));
    }
    displayed.join(", ")
}

fn timestamp(time: Timestamp) -> String {
    time.strftime(TIMESTAMP_FORMAT).to_string()
}

fn rebuild_message(time: Timestamp, changes: &BTreeMap<PathBuf, SourceChange>) -> String {
    format!(
        "[{}] Rebuilt site ({})",
        timestamp(time),
        describe_changes(changes)
    )
}

/// The final source-level meaning of a coalesced filesystem event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceChange {
    Added,
    Changed,
    Deleted,
}

impl SourceChange {
    fn from_event_kind(kind: &EventKind) -> Option<Self> {
        match kind {
            EventKind::Create(_) => Some(Self::Added),
            EventKind::Modify(_) | EventKind::Any => Some(Self::Changed),
            EventKind::Remove(_) => Some(Self::Deleted),
            EventKind::Access(_) | EventKind::Other => None,
        }
    }
}

impl std::fmt::Display for SourceChange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Deleted => "deleted",
        })
    }
}

#[derive(Debug, Default)]
struct SourceChanges {
    events: BTreeMap<PathBuf, ChangeEvents>,
}

#[derive(Debug, Default)]
struct ChangeEvents {
    added: bool,
    changed: bool,
    deleted: bool,
}

impl SourceChanges {
    fn record(&mut self, path: PathBuf, change: SourceChange) {
        let events = self.events.entry(path).or_default();
        match change {
            SourceChange::Added => events.added = true,
            SourceChange::Changed => events.changed = true,
            SourceChange::Deleted => events.deleted = true,
        }
    }

    fn extend(&mut self, other: Self) {
        for (path, events) in other.events {
            let current = self.events.entry(path).or_default();
            current.added |= events.added;
            current.changed |= events.changed;
            current.deleted |= events.deleted;
        }
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    fn finalize(self, root: &Path) -> BTreeMap<PathBuf, SourceChange> {
        self.events
            .into_iter()
            .filter_map(|(path, events)| {
                let exists = root.join(&path).exists();
                let change = match (exists, events.added, events.changed, events.deleted) {
                    (true, true, false, false) => Some(SourceChange::Added),
                    (true, _, _, _) => Some(SourceChange::Changed),
                    // Some backends report a deletion as a generic metadata
                    // modification. The absent final path is authoritative
                    // when it was not also created in this debounce window.
                    (false, false, _, _) => Some(SourceChange::Deleted),
                    // An editor's temporary file can be created and removed
                    // within one debounce window. It leaves no source state to
                    // rebuild, so deliberately omit it.
                    (false, _, _, _) => None,
                };
                change.map(|change| (path, change))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn observation_failures_explain_how_to_recover() {
        let error = Error::Observe {
            path: PathBuf::from("/site"),
            source: notify::Error::generic("watch limit reached"),
        };

        assert!(error.to_string().contains("/site"));
        assert!(
            error
                .help()
                .expect("observation error has recovery guidance")
                .to_string()
                .contains("full build")
        );
    }

    #[test]
    fn ignores_read_events_created_by_the_build() {
        let root = Path::new("/site");
        let read = Event::new(EventKind::Access(notify::event::AccessKind::Read))
            .add_path(root.join("content/index.page"));
        let write = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join("content/index.page"));

        assert!(changed_source_paths(root, &read).is_empty());
        assert!(
            changed_source_paths(root, &write)
                .events
                .contains_key(Path::new("content/index.page"))
        );
    }

    #[test]
    fn recognizes_deployment_and_license_text_changes_as_build_inputs() {
        let root = Path::new("/site");
        let write = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join("deploy/.htaccess"))
        .add_path(root.join("LICENSES/MIT.txt"));

        let changes = changed_source_paths(root, &write);
        assert!(changes.events.contains_key(Path::new("deploy/.htaccess")));
        assert!(changes.events.contains_key(Path::new("LICENSES/MIT.txt")));
    }

    #[test]
    fn reports_a_short_deterministic_change_summary() {
        let changes = BTreeMap::from([
            (PathBuf::from("templates/base.tera"), SourceChange::Changed),
            (PathBuf::from("content/post.page"), SourceChange::Added),
            (PathBuf::from("data/site.toml"), SourceChange::Deleted),
            (PathBuf::from("content/post.md"), SourceChange::Changed),
        ]);

        assert_eq!(
            describe_changes(&changes),
            "changed content/post.md, added content/post.page, deleted data/site.toml, and 1 more"
        );
    }

    #[test]
    fn prints_an_unambiguous_utc_timestamp() {
        assert_eq!(timestamp(Timestamp::UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn rebuild_messages_are_compact_status_lines() {
        let changes = BTreeMap::from([(PathBuf::from("content/post.md"), SourceChange::Changed)]);
        assert_eq!(
            rebuild_message(Timestamp::UNIX_EPOCH, &changes),
            "[1970-01-01T00:00:00.000Z] Rebuilt site (changed content/post.md)"
        );
    }

    #[test]
    fn classifies_the_final_state_and_omits_transient_files() {
        let root = std::env::temp_dir().join(format!(
            "heine-watch-test-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("content")).expect("create fixture");
        fs::write(root.join("content/added.md"), "added").expect("write added file");
        fs::write(root.join("content/changed.md"), "changed").expect("write changed file");
        let mut changes = SourceChanges::default();
        changes.record(PathBuf::from("content/added.md"), SourceChange::Added);
        changes.record(PathBuf::from("content/changed.md"), SourceChange::Changed);
        changes.record(PathBuf::from("content/deleted.md"), SourceChange::Deleted);
        changes.record(
            PathBuf::from("content/deleted-as-modification.md"),
            SourceChange::Changed,
        );
        changes.record(PathBuf::from("content/temporary.md~"), SourceChange::Added);
        changes.record(
            PathBuf::from("content/temporary.md~"),
            SourceChange::Deleted,
        );

        assert_eq!(
            changes.finalize(&root),
            BTreeMap::from([
                (PathBuf::from("content/added.md"), SourceChange::Added),
                (PathBuf::from("content/changed.md"), SourceChange::Changed),
                (PathBuf::from("content/deleted.md"), SourceChange::Deleted),
                (
                    PathBuf::from("content/deleted-as-modification.md"),
                    SourceChange::Deleted,
                ),
            ])
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
