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
use notify::{Config, Event, EventKind, EventKindMask, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;
use tokio::sync::watch;

use super::{PublishedSite, next_generation};
use crate::{
    BuildOptions, build_at_with_publication_in_runtime, project::ProjectPaths,
    renderer::RendererRuntime,
};

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
            "source watching may be incomplete; correct the underlying problem and restart `heine serve`"
        )
    )]
    Observe {
        path: PathBuf,
        #[source]
        source: notify::Error,
    },
    #[error("could not restore source watching below {path}")]
    #[diagnostic(
        code(heine::server::watch),
        help("restart `heine serve` to restore source watching")
    )]
    RestoreWatch {
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
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            // The receiver owns reporting and coalescing. A disconnected server
            // simply ends the watcher thread on its next receive.
            let _ = events.send(event);
        },
        Config::default().with_event_kinds(EventKindMask::CORE),
    )
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
        build: BuildOptions,
        kit_paths: Option<crate::kit::Paths>,
        renderer_runtime: RendererRuntime,
    ) -> Result<SourceWatcher, Error> {
        let thread = thread::Builder::new()
            .name("heine-source-watcher".into())
            .spawn(move || {
                watch_loop(
                    self.root,
                    self.watcher,
                    self.receiver,
                    published,
                    build,
                    kit_paths,
                    renderer_runtime,
                )
            })
            .map_err(|source| Error::Thread { source })?;
        Ok(SourceWatcher { _thread: thread })
    }
}

fn watch_loop(
    root: PathBuf,
    mut watcher: impl Watcher,
    receiver: Receiver<notify::Result<Event>>,
    published: watch::Sender<PublishedSite>,
    build: BuildOptions,
    mut kit_paths: Option<crate::kit::Paths>,
    renderer_runtime: RendererRuntime,
) {
    let mut generation = published.borrow().generation;
    while let Ok(event) = receiver.recv() {
        let mut changed_paths = match event {
            Ok(event) => changed_source_paths(&root, &event, kit_paths.as_ref()),
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
                Ok(event) => {
                    changed_paths.extend(changed_source_paths(&root, &event, kit_paths.as_ref()))
                }
                Err(error) => report_observation_error(&root, error),
            }
        }
        let rescan = changed_paths.rescan;
        let changed_paths = changed_paths.finalize(&root);
        if changed_paths.is_empty() && !rescan {
            continue;
        }
        if rescan && let Err(error) = restore_watch(&mut watcher, &root) {
            super::write_message(
                std::io::stderr().lock(),
                format_args!("{:?}", Report::new(error)),
            );
        }
        match build_at_with_publication_in_runtime(&root, build, Some(&renderer_runtime)) {
            Ok((build, _)) => {
                record_publication(&published, &mut generation, &mut kit_paths, build);
                super::write_message(
                    std::io::stdout().lock(),
                    format_args!(
                        "{}",
                        rebuild_message(Timestamp::now(), &changed_paths, rescan)
                    ),
                );
            }
            Err(failure) => {
                let report =
                    record_build_failure(failure, &published, &mut generation, &mut kit_paths);
                super::write_message(std::io::stderr().lock(), format_args!("{report:?}"));
            }
        }
    }
}

fn restore_watch(watcher: &mut impl Watcher, root: &Path) -> Result<(), Error> {
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|source| Error::RestoreWatch {
            path: root.to_path_buf(),
            source,
        })
}

fn record_build_failure(
    failure: crate::BuildFailure,
    published: &watch::Sender<PublishedSite>,
    generation: &mut u64,
    kit_paths: &mut Option<crate::kit::Paths>,
) -> miette::Report {
    if let Some(build) = failure.published {
        record_publication(published, generation, kit_paths, *build);
    }
    failure.report
}

fn record_publication(
    published: &watch::Sender<PublishedSite>,
    generation: &mut u64,
    kit_paths: &mut Option<crate::kit::Paths>,
    build: crate::BuildPublication,
) {
    *generation = next_generation(*generation);
    kit_paths.clone_from(&build.kit_paths);
    let _ = published.send(PublishedSite::from_build(build, *generation));
}

fn changed_source_paths(
    root: &Path,
    event: &Event,
    selected_kit: Option<&crate::kit::Paths>,
) -> SourceChanges {
    if event.need_rescan() {
        return SourceChanges {
            rescan: true,
            ..SourceChanges::default()
        };
    }
    let Some(change) = SourceChange::from_event_kind(&event.kind) else {
        return SourceChanges::default();
    };
    let paths = ProjectPaths::new(root);
    event
        .paths
        .iter()
        .filter(|path| {
            paths.is_build_input(path) || selected_kit.is_some_and(|kit| kit.is_runtime_input(path))
        })
        .map(|path| path.strip_prefix(root).unwrap_or(path).to_path_buf())
        .fold(SourceChanges::default(), |mut changes, path| {
            changes.record(path, change);
            changes
        })
}

