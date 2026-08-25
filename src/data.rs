//! Loading of non-published, site-wide structured template data.
//!
//! `data/` is deliberately a small, deterministic input namespace rather
//! than a second content tree or a filesystem lookup mechanism. JSON and TOML
//! files map to keys formed by their path relative to `data/`, without the
//! extension.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_json::{Map, Value};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{locale::Locale, source_diagnostic, structured};

/// Validated site-wide template data and the latest source input that formed
/// it. The timestamp lets quick builds re-render pages when shared data
/// changes.
#[derive(Debug)]
pub(crate) struct SiteData {
    value: Value,
    latest_input: Option<SystemTime>,
}

impl SiteData {
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        Self::load_excluding(root, &BTreeSet::new())
    }

    /// Loads shared data while reserving configured locale directories for
    /// `locale_data`. This prevents one input from silently appearing in two
    /// template namespaces.
    pub(crate) fn load_excluding(
        root: &Path,
        excluded_top_level: &BTreeSet<String>,
    ) -> Result<Self, Error> {
        match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::DataSymlink { path: root.into() });
            }
            Ok(metadata) if !metadata.is_dir() => return Err(Error::NotDirectory(root.into())),
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::empty()),
            Err(source) => {
                return Err(Error::Inspect {
                    path: root.into(),
                    source,
                });
            }
        }

        let mut tree = BTreeMap::new();
        let mut latest_input: Option<SystemTime> = None;
        for entry in WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| {
                entry.path() == root
                    || !entry.path().parent().is_some_and(|parent| {
                        parent == root
                            && excluded_top_level
                                .contains(&entry.file_name().to_string_lossy().into_owned())
                    })
            })
        {
            let entry = entry.map_err(|source| Error::Walk {
                path: root.into(),
                source,
            })?;
            if entry.file_type().is_dir() {
                continue;
            }
            if entry.file_type().is_symlink() {
                return Err(Error::DataSymlink {
                    path: entry.path().into(),
                });
            }
            if !entry.file_type().is_file() {
                return Err(Error::UnsupportedEntry {
                    path: entry.path().into(),
                });
            }
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|_| Error::OutsideRoot {
                path: path.into(),
                root: root.into(),
            })?;
            let keys = data_keys(relative, path)?;
            let (value, modified) = load_file(path)?;
            latest_input = Some(match latest_input {
                Some(latest) => latest.max(modified),
                None => modified,
            });
            insert(&mut tree, &keys, value, path)?;
        }

        Ok(Self {
            value: Value::Object(to_json_map(tree)),
            latest_input,
        })
    }

    pub(crate) fn load_localized<'a>(
        root: &Path,
        locales: impl IntoIterator<Item = &'a Locale>,
    ) -> Result<(Self, BTreeMap<Locale, Self>), Error> {
        let locales = locales.into_iter().cloned().collect::<Vec<_>>();
        let excluded = locales.iter().map(ToString::to_string).collect();
        let shared = Self::load_excluding(root, &excluded)?;
        let localized = locales
            .iter()
            .map(|locale| {
                let locale_root = root.join(locale.to_string());
                Self::load(&locale_root).map(|data| (locale.clone(), data))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok((shared, localized))
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn latest_input(&self) -> Option<SystemTime> {
        self.latest_input
    }

    fn empty() -> Self {
        Self {
            value: Value::Object(Map::new()),
            latest_input: None,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
pub(crate) enum Error {
    #[error("data path {0} is not a directory")]
    NotDirectory(PathBuf),
    #[error("could not inspect data path {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not walk data directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("data entry {path} must be a regular file or directory")]
    UnsupportedEntry { path: PathBuf },
    #[error("data path {path} is a symbolic link")]
    DataSymlink { path: PathBuf },
    #[error("data file {path} must use a .json or .toml extension")]
    UnsupportedFormat { path: PathBuf },
    #[error("data path {path} contains a key that cannot be used with data.* in templates")]
    InvalidKey { path: PathBuf },
    #[error("data path {path} must use UTF-8 path components")]
    NonUtf8Path { path: PathBuf },
    #[error("data path {path} is outside data root {root}")]
    OutsideRoot { path: PathBuf, root: PathBuf },
    #[error("data paths {first} and {second} produce the same template key")]
    KeyCollision { first: PathBuf, second: PathBuf },
    #[error("could not read data file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Invalid(#[from] Box<DataDiagnostic>),
}

#[derive(Debug, Error, Diagnostic)]
#[error("invalid {format} data in {path}")]
#[diagnostic(code(heine::data::invalid))]
pub(crate) struct DataDiagnostic {
    path: PathBuf,
    format: &'static str,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{message}")]
    span: SourceSpan,
}

enum Node {
    Branch(BTreeMap<String, Node>),
    Leaf { value: Value, path: PathBuf },
}

fn data_keys(path: &Path, full_path: &Path) -> Result<Vec<String>, Error> {
    if path
        .components()
        .any(|component| component.as_os_str().to_str().is_none())
    {
        return Err(Error::NonUtf8Path {
            path: full_path.into(),
        });
    }
    if structured::Format::from_path(path).is_none() {
        return Err(Error::UnsupportedFormat {
            path: full_path.into(),
        });
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| Error::NonUtf8Path {
            path: full_path.into(),
        })?;
    let mut keys = path
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| Error::NonUtf8Path {
            path: full_path.into(),
        })?;
    keys.push(stem.to_owned());
    if keys.iter().all(|key| is_template_identifier(key)) {
        Ok(keys)
    } else {
        Err(Error::InvalidKey {
            path: full_path.into(),
        })
    }
}

fn is_template_identifier(key: &str) -> bool {
    let mut characters = key.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn load_file(path: &Path) -> Result<(Value, SystemTime), Error> {
    let source = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.into(),
        source,
    })?;
    let format = structured::Format::from_path(path)
        .expect("data_keys validates the extension before loading the file");
    let value = structured::parse(format, &source).map_err(|error| {
        invalid_diagnostic(path, format.name(), &source, error.message, error.span)
    })?;
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| Error::Inspect {
            path: path.into(),
            source,
        })?;
    Ok((value, modified))
}

fn invalid_diagnostic(
    path: &Path,
    format: &'static str,
    source: &str,
    message: String,
    span: SourceSpan,
) -> Error {
    Error::Invalid(Box::new(DataDiagnostic {
        path: path.into(),
        format,
        message,
        source_code: source_diagnostic::named_source(path, source, format),
        span,
    }))
}

fn insert(
    tree: &mut BTreeMap<String, Node>,
    keys: &[String],
    value: Value,
    path: &Path,
) -> Result<(), Error> {
    let (last, parents) = keys.split_last().expect("data paths have a file stem");
    let mut branch = tree;
    for key in parents {
        let node = branch
            .entry(key.clone())
            .or_insert_with(|| Node::Branch(BTreeMap::new()));
        match node {
            Node::Branch(next) => branch = next,
            Node::Leaf { path: first, .. } => {
                return Err(Error::KeyCollision {
                    first: first.clone(),
                    second: path.into(),
                });
            }
        }
    }
    match branch.entry(last.clone()) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(Node::Leaf {
                value,
                path: path.into(),
            });
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) => Err(Error::KeyCollision {
            first: match entry.get() {
                Node::Leaf { path, .. } => path.clone(),
                Node::Branch(branch) => first_path(branch),
            },
            second: path.into(),
        }),
    }
}

fn first_path(branch: &BTreeMap<String, Node>) -> PathBuf {
    match branch
        .values()
        .next()
        .expect("a branch was created for a file")
    {
        Node::Leaf { path, .. } => path.clone(),
        Node::Branch(branch) => first_path(branch),
    }
}

fn to_json_map(tree: BTreeMap<String, Node>) -> Map<String, Value> {
    tree.into_iter()
        .map(|(key, node)| {
            let value = match node {
                Node::Branch(branch) => Value::Object(to_json_map(branch)),
                Node::Leaf { value, .. } => value,
            };
            (key, value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_identifiers_are_simple_and_dot_accessible() {
        assert!(is_template_identifier("navigation"));
        assert!(is_template_identifier("menu_2"));
        assert!(!is_template_identifier("2menu"));
        assert!(!is_template_identifier("main-menu"));
    }
}
