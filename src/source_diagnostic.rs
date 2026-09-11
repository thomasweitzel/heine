//! Shared source-location support for structured user-authored files.

use miette::{NamedSource, SourceSpan};
use std::path::{Path, PathBuf};

/// The project-owned naming policy for diagnostics that identify authored
/// sources. Filesystem diagnostics retain their physical paths instead.
#[derive(Debug, Clone)]
pub(crate) struct DiagnosticPaths {
    project_root: PathBuf,
}

impl DiagnosticPaths {
    pub(crate) fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    /// Returns a stable project-relative name when the source belongs to this
    /// project. The unchanged physical path makes diagnostics for unexpected
    /// external inputs useful without guessing a relationship that is absent.
    pub(crate) fn display_path(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.project_root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.into())
    }

    pub(crate) fn named_source(
        &self,
        path: &Path,
        source: impl Into<String>,
        language: &'static str,
    ) -> NamedSource<String> {
        named_source_at_display_path(&self.display_path(path), source, language)
    }
}

/// Creates a source diagnostic from an already-normalized display path.
///
/// Callers with a physical filesystem path must use
/// [`DiagnosticPaths::named_source`] instead.
pub(crate) fn named_source_at_display_path(
    path: &Path,
    source: impl Into<String>,
    language: &'static str,
) -> NamedSource<String> {
    NamedSource::new(path.display().to_string(), source.into()).with_language(language)
}

pub(crate) fn json_span(source: &str, error: &serde_json::Error) -> SourceSpan {
    let line_start = source
        .lines()
        .take(error.line().saturating_sub(1))
        .map(|line| line.len() + 1)
        .sum::<usize>();
    let offset = line_start + error.column().saturating_sub(1);
    (offset, 1).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_paths_are_relative_only_inside_the_project() {
        let diagnostics = DiagnosticPaths::new(Path::new("/work/site"));

        assert_eq!(
            diagnostics.display_path(Path::new("/work/site/content/index.page")),
            PathBuf::from("content/index.page")
        );
        assert_eq!(
            diagnostics.display_path(Path::new("/work/site-other/content/index.page")),
            PathBuf::from("/work/site-other/content/index.page")
        );
    }
}
