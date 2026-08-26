//! Shared invariants for paths below a project-owned source root.
//!
//! Source files become page IDs, asset IDs, template names, and URLs, so this
//! module keeps their UTF-8 and separator rules in one place. It deliberately
//! does not perform filesystem I/O or choose module-specific diagnostics.

use std::path::{Component, Path, PathBuf};

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

pub(crate) fn url(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(
                part.to_str()
                    .expect("validated source paths have safe UTF-8 components"),
            ),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => None,
        })
        .collect::<Vec<_>>()
        .join("/")
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
    fn simple_identifiers_remain_dot_addressable() {
        assert!(is_simple_identifier("posts"));
        assert!(is_simple_identifier("_internal2"));
        assert!(!is_simple_identifier("blog-posts"));
        assert!(!is_simple_identifier("2posts"));
        assert!(!is_simple_identifier(""));
    }
}
