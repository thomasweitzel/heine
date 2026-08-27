//! Discovery of opaque deployment files copied to the generated site's root.
//!
//! `deploy/` is intentionally separate from content assets: its files are not
//! addressable from templates, versioned, localized, or interpreted by Heine.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;
use walkdir::WalkDir;

use crate::{
    output::{CopiedFiles, OutputPath},
    source_path,
};

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not inspect deployment directory {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("deployment path {path} is not a directory")]
    NotDirectory { path: PathBuf },
    #[error("deployment path {path} is a symbolic link")]
    Symlink { path: PathBuf },
    #[error("could not walk deployment directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("deployment path {path} is not a regular file or directory")]
    UnsupportedEntry { path: PathBuf },
    #[error("deployment path {path} is outside deployment directory {root}")]
    OutsideRoot { path: PathBuf, root: PathBuf },
    #[error("deployment path {path} must use UTF-8 components and must not contain backslashes")]
    NonUtf8Path { path: PathBuf },
    #[error(
        "deployment file {source_path} would be published to invalid output path {output}: {message}"
    )]
    InvalidOutputPath {
        source_path: PathBuf,
        output: PathBuf,
        message: String,
    },
}

/// A validated, deterministic inventory of deployment files.
#[derive(Debug, Default)]
pub(crate) struct Catalog {
    entries: BTreeMap<OutputPath, PathBuf>,
    paths: Vec<OutputPath>,
}

impl Catalog {
    /// Loads an optional `deploy/` root. An absent root intentionally means
    /// that a project has no deployment files.
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Symlink { path: root.into() });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::NotDirectory { path: root.into() });
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(Error::Inspect {
                    path: root.into(),
                    source,
                });
            }
        }

        let mut entries = BTreeMap::new();
        for entry in WalkDir::new(root).sort_by_file_name() {
            let entry = entry.map_err(|source| Error::Walk {
                path: root.into(),
                source,
            })?;
            if entry.path() == root || entry.file_type().is_dir() {
                continue;
            }
            if entry.file_type().is_symlink() {
                return Err(Error::Symlink {
                    path: entry.path().into(),
                });
            }
            if !entry.file_type().is_file() {
                return Err(Error::UnsupportedEntry {
                    path: entry.path().into(),
                });
            }
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| Error::OutsideRoot {
                    path: entry.path().into(),
                    root: root.into(),
                })?;
            if !source_path::has_safe_components(relative) {
                return Err(Error::NonUtf8Path {
                    path: entry.path().into(),
                });
            }
            let output = OutputPath::try_new(relative.into()).map_err(|message| {
                Error::InvalidOutputPath {
                    source_path: entry.path().into(),
                    output: relative.into(),
                    message,
                }
            })?;
            entries.insert(output, entry.path().into());
        }
        let paths = entries.keys().cloned().collect();
        Ok(Self { entries, paths })
    }

    pub(crate) fn paths(&self) -> &[OutputPath] {
        &self.paths
    }

    pub(crate) fn source_path(&self, output: &OutputPath) -> &Path {
        &self.entries[output]
    }
}

impl CopiedFiles for Catalog {
    fn paths(&self) -> &[OutputPath] {
        self.paths()
    }

    fn source_path(&self, output: &OutputPath) -> &Path {
        self.source_path(output)
    }
}
