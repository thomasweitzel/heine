//! Output-target ownership and safety checks.

use super::{Error, MARKER, MARKER_CONTENTS};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct OutputTarget {
    site_root: PathBuf,
    relative_root: PathBuf,
    root: PathBuf,
}
impl OutputTarget {
    pub(crate) fn new(site_root: &Path, relative_root: &Path) -> Self {
        Self {
            site_root: site_root.into(),
            relative_root: relative_root.into(),
            root: site_root.join(relative_root),
        }
    }
    pub(super) fn root(&self) -> &Path {
        &self.root
    }
    pub(super) fn ensure_no_symlinked_ancestor(&self) -> Result<(), Error> {
        let mut current = self.site_root.clone();
        let mut components = self.relative_root.components().peekable();
        while let Some(component) = components.next() {
            if components.peek().is_none() {
                break;
            }
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(Error::OutputAncestorSymlink { path: current });
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(source) => {
                    return Err(Error::Io {
                        path: current,
                        source,
                    });
                }
            }
        }
        Ok(())
    }
}

pub(super) struct OutputDirectory<'a> {
    path: &'a Path,
    exists: bool,
    marker: MarkerState,
    empty: bool,
}
impl<'a> OutputDirectory<'a> {
    pub(super) fn inspect(path: &'a Path) -> Result<Self, Error> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    exists: false,
                    marker: MarkerState::Absent,
                    empty: true,
                });
            }
            Err(source) => {
                return Err(Error::Io {
                    path: path.into(),
                    source,
                });
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(Error::OutputSymlink { path: path.into() });
        }
        if !metadata.is_dir() {
            return Err(Error::OutputNotDirectory { path: path.into() });
        }
        Ok(Self {
            path,
            exists: true,
            marker: marker_state(path)?,
            empty: is_empty(path)?,
        })
    }
    pub(super) fn require_replacement_permission(&self, clean_output: bool) -> Result<(), Error> {
        if self.marker == MarkerState::Invalid && !clean_output {
            return Err(Error::InvalidMarker {
                path: self.path.into(),
                marker: self.path.join(MARKER),
            });
        }
        if self.exists && !self.is_managed() && !self.empty && !clean_output {
            return Err(Error::UnmanagedOutput {
                path: self.path.into(),
            });
        }
        Ok(())
    }
    pub(super) fn require_update_permission(&self) -> Result<(), Error> {
        if self.marker == MarkerState::Invalid {
            return Err(Error::InvalidMarker {
                path: self.path.into(),
                marker: self.path.join(MARKER),
            });
        }
        if self.exists && !self.is_managed() && !self.empty {
            return Err(Error::UnmanagedOutput {
                path: self.path.into(),
            });
        }
        Ok(())
    }
    pub(super) fn exists(&self) -> bool {
        self.exists
    }
    pub(super) fn is_empty(&self) -> bool {
        self.empty
    }
    fn is_managed(&self) -> bool {
        self.marker == MarkerState::Current
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerState {
    Absent,
    Current,
    Invalid,
}
pub(super) fn write_marker(root: &Path) -> Result<(), Error> {
    write_file(&root.join(MARKER), MARKER_CONTENTS)
}
pub(super) fn write_file(path: &Path, contents: &str) -> Result<(), Error> {
    ensure_parent(path)?;
    fs::write(path, contents).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}

pub(super) fn ensure_directory(path: &Path) -> Result<(), Error> {
    fs::create_dir_all(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}

pub(super) fn ensure_parent(path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }
    Ok(())
}
pub(super) fn invalid_path(path: &Path, message: &'static str) -> Error {
    Error::Io {
        path: path.into(),
        source: io::Error::new(io::ErrorKind::InvalidInput, message),
    }
}
fn is_empty(path: &Path) -> Result<bool, Error> {
    fs::read_dir(path)
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?
        .next()
        .transpose()
        .map(|entry| entry.is_none())
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
}
fn marker_state(root: &Path) -> Result<MarkerState, Error> {
    let marker = root.join(MARKER);
    match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            fs::read_to_string(&marker)
                .map(|contents| {
                    if contents == MARKER_CONTENTS {
                        MarkerState::Current
                    } else {
                        MarkerState::Invalid
                    }
                })
                .map_err(|source| Error::Io {
                    path: marker,
                    source,
                })
        }
        Ok(_) => Ok(MarkerState::Invalid),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(MarkerState::Absent),
        Err(source) => Err(Error::Io {
            path: marker,
            source,
        }),
    }
}
