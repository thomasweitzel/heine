//! Raw and validated page representations, plus the template-facing page view.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::SystemTime,
};

use miette::{NamedSource, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{Date, OffsetDateTime};

use crate::{content::MarkdownSource, output::OutputPath, source_path};

/// A stable logical page identifier, always relative to `content/` without `.page`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(crate) struct PageId(String);

impl PageId {
    pub(super) fn from_source_path(path: &Path) -> Self {
        Self(source_path::url(path))
    }

    pub(super) fn is_index(&self) -> bool {
        self.0 == "index" || self.0.ends_with("/index")
    }
}

impl std::fmt::Display for PageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A template identifier relative to `templates/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TemplateName(String);

impl TemplateName {
    pub(super) fn new(name: String) -> Self {
        Self(name)
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TemplateName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A URL path relative to the generated site's root.
#[derive(Debug, Clone)]
pub(super) struct UrlPath(String);

impl UrlPath {
    pub(super) fn from_output(path: &OutputPath) -> Self {
        Self(source_path::url(path.as_path()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct Task {
    pub(super) page_id: PageId,
    pub(super) template: TemplateName,
    pub(super) page_source: NamedSource<String>,
    pub(super) template_source: NamedSource<String>,
    pub(super) template_span: SourceSpan,
    pub(super) output_span: SourceSpan,
    pub(super) output_path: OutputPath,
    pub(super) latest_input: SystemTime,
}

/// A page resolved from its serialized form, retaining the source inputs used
/// to classify the remainder of `content/` as publishable assets.
pub(super) struct LoadedPage {
    pub(super) page: Page,
    pub(super) task: Option<Task>,
    pub(super) content_inputs: BTreeSet<PathBuf>,
    pub(super) markdown_sources: Vec<MarkdownSource>,
}

/// The TOML shape written by site authors. This is intentionally separate
/// from `Page`, whose fields represent only validated application state.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPage {
    pub(super) template: Option<toml::Spanned<toml::Value>>,
    pub(super) title: Option<toml::Spanned<String>>,
    pub(super) description: Option<toml::Spanned<String>>,
    pub(super) summary: Option<toml::Spanned<String>>,
    pub(super) date: Option<toml::Spanned<toml::Value>>,
    pub(super) extra: Option<toml::Spanned<toml::Value>>,
    #[serde(default)]
    pub(super) content: BTreeMap<String, toml::Spanned<String>>,
    #[serde(default)]
    pub(super) tags: Vec<String>,
    pub(super) weight: Option<toml::Spanned<toml::Value>>,
    #[serde(default = "default_suffix")]
    pub(super) suffix: toml::Spanned<String>,
}

/// The common authored metadata shape for pages and directory descriptors.
#[derive(Debug, Default)]
pub(super) struct RawMetadata {
    pub(super) title: Option<toml::Spanned<String>>,
    pub(super) description: Option<toml::Spanned<String>>,
    pub(super) summary: Option<toml::Spanned<String>>,
    pub(super) date: Option<toml::Spanned<toml::Value>>,
    pub(super) extra: Option<toml::Spanned<toml::Value>>,
}

/// A validated authored date, retained in a form future chronological indexes
/// can order without reparsing template-facing strings.
#[derive(Debug, Clone)]
pub(super) enum MetadataDate {
    Date(Date),
    OffsetDateTime(OffsetDateTime),
}

impl MetadataDate {
    pub(super) fn render(&self) -> String {
        match self {
            Self::Date(value) => value.to_string(),
            Self::OffsetDateTime(value) => value
                .format(&time::format_description::well_known::Rfc3339)
                .expect("valid offset date-times always format as RFC 3339"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Metadata {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) date: Option<MetadataDate>,
    pub(super) extra: Value,
}

impl Metadata {
    pub(super) fn field(&self, name: &str) -> bool {
        match name {
            "title" => self.title.is_some(),
            "description" => self.description.is_some(),
            "summary" => self.summary.is_some(),
            "date" => self.date.is_some(),
            _ => name
                .strip_prefix("extra.")
                .and_then(|path| {
                    path.split('.')
                        .try_fold(&self.extra, |value, part| value.get(part))
                })
                .is_some(),
        }
    }
}

fn default_suffix() -> toml::Spanned<String> {
    toml::Spanned::new(0..0, ".html".into())
}

/// A single filename extension used to derive a rendered page path.
///
/// It deliberately cannot carry path syntax: rendered output locations are
/// derived only from a page's validated source-relative path.
#[derive(Debug, Clone)]
pub(super) struct OutputSuffix(String);

impl OutputSuffix {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OutputSuffix {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let extension = value.trim_start_matches('.');
        if extension.is_empty()
            || extension.contains(['/', '\\'])
            || extension.contains('\0')
            || extension == "."
            || extension == ".."
        {
            return Err(
                "field `suffix` must be a non-empty filename extension without path separators"
                    .into(),
            );
        }
        Ok(Self(extension.into()))
    }
}

/// Validated page state. It deliberately has no template-engine traits.
#[derive(Debug, Clone)]
pub(super) struct Page {
    pub(super) id: PageId,
    pub(super) path: Option<UrlPath>,
    pub(super) dir: String,
    pub(super) filename: Option<String>,
    pub(super) subdirs: Vec<String>,
    pub(super) content: BTreeMap<String, Value>,
    pub(super) metadata: Metadata,
    pub(super) directory: DirectoryView,
    pub(super) tags: Vec<String>,
    pub(super) weight: Option<i64>,
    pub(super) siblings: Vec<PageId>,
    pub(super) lighter: Option<PageId>,
    pub(super) heavier: Option<PageId>,
}

impl Page {
    pub(super) fn view(&self) -> PageView {
        PageView {
            id: self.id.clone(),
            path: self.path.as_ref().map(|path| path.as_str().to_owned()),
            dir: self.dir.clone(),
            filename: self.filename.clone(),
            subdirs: self.subdirs.clone(),
            content: self.content.clone(),
            title: self.metadata.title.clone(),
            description: self.metadata.description.clone(),
            summary: self.metadata.summary.clone(),
            date: self.metadata.date.as_ref().map(MetadataDate::render),
            extra: self.metadata.extra.clone(),
            directory: self.directory.clone(),
            tags: self.tags.clone(),
            weight: self.weight,
            siblings: self.siblings.clone(),
            lighter: self.lighter.clone(),
            heavier: self.heavier.clone(),
        }
    }
}

/// Deliberate, serializable template contract derived from a validated page.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PageView {
    id: PageId,
    path: Option<String>,
    dir: String,
    filename: Option<String>,
    subdirs: Vec<String>,
    content: BTreeMap<String, Value>,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    date: Option<String>,
    extra: Value,
    directory: DirectoryView,
    tags: Vec<String>,
    weight: Option<i64>,
    siblings: Vec<PageId>,
    lighter: Option<PageId>,
    heavier: Option<PageId>,
}

/// The immediate containing directory's deliberately small template view.
#[derive(Debug, Clone, Default, Serialize)]
pub(super) struct DirectoryView {
    id: String,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    extra: Value,
}

impl DirectoryView {
    pub(super) fn new(id: String, metadata: &Metadata) -> Self {
        Self {
            id,
            title: metadata.title.clone(),
            description: metadata.description.clone(),
            summary: metadata.summary.clone(),
            extra: metadata.extra.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_index_page_names_receive_index_ordering() {
        assert!(PageId("index".into()).is_index());
        assert!(PageId("blog/index".into()).is_index());
        assert!(!PageId("thinking".into()).is_index());
        assert!(!PageId("myindex".into()).is_index());
    }
}
