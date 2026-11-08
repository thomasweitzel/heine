//! Staged full-build publication.

use super::{
    Error,
    target::{ensure_directory, invalid_path},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct StagingDirectory {
    path: PathBuf,
    output: PathBuf,
    published: bool,
}
impl StagingDirectory {
    pub(super) fn create(output: &Path) -> Result<Self, Error> {
        let parent = output
            .parent()
            .ok_or_else(|| invalid_path(output, "output path has no parent"))?;
        ensure_directory(parent)?;
        let name = output
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_path(output, "output directory name is not UTF-8"))?;
        for _ in 0..32 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.heine-staging-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        output: output.into(),
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(Error::Io { path, source }),
            }
        }
        Err(Error::Io {
            path: parent.into(),
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not create staging directory",
            ),
        })
    }
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
    pub(super) fn publish(&mut self, replace_existing: bool) -> Result<(), Error> {
        self.publish_with(
            replace_existing,
            |from, to| fs::rename(from, to),
            |path| fs::remove_dir_all(path),
        )
    }

    fn publish_with(
        &mut self,
        replace_existing: bool,
        mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
        mut remove: impl FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), Error> {
        if !replace_existing {
            rename(&self.path, &self.output).map_err(|source| Error::Io {
                path: self.path.clone(),
                source,
            })?;
            self.published = true;
            return Ok(());
        }
        let backup = self.backup_path()?;
        rename(&self.output, &backup).map_err(|source| Error::Io {
            path: self.output.clone(),
            source,
        })?;
        if let Err(source) = rename(&self.path, &self.output) {
            if let Err(restore_source) = rename(&backup, &self.output) {
                return Err(Error::Restore {
                    path: self.output.clone(),
                    backup,
                    publication: source,
                    source: restore_source,
                });
            }
            return Err(Error::Io {
                path: self.path.clone(),
                source,
            });
        }
        self.published = true;
        remove(&backup).map_err(|source| Error::PublishedWithBackup {
            output: self.output.clone(),
            backup,
            source,
        })
    }
    fn backup_path(&self) -> Result<PathBuf, Error> {
        let parent = self
            .output
            .parent()
            .ok_or_else(|| invalid_path(&self.output, "output path has no parent"))?;
        let name = self
            .output
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_path(&self.output, "output directory name is not UTF-8"))?;
        for _ in 0..32 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.heine-previous-{}-{sequence}",
                std::process::id()
            ));
            if !path.exists() {
                return Ok(path);
            }
        }
        Err(Error::Io {
            path: parent.into(),
            source: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not create output backup path",
            ),
        })
    }
}
impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> (tempfile::TempDir, StagingDirectory) {
        let root = tempfile::tempdir().expect("temporary site");
        let output = root.path().join("public");
        fs::create_dir(&output).expect("previous output directory");
        fs::write(output.join("index.html"), "previous").expect("previous site");
        let staging = StagingDirectory::create(&output).expect("staging directory");
        fs::write(staging.path().join("index.html"), "new").expect("new site");
        (root, staging)
    }

    #[test]
    fn failed_replacement_restores_previous_output() {
        let (_root, mut staging) = site();
        let mut calls = 0;
        let result = staging.publish_with(
            true,
            |from, to| {
                calls += 1;
                if calls == 2 {
                    Err(io::Error::other("replacement failed"))
                } else {
                    fs::rename(from, to)
                }
            },
            |path| fs::remove_dir_all(path),
        );
        assert!(matches!(result, Err(Error::Io { .. })));
        assert_eq!(
            fs::read_to_string(staging.output.join("index.html")).expect("restored site"),
            "previous"
        );
        assert_eq!(calls, 3);
    }

    #[test]
    fn failed_restore_reports_backup_and_both_errors() {
        let (_root, mut staging) = site();
        let mut calls = 0;
        let result = staging.publish_with(
            true,
            |from, to| {
                calls += 1;
                match calls {
                    2 => Err(io::Error::other("replacement failed")),
                    3 => Err(io::Error::other("restore failed")),
                    _ => fs::rename(from, to),
                }
            },
            |path| fs::remove_dir_all(path),
        );
        let error = result.expect_err("restore must fail");
        let Error::Restore {
            path,
            backup,
            publication,
            source,
        } = &error
        else {
            panic!("expected restore error");
        };
        assert_eq!(path, &staging.output);
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(backup.join("index.html")).expect("backup"),
            "previous"
        );
        assert_eq!(publication.to_string(), "replacement failed");
        assert_eq!(source.to_string(), "restore failed");
        let report = format!("{:?}", miette::Report::new(error));
        assert!(report.contains("replacement failed"), "{report}");
        assert!(report.contains("restore failed"), "{report}");
        assert!(report.contains("heine-previous"), "{report}");
    }

    #[test]
    fn failed_cleanup_reports_new_output_and_preserves_backup() {
        let (_root, mut staging) = site();
        let result = staging.publish_with(
            true,
            |from, to| fs::rename(from, to),
            |_| Err(io::Error::other("cleanup failed")),
        );
        let Err(Error::PublishedWithBackup { output, backup, .. }) = result else {
            panic!("expected backup cleanup error");
        };
        assert_eq!(
            fs::read_to_string(output.join("index.html")).expect("new output"),
            "new"
        );
        assert_eq!(
            fs::read_to_string(backup.join("index.html")).expect("backup"),
            "previous"
        );
        assert!(staging.published);
    }
}