fn report_observation_error(root: &Path, source: notify::Error) {
    super::write_message(
        std::io::stderr().lock(),
        format_args!(
            "{:?}",
            Report::new(Error::Observe {
                path: root.into(),
                source,
            })
        ),
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

fn rebuild_message(
    time: Timestamp,
    changes: &BTreeMap<PathBuf, SourceChange>,
    rescan: bool,
) -> String {
    if rescan {
        return format!(
            "[{}] Rebuilt site (filesystem changes may have been missed)",
            timestamp(time)
        );
    }
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
    rescan: bool,
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
        self.rescan |= other.rescan;
        for (path, events) in other.events {
            let current = self.events.entry(path).or_default();
            current.added |= events.added;
            current.changed |= events.changed;
            current.deleted |= events.deleted;
        }
    }

    fn is_empty(&self) -> bool {
        self.events.is_empty() && !self.rescan
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
    fn installed_output_with_cleanup_failure_reaches_server_state() {
        let root = tempfile::tempdir().expect("site root");
        fs::create_dir_all(root.path().join("content/en")).expect("content");
        fs::create_dir(root.path().join("templates")).expect("templates");
        let config = root.path().join("heine.toml");
        fs::write(&config, "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n").expect("config");
        fs::write(
            root.path().join("content/en/index.page"),
            "template='page.tera'\n",
        )
        .expect("page");
        let template = root.path().join("templates/page.tera");
        fs::write(&template, "previous").expect("initial template");
        let build = BuildOptions::new(crate::BuildMode::Full);
        let (initial, runtime) =
            crate::build_at_with_renderer_runtime(root.path(), build).expect("initial build");
        let (published, mut receiver) = watch::channel(PublishedSite::from_build(initial, 7));
        let mut generation = 7;
        let mut kit_paths = None;

        fs::write(&config, "[site]\ntime_zone='Europe/Berlin'\nbase_path='/new/'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n").expect("updated config");
        fs::write(&template, "new").expect("updated template");
        crate::output::fail_next_backup_cleanup();
        let failure = match build_at_with_publication_in_runtime(root.path(), build, Some(&runtime))
        {
            Ok(_) => panic!("backup cleanup must fail"),
            Err(failure) => failure,
        };
        let report = record_build_failure(failure, &published, &mut generation, &mut kit_paths);
        assert!(
            report.to_string().contains("could not remove"),
            "{report:?}"
        );
        assert!(
            format!("{report:?}").contains("heine-previous"),
            "{report:?}"
        );
        let backups: Vec<_> = fs::read_dir(root.path())
            .expect("site entries")
            .map(|entry| entry.expect("site entry").path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("heine-previous"))
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read_to_string(backups[0].join("index.html")).expect("previous output"),
            "previous"
        );
        assert_eq!(
            fs::read_to_string(root.path().join("public/index.html")).expect("published output"),
            "new"
        );
        assert!(receiver.has_changed().expect("server notified"));
        let current = receiver.borrow_and_update().clone();
        assert_eq!(current.generation, 8);
        assert_eq!(current.base_path.as_str(), "/new/");
        assert_eq!(current.output_root, root.path().join("public"));

        fs::write(
            root.path().join("content/en/index.page"),
            "template='missing.tera'\n",
        )
        .expect("invalid page");
        let failure = match build_at_with_publication_in_runtime(root.path(), build, Some(&runtime))
        {
            Ok(_) => panic!("missing template must fail"),
            Err(failure) => failure,
        };
        let _ = record_build_failure(failure, &published, &mut generation, &mut kit_paths);
        assert_eq!(
            fs::read_to_string(root.path().join("public/index.html")).expect("unchanged output"),
            "new"
        );
        assert!(!receiver.has_changed().expect("server state remains"));
        assert_eq!(generation, 8);
    }

    #[test]
    fn rebuild_error_updates_server_only_when_new_output_was_published() {
        let original = PublishedSite {
            base_path: "/old/".try_into().expect("base path"),
            output_root: PathBuf::from("old-output"),
            generation: 7,
        };
        let (published, receiver) = watch::channel(original);
        let mut generation = 7;
        let old_name = crate::kit::KitName::parse("old".into()).expect("old kit name");
        let new_name = crate::kit::KitName::parse("new".into()).expect("new kit name");
        let mut kit_paths = Some(crate::kit::Paths::below_site_root(
            Path::new("site"),
            &old_name,
        ));
        let build = || crate::BuildPublication {
            base_path: "/new/".try_into().expect("base path"),
            output_root: PathBuf::from("new-output"),
            kit_paths: Some(crate::kit::Paths::below_site_root(
                Path::new("site"),
                &new_name,
            )),
        };
        let failure = crate::BuildFailure::output(
            crate::output::Error::Io {
                path: PathBuf::from("new-output"),
                source: std::io::Error::other("write failed"),
            },
            build(),
        );
        let report = record_build_failure(failure, &published, &mut generation, &mut kit_paths);
        assert!(report.to_string().contains("could not write output"));
        assert_eq!(receiver.borrow().generation, 7);
        assert_eq!(receiver.borrow().base_path.as_str(), "/old/");
        assert_eq!(
            kit_paths.as_ref().expect("old kit").manifest(),
            Path::new("site/kits/old/kit.toml")
        );

        let failure = crate::BuildFailure::output(
            crate::output::Error::PublishedWithBackup {
                output: PathBuf::from("new-output"),
                backup: PathBuf::from("backup"),
                source: std::io::Error::other("cleanup failed"),
            },
            build(),
        );
        let report = record_build_failure(failure, &published, &mut generation, &mut kit_paths);
        assert!(report.to_string().contains("could not remove"));
        let current = receiver.borrow();
        assert_eq!(generation, 8);
        assert_eq!(current.generation, 8);
        assert_eq!(current.base_path.as_str(), "/new/");
        assert_eq!(current.output_root, PathBuf::from("new-output"));
        assert_eq!(
            kit_paths.as_ref().expect("new kit").manifest(),
            Path::new("site/kits/new/kit.toml")
        );
    }

    #[test]
    fn rebuild_coordinator_survives_a_closed_stdout_pipe() {
        struct ChildGuard {
            child: std::process::Child,
            running: bool,
        }
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                if self.running {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                }
            }
        }

        const CHILD_ROOT: &str = "HEINE_CLOSED_STDOUT_TEST_ROOT";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = PathBuf::from(root);
            let build = BuildOptions::new(crate::BuildMode::Full);
            let (initial, runtime) =
                crate::build_at_with_renderer_runtime(&root, build).expect("initial build");
            let (published, mut receiver) = watch::channel(PublishedSite::from_build(initial, 0));
            let (events, incoming) = mpsc::channel();
            let watcher = notify::recommended_watcher(|_| {}).expect("watcher");
            let coordinator_root = root.clone();
            let coordinator = thread::spawn(move || {
                watch_loop(
                    coordinator_root,
                    watcher,
                    incoming,
                    published,
                    build,
                    None,
                    runtime,
                );
            });
            fs::write(root.join("ready"), "ready").expect("signal readiness");
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            while !root.join("continue").exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "parent did not continue"
                );
                thread::sleep(Duration::from_millis(10));
            }
            let template = root.join("templates/page.tera");
            for (generation, text) in [(1, "first"), (2, "second")] {
                fs::write(&template, text).expect("change template");
                events
                    .send(Ok(Event::new(EventKind::Any).add_path(template.clone())))
                    .expect("send change");
                let deadline = std::time::Instant::now() + Duration::from_secs(15);
                while receiver.borrow_and_update().generation < generation {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "rebuild did not complete"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                assert_eq!(
                    fs::read_to_string(root.join("public/index.html")).unwrap(),
                    text
                );
            }
            drop(events);
            coordinator
                .join()
                .expect("coordinator survives closed stdout");
            // The test harness also writes to stdout after a test. Exit before it
            // encounters the deliberately closed pipe.
            std::process::exit(0);
        }

        let root = tempfile::tempdir().expect("site root");
        fs::create_dir_all(root.path().join("content/en")).unwrap();
        fs::create_dir(root.path().join("templates")).unwrap();
        fs::write(root.path().join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n").unwrap();
        fs::write(
            root.path().join("content/en/index.page"),
            "template='page.tera'\n",
        )
        .unwrap();
        fs::write(root.path().join("templates/page.tera"), "initial").unwrap();
        let mut child = ChildGuard {
            child: std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "server::watch::tests::rebuild_coordinator_survives_a_closed_stdout_pipe",
                    "--nocapture",
                ])
                .env(CHILD_ROOT, root.path())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("start child test"),
            running: true,
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !root.path().join("ready").exists() {
            if let Some(status) = child.child.try_wait().unwrap() {
                child.running = false;
                panic!("child exited before readiness: {status}");
            }
            if std::time::Instant::now() >= deadline {
                panic!("child did not become ready");
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(child.child.stdout.take());
        fs::write(root.path().join("continue"), "continue").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(35);
        loop {
            if let Some(status) = child.child.try_wait().unwrap() {
                child.running = false;
                assert!(status.success(), "child failed: {status}");
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("child did not finish");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

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
                .contains("source watching may be incomplete")
        );
        assert!(
            error
                .help()
                .expect("observation error has recovery guidance")
                .to_string()
                .contains("restart `heine serve`")
        );
    }

    #[test]
    fn failed_watch_restoration_names_the_root_and_remedy() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");
        let mut watcher = notify::recommended_watcher(|_| {}).unwrap();
        let error =
            restore_watch(&mut watcher, &missing).expect_err("missing root cannot be watched");
        assert!(error.to_string().contains(&missing.display().to_string()));
        assert!(
            error
                .help()
                .unwrap()
                .to_string()
                .contains("restart `heine serve`")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn restoring_recursive_watch_covers_a_directory_created_while_unwatched() {
        let root = tempfile::tempdir().unwrap();
        let mut pending = prepare(root.path().to_path_buf()).unwrap();
        pending.watcher.unwatch(root.path()).unwrap();
        let new_directory = root.path().join("new-directory");
        fs::create_dir(&new_directory).unwrap();
        restore_watch(&mut pending.watcher, root.path()).unwrap();

        let file = new_directory.join("later-edit.txt");
        fs::write(&file, "later edit").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(Ok(event)) = pending.receiver.recv_timeout(Duration::from_millis(100))
                && event.paths.contains(&file)
                && SourceChange::from_event_kind(&event.kind).is_some()
            {
                return;
            }
        }
        panic!("restored recursive watch did not report the later edit");
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

        assert!(changed_source_paths(root, &read, None).is_empty());
        assert!(
            changed_source_paths(root, &write, None)
                .events
                .contains_key(Path::new("content/index.page"))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_watcher_reports_creates_renames_and_removals() {
        let root = tempfile::tempdir().unwrap();
        let pending = prepare(root.path().to_path_buf()).unwrap();
        let original = root.path().join("original.txt");
        let renamed = root.path().join("renamed.txt");
        fs::write(&original, "content").unwrap();
        fs::rename(&original, &renamed).unwrap();
        fs::remove_file(&renamed).unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut seen = [false; 3];
        while !seen.iter().all(|value| *value) && std::time::Instant::now() < deadline {
            if let Ok(Ok(event)) = pending.receiver.recv_timeout(Duration::from_millis(100)) {
                assert!(!matches!(event.kind, EventKind::Access(_)));
                match event.kind {
                    EventKind::Create(_) => seen[0] = true,
                    EventKind::Modify(notify::event::ModifyKind::Name(_)) => seen[1] = true,
                    EventKind::Remove(_) => seen[2] = true,
                    _ => {}
                }
            }
        }
        assert_eq!(seen, [true; 3], "missing create, rename, or remove event");
    }

    #[test]
    fn event_filter_keeps_changes_and_rescan_notifications() {
        let mask = Config::default()
            .with_event_kinds(EventKindMask::CORE)
            .event_kinds();
        assert!(mask.matches(&EventKind::Create(notify::event::CreateKind::Any)));
        assert!(
            mask.matches(&EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Any
            )))
        );
        assert!(mask.matches(&EventKind::Remove(notify::event::RemoveKind::Any)));
        assert!(mask.matches(&EventKind::Other));
        assert!(!mask.matches(&EventKind::Access(notify::event::AccessKind::Read)));
    }

    #[test]
    fn recognizes_deployment_and_license_text_changes_as_build_inputs() {
        let root = Path::new("/site");
        let write = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join("deploy/.htaccess"))
        .add_path(root.join("LICENSES/MIT.txt"));

        let changes = changed_source_paths(root, &write, None);
        assert!(changes.events.contains_key(Path::new("deploy/.htaccess")));
        assert!(changes.events.contains_key(Path::new("LICENSES/MIT.txt")));
    }

    #[test]
    fn recognizes_renderer_source_files_below_content() {
        let root = Path::new("/site");
        let write = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join("content/en/diagrams/request.puml"));

        let changes = changed_source_paths(root, &write, None);
        assert!(
            changes
                .events
                .contains_key(Path::new("content/en/diagrams/request.puml"))
        );
    }

    #[test]
    fn watches_selected_kit_runtime_inputs_but_not_snapshots_or_other_kits() {
        let root = Path::new("/site");
        let name = crate::kit::KitName::parse("paper".into()).expect("valid kit name");
        let kit = crate::kit::Paths::below_site_root(root, &name);
        let write = Event::new(EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(root.join("kits/paper/kit.toml"))
        .add_path(root.join("kits/paper/templates/base.tera"))
        .add_path(root.join("kits/paper/site/content/en/index.page"))
        .add_path(root.join("kits/paper/README.md"))
        .add_path(root.join("kits/other/templates/base.tera"));

        let changes = changed_source_paths(root, &write, Some(&kit));
        assert!(
            changes
                .events
                .contains_key(Path::new("kits/paper/kit.toml"))
        );
        assert!(
            changes
                .events
                .contains_key(Path::new("kits/paper/templates/base.tera"))
        );
        assert!(
            !changes
                .events
                .contains_key(Path::new("kits/paper/site/content/en/index.page"))
        );
        assert!(
            !changes
                .events
                .contains_key(Path::new("kits/paper/README.md"))
        );
        assert!(
            !changes
                .events
                .contains_key(Path::new("kits/other/templates/base.tera"))
        );
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
            rebuild_message(Timestamp::UNIX_EPOCH, &changes, false),
            "[1970-01-01T00:00:00.000Z] Rebuilt site (changed content/post.md)"
        );
        assert_eq!(
            rebuild_message(Timestamp::UNIX_EPOCH, &changes, true),
            "[1970-01-01T00:00:00.000Z] Rebuilt site (filesystem changes may have been missed)"
        );
    }

    #[test]
    fn rescan_is_retained_in_both_batch_orders() {
        let root = Path::new("/site");
        let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        let change = Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
            .add_path(root.join("content/en/post.md"));
        for (first, second) in [(&rescan, &change), (&change, &rescan)] {
            let mut batch = changed_source_paths(root, first, None);
            batch.extend(changed_source_paths(root, second, None));
            assert!(batch.rescan, "rescan must survive coalescing");
            assert!(batch.events.contains_key(Path::new("content/en/post.md")));
        }
    }

    #[test]
    fn rescan_rebuilds_even_without_paths_and_in_either_batch_order() {
        struct RecordingWatcher(mpsc::Sender<(PathBuf, RecursiveMode)>);

        impl Watcher for RecordingWatcher {
            fn new<F: notify::EventHandler>(_: F, _: Config) -> notify::Result<Self> {
                Err(notify::Error::generic("not used in this test"))
            }

            fn watch(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
                self.0.send((path.to_path_buf(), mode)).unwrap();
                Ok(())
            }

            fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
                Ok(())
            }

            fn kind() -> notify::WatcherKind {
                notify::WatcherKind::NullWatcher
            }
        }

        let root = tempfile::tempdir().expect("site root");
        fs::create_dir_all(root.path().join("content/en")).unwrap();
        fs::create_dir(root.path().join("templates")).unwrap();
        fs::write(root.path().join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n").unwrap();
        fs::write(
            root.path().join("content/en/index.page"),
            "template='page.tera'\n",
        )
        .unwrap();
        let template = root.path().join("templates/page.tera");
        fs::write(&template, "initial").unwrap();
        let build = BuildOptions::new(crate::BuildMode::Full);
        let (initial, runtime) = crate::build_at_with_renderer_runtime(root.path(), build).unwrap();
        let (published, receiver) = watch::channel(PublishedSite::from_build(initial, 0));
        let (events, incoming) = mpsc::channel();
        let (watch_calls, watched) = mpsc::channel();
        let watcher = RecordingWatcher(watch_calls);
        let site_root = root.path().to_path_buf();
        let coordinator = thread::spawn(move || {
            watch_loop(
                site_root, watcher, incoming, published, build, None, runtime,
            )
        });

        for (generation, text) in [(1, "first"), (2, "second"), (3, "third")] {
            fs::write(&template, text).unwrap();
            let rescan = || Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
            let change = || {
                Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(template.clone())
            };
            match generation {
                1 => events.send(Ok(rescan())).unwrap(),
                2 => {
                    events.send(Ok(rescan())).unwrap();
                    events.send(Ok(change())).unwrap();
                }
                _ => {
                    events.send(Ok(change())).unwrap();
                    events.send(Ok(rescan())).unwrap();
                }
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while receiver.borrow().generation < generation {
                assert!(
                    std::time::Instant::now() < deadline,
                    "rescan did not rebuild"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(
                watched.recv_timeout(Duration::from_secs(1)).unwrap(),
                (root.path().to_path_buf(), RecursiveMode::Recursive)
            );
            assert_eq!(
                fs::read_to_string(root.path().join("public/index.html")).unwrap(),
                text
            );
        }
        drop(events);
        coordinator.join().unwrap();
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
