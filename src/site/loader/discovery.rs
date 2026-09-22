//! Safe, deterministic discovery of one content tree.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use walkdir::{DirEntry, WalkDir};

use crate::{site::Error, source_path};

pub(in crate::site) fn content_entries(root: &Path) -> Result<Vec<DirEntry>, Error> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Walk)
        .and_then(|entries| {
            for entry in &entries {
                if entry.file_type().is_symlink() {
                    return Err(Error::ContentSymlink {
                        path: entry.path().into(),
                    });
                }
                let relative = relative_path(root, entry.path())?;
                if !source_path::has_safe_components(&relative) {
                    return Err(Error::NonUtf8Path {
                        path: entry.path().into(),
                    });
                }
            }
            if let Some(collision) = source_path::normalization_collision(
                entries
                    .iter()
                    .filter(|entry| entry.path() != root)
                    .map(|entry| {
                        relative_path(root, entry.path())
                            .map(|relative| (relative, entry.path().into()))
                    })
                    .collect::<Result<Vec<_>, Error>>()?,
            ) {
                return Err(Error::NormalizationCollision(Box::new(collision)));
            }
            Ok(entries)
        })
}

/// Builds the child-directory view once for a locale tree instead of scanning
/// every discovered entry again for every page in that tree.
pub(in crate::site) fn subdirectories(entries: &[DirEntry]) -> BTreeMap<PathBuf, Vec<String>> {
    let mut directories: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for entry in entries.iter().filter(|entry| entry.file_type().is_dir()) {
        let Some(parent) = entry.path().parent() else {
            continue;
        };
        let name = entry
            .file_name()
            .to_str()
            .expect("content discovery validates safe UTF-8 path components")
            .to_owned();
        directories
            .entry(parent.to_path_buf())
            .or_default()
            .push(name);
    }
    for names in directories.values_mut() {
        names.sort();
    }
    directories
}

pub(in crate::site) fn relative_path(root: &Path, path: &Path) -> Result<PathBuf, Error> {
    source_path::relative_to(root, path).ok_or_else(|| Error::OutsideContent {
        path: path.into(),
        root: root.into(),
    })
}
