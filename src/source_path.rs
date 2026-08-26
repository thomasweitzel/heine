//! Shared invariants for paths below a project-owned source root.
//!
//! Source files become page IDs, asset IDs, template names, and URLs, so this
//! module keeps their UTF-8 and separator rules in one place. It deliberately
//! does not perform filesystem I/O or choose module-specific diagnostics.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

/// Two project-owned paths that remain distinct as byte strings but have the
/// same NFC spelling. Heine preserves the authored spelling, but such a pair
/// cannot be addressed predictably across filesystems with different Unicode
/// normalization policies.
#[derive(Debug, Diagnostic, Error)]
#[error("paths {first} and {second} differ only by Unicode normalization")]
#[diagnostic(
    code(heine::source_path::normalization_collision),
    help("rename one path; use one NFC-normalized spelling consistently")
)]
pub(crate) struct NormalizationCollision {
    first: PathBuf,
    second: PathBuf,
}

/// A validated logical path below one project-owned source root.
///
/// It is deliberately distinct from a filesystem path: it has no root, dot
/// components, or backslashes, and every component is UTF-8. Asset and page
/// lookups accept this grammar at their template boundary before resolving it
/// against their separate domains.
#[derive(Debug, Clone)]
pub(crate) struct LogicalPath {
    path: PathBuf,
    spelling: String,
}

// The spelling, rather than the platform-native `PathBuf`, is the identity of
// a logical path. This keeps a discovered Windows path and a slash-separated
// template ID in one stable, cross-platform namespace.
impl PartialEq for LogicalPath {
    fn eq(&self, other: &Self) -> bool {
        self.spelling == other.spelling
    }
}

impl Eq for LogicalPath {}

impl PartialOrd for LogicalPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogicalPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.spelling.cmp(&other.spelling)
    }
}

impl LogicalPath {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if value.contains('\\') {
            return None;
        }
        let path = PathBuf::from(value);
        Self::from_path(&path)
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.spelling
    }

    pub(crate) fn from_path(path: &Path) -> Option<Self> {
        is_logical_id(path).then(|| Self {
            path: path.into(),
            spelling: path
                .components()
                .filter_map(|component| match component {
                    Component::Normal(part) => part.to_str(),
                    Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_) => None,
                })
                .collect::<Vec<_>>()
                .join("/"),
        })
    }
}

/// Finds the first deterministic NFC collision among already validated logical
/// paths. `logical` supplies the namespace identity, while `source` is kept
/// verbatim for a useful filesystem diagnostic.
pub(crate) fn normalization_collision(
    paths: impl IntoIterator<Item = (PathBuf, PathBuf)>,
) -> Option<NormalizationCollision> {
    let mut seen = BTreeMap::new();
    for (logical, source) in paths {
        let key = normalized_spelling(&logical)?;
        if let Some(first) = seen.insert(key, source.clone()) {
            return Some(NormalizationCollision {
                first,
                second: source,
            });
        }
    }
    None
}

/// NFC-normalizes a string without changing the spelling stored in the source
/// or output model. This is a collision key, never a rewritten identifier.
pub(crate) fn nfc(value: &str) -> String {
    value.nfc().collect()
}

fn normalized_spelling(path: &Path) -> Option<String> {
    path.components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str().map(nfc),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
}

pub(crate) fn relative_to(root: &Path, path: &Path) -> Option<PathBuf> {
    path.strip_prefix(root).ok().map(Path::to_path_buf)
}

pub(crate) fn has_safe_components(path: &Path) -> bool {
    path.components().all(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|component| !component.contains('\\'))
    })
}

/// Validates a template-facing asset ID, which is always rooted at `content/`.
pub(crate) fn is_logical_id(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(
            |component| matches!(component, Component::Normal(part) if part.to_str().is_some_and(|part| !part.contains('\\'))),
        )
}

/// Returns whether a value can be addressed predictably through Tera dot
/// notation. Configuration and authored metadata use this for names that
/// become template-map keys.
pub(crate) fn is_simple_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backslashes_are_not_valid_source_or_logical_path_syntax() {
        let path = Path::new("images\\logo.svg");

        assert!(!has_safe_components(path));
        assert!(!is_logical_id(path));
    }

    #[test]
    fn logical_paths_retain_only_valid_relative_components() {
        assert_eq!(
            LogicalPath::parse("images/logo.svg")
                .as_ref()
                .map(LogicalPath::as_path),
            Some(Path::new("images/logo.svg"))
        );
        assert!(LogicalPath::parse("../logo.svg").is_none());
        assert!(LogicalPath::parse("/logo.svg").is_none());
        assert!(LogicalPath::parse("images\\logo.svg").is_none());
    }

    #[test]
    fn logical_path_identity_uses_its_canonical_spelling() {
        let discovered =
            LogicalPath::from_path(Path::new("images//logo.svg")).expect("logical source path");
        let requested = LogicalPath::parse("images/logo.svg").expect("logical template ID");

        assert_eq!(discovered, requested);
        assert_eq!(discovered.cmp(&requested), std::cmp::Ordering::Equal);
    }

    #[test]
    fn normalization_collisions_preserve_both_authored_paths() {
        let first = PathBuf::from("café.page");
        let second = PathBuf::from("cafe\u{301}.page");
        let collision = normalization_collision([
            (first.clone(), PathBuf::from("content/en").join(&first)),
            (second.clone(), PathBuf::from("content/en").join(&second)),
        ])
        .expect("NFC-equivalent paths collide");

        assert!(collision.to_string().contains("café.page"));
        assert!(collision.to_string().contains("cafe\u{301}.page"));
    }

    #[test]
    fn simple_identifiers_remain_dot_addressable() {
        assert!(is_simple_identifier("posts"));
        assert!(is_simple_identifier("_internal2"));
        assert!(!is_simple_identifier("blog-posts"));
        assert!(!is_simple_identifier("2posts"));
        assert!(!is_simple_identifier(""));
    }
}
