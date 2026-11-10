//! Creation of independent sites from the files shipped with Heine.

mod files;

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Failure to create a site, including any entries left after cleanup.
#[derive(Debug, thiserror::Error)]
#[error("could not create site entry {path}: {source}{cleanup}", path = .path.display())]
pub struct CreateError {
    path: PathBuf,
    #[source]
    source: io::Error,
    cleanup: String,
}

enum Created {
    Directory(PathBuf),
    File(PathBuf, &'static [u8]),
}

/// Creates a site at a destination that does not already exist.
pub fn create(destination: &Path) -> Result<(), CreateError> {
    create_with_files(destination, files::FILES)
}

fn create_with_files(
    destination: &Path,
    files: &[(&str, &'static [u8])],
) -> Result<(), CreateError> {
    fs::create_dir(destination).map_err(|source| CreateError {
        path: destination.to_owned(),
        cleanup: if source.kind() == io::ErrorKind::AlreadyExists {
            "; choose a destination that does not exist".to_owned()
        } else {
            String::new()
        },
        source,
    })?;
    let mut created = vec![Created::Directory(destination.to_owned())];
    let mut directories = BTreeSet::new();
    let result = (|| {
        for &(name, bytes) in files {
            let relative = Path::new(name);
            let mut current = destination.to_owned();
            if let Some(parent) = relative.parent() {
                for part in parent.components() {
                    current.push(part);
                    if directories.insert(current.clone()) {
                        fs::create_dir(&current).map_err(|error| (current.clone(), error))?;
                        created.push(Created::Directory(current.clone()));
                    }
                }
            }
            let path = destination.join(relative);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|error| (path.clone(), error))?;
            created.push(Created::File(path.clone(), bytes));
            file.write_all(bytes).map_err(|error| (path, error))?;
        }
        Ok(())
    })();
    if let Err((path, source)) = result {
        let remaining = cleanup(created);
        let cleanup = if remaining.is_empty() {
            String::new()
        } else {
            format!("; could not safely remove: {}", remaining.join(", "))
        };
        return Err(CreateError {
            path,
            source,
            cleanup,
        });
    }
    Ok(())
}

fn cleanup(created: Vec<Created>) -> Vec<String> {
    let mut remaining = Vec::new();
    for entry in created.into_iter().rev() {
        let path = match &entry {
            Created::Directory(path) | Created::File(path, _) => path,
        };
        let removed = match entry {
            Created::File(ref path, expected) => {
                fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
                    && fs::read(path).is_ok_and(|bytes| bytes == expected)
                    && fs::remove_file(path).is_ok()
            }
            Created::Directory(ref path) => {
                fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
                    && fs::remove_dir(path).is_ok()
            }
        };
        if !removed {
            remaining.push(path.display().to_string());
        }
    }
    remaining
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn inventory_matches_the_authored_site() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("new-site");
        let expected: BTreeSet<_> = files::FILES.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            expected.len(),
            files::FILES.len(),
            "duplicate inventory path"
        );
        for name in &expected {
            assert!(
                !name.contains('\\')
                    && Path::new(name)
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_))),
                "invalid inventory path: {name}"
            );
        }
        let actual: BTreeSet<_> = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| {
                entry.depth() != 1
                    || !matches!(entry.file_name().to_str(), Some("public" | ".heine"))
            })
            .map(|entry| {
                let entry = entry.expect("read site entry");
                assert!(
                    entry.file_type().is_file() || entry.file_type().is_dir(),
                    "unexpected source entry: {}",
                    entry.path().display()
                );
                entry
            })
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                entry
                    .path()
                    .strip_prefix(&root)
                    .expect("site path")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            actual,
            expected.iter().map(|name| name.to_string()).collect()
        );
        assert_eq!(
            fs::read(root.join("content/assets/js/search.js")).expect("new search client"),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("starter-site/kits/starter/assets/js/search.js")
            )
            .expect("canonical search client")
        );
    }

    #[test]
    fn creates_identical_sites_and_never_overwrites_one() {
        let temporary = tempfile::tempdir().expect("temporary parent");
        let first = temporary.path().join("first site");
        let second = temporary.path().join("second");
        create(&first).expect("first site");
        create(&second).expect("second site");
        for &(path, bytes) in files::FILES {
            assert_eq!(fs::read(first.join(path)).expect("first file"), bytes);
            assert_eq!(fs::read(second.join(path)).expect("second file"), bytes);
        }
        fs::write(first.join("README.md"), "author changes").expect("edit site");
        assert!(create(&first).is_err());
        assert_eq!(
            fs::read_to_string(first.join("README.md")).expect("preserved file"),
            "author changes"
        );
    }

    #[test]
    fn failed_creation_removes_its_own_complete_entries() {
        let temporary = tempfile::tempdir().expect("temporary parent");
        let destination = temporary.path().join("site");
        let files: &[(&str, &[u8])] = &[("content/one", b"one"), ("content/one", b"two")];
        assert!(create_with_files(&destination, files).is_err());
        assert!(!destination.exists());
    }

    #[test]
    fn cleanup_preserves_unrecognized_content() {
        let temporary = tempfile::tempdir().expect("temporary parent");
        let directory = temporary.path().join("site");
        fs::create_dir(&directory).expect("site directory");
        let own = directory.join("own");
        let other = directory.join("other");
        fs::write(&own, b"own").expect("created file");
        fs::write(&other, b"other").expect("unrecognized file");
        let remaining = cleanup(vec![
            Created::Directory(directory.clone()),
            Created::File(own.clone(), b"own"),
        ]);
        assert!(!own.exists());
        assert_eq!(fs::read(other).expect("unrecognized file"), b"other");
        assert_eq!(remaining.len(), 1);
        assert!(directory.exists());
    }
}
