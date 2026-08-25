//! Tera is contained in this module; the rest of the application uses only
//! render-facing serde values rather than template-engine types.

mod content;
mod localization;
mod resources;
mod runtime;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;
use walkdir::WalkDir;

use crate::{
    asset::AssetLinks,
    localization::Localization,
    site::{PageId, PageLinks, PageView},
    source_diagnostic,
};

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not read template {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not walk template directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("template path {path} must use UTF-8 components and must not contain backslashes")]
    NonUtf8Path { path: PathBuf },
    #[error("template path {path} is a symbolic link")]
    TemplateSymlink { path: PathBuf },
    #[error("could not load templates from {path}")]
    Load {
        path: PathBuf,
        #[source]
        source: tera::Error,
    },
    #[error("could not render template {template}")]
    Render {
        template: String,
        #[source]
        source: tera::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Detailed(#[from] Box<TemplateDiagnostic>),
}

/// Template-only data derived from the resolved site and asset catalog.
///
/// This keeps Tera's needs at the rendering boundary instead of letting them
/// shape the internal domain model.
pub(crate) struct TemplateResources {
    asset_links: AssetLinks,
    page_links: PageLinks,
    localization: std::sync::Arc<Localization>,
    highlight_css: Option<String>,
}

/// Values deliberately exposed to one template render. This is the boundary
/// between the resolved site model and Tera's serialization requirements.
#[derive(Serialize)]
pub(crate) struct RenderContext<'a> {
    pub(crate) locale: LocaleView,
    pub(crate) page: &'a PageView,
    pub(crate) pages: &'a BTreeMap<PageId, PageView>,
    pub(crate) tags: &'a BTreeMap<String, Vec<PageId>>,
    pub(crate) assets: &'a [String],
    pub(crate) data: &'a serde_json::Value,
    pub(crate) locale_data: &'a serde_json::Value,
    pub(crate) root: &'a str,
    #[serde(rename = "__heine_runtime")]
    pub(crate) runtime: RuntimeContext<'a>,
}

/// The locale identity deliberately exposed to templates. It does not leak
/// locale paths, bundles, or automatic direction guesses.
#[derive(Serialize)]
pub(crate) struct LocaleView {
    pub(crate) code: String,
}

/// Values required by Heine's built-ins but not part of the public template
/// model. Their reserved context key prevents ordinary template locals such as
/// `locale`, `page`, or `root` from changing how built-ins resolve resources.
#[derive(Serialize)]
pub(crate) struct RuntimeContext<'a> {
    pub(crate) locale: String,
    pub(crate) page_dir: &'a str,
    pub(crate) root: &'a str,
}

impl TemplateResources {
    pub(crate) fn new(
        asset_links: AssetLinks,
        page_links: PageLinks,
        localization: std::sync::Arc<Localization>,
        highlight_css: Option<String>,
    ) -> Self {
        Self {
            asset_links,
            page_links,
            localization,
            highlight_css,
        }
    }
}

/// A Tera failure tied back to a concrete source file owned by Heine.
#[derive(Debug, Diagnostic, Error)]
#[error("{stage} error in template {template:?}: {message}")]
#[diagnostic(code(heine::template::invalid))]
pub(crate) struct TemplateDiagnostic {
    stage: &'static str,
    template: String,
    message: String,
    #[source_code]
    source_code: miette::NamedSource<String>,
    label: String,
    #[label("{label}")]
    span: miette::SourceSpan,
}

pub(crate) struct TemplateEngine {
    tera: tera::Tera,
    sources: BTreeMap<String, miette::NamedSource<String>>,
}

impl TemplateEngine {
    pub(crate) fn load(
        templates: &Path,
        resources: TemplateResources,
        markdown_renderer: crate::markdown::Renderer,
    ) -> Result<Self, Error> {
        let mut tera = tera::Tera::default();
        let runtime = runtime::TemplateRuntime::from(resources);
        resources::register(&mut tera, &runtime);
        content::register(&mut tera, &runtime, markdown_renderer);
        localization::register(&mut tera, &runtime);
        let entries = WalkDir::new(templates)
            .sort_by_file_name()
            .into_iter()
            .map(|entry| {
                entry.map_err(|source| Error::Walk {
                    path: templates.into(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sources = entries
            .into_iter()
            .map(|entry| {
                if entry.file_type().is_symlink() {
                    Err(Error::TemplateSymlink {
                        path: entry.path().into(),
                    })
                } else {
                    Ok(entry)
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let path = entry.path().to_path_buf();
                let name = path
                    .strip_prefix(templates)
                    .expect("walked templates remain below their root");
                if !crate::source_path::has_safe_components(name) {
                    return Err(Error::NonUtf8Path { path });
                }
                let name = name
                    .to_str()
                    .ok_or_else(|| Error::NonUtf8Path { path: path.clone() })?;
                let name = name.replace('\\', "/");
                Ok((path, name))
            })
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .map(|(path, name)| {
                fs::read_to_string(&path)
                    .map(|source| (name, source_diagnostic::named_source(&path, source, "Tera")))
                    .map_err(|source| Error::Read { path, source })
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let raw_templates = sources
            .iter()
            .map(|(name, source)| (name.as_str(), source.inner().as_str()));
        tera.add_raw_templates(raw_templates)
            .map_err(|source| Error::from_tera("parse", templates, &sources, source))?;
        Ok(Self { tera, sources })
    }

    pub(crate) fn render(
        &self,
        template: &str,
        context: RenderContext<'_>,
    ) -> Result<String, Error> {
        let context =
            tera::Context::from_serialize(&context).expect("render context is serializable");
        self.tera.render(template, &context).map_err(|source| {
            Error::from_tera("render", Path::new(template), &self.sources, source)
        })
    }

    pub(crate) fn contains(&self, template: &str) -> bool {
        self.tera.contains_template(template)
    }
}

impl Error {
    fn from_tera(
        stage: &'static str,
        location: &Path,
        sources: &BTreeMap<String, miette::NamedSource<String>>,
        source: tera::Error,
    ) -> Self {
        if let tera::ErrorKind::MissingParent { current, parent } = source.kind()
            && let Some(diagnostic) = Self::template_reference_diagnostic(
                "template inheritance",
                current,
                parent,
                format!("parent template {parent:?} was not found"),
                sources,
            )
        {
            return diagnostic;
        }
        let report = match source.kind() {
            tera::ErrorKind::SyntaxError(report) | tera::ErrorKind::RenderingError(report) => {
                report
            }
            _ => {
                return if stage == "render" {
                    Self::Render {
                        template: location.display().to_string(),
                        source,
                    }
                } else {
                    Self::Load {
                        path: location.into(),
                        source,
                    }
                };
            }
        };
        let template = report.filename();
        let Some(source_code) = sources.get(template) else {
            return if stage == "render" {
                Self::Render {
                    template: location.display().to_string(),
                    source,
                }
            } else {
                Self::Load {
                    path: location.into(),
                    source,
                }
            };
        };
        let range = report.span().range.clone();
        Self::Detailed(Box::new(TemplateDiagnostic {
            stage,
            template: template.into(),
            message: report.message().into(),
            source_code: source_code.clone(),
            label: "error here".into(),
            span: (range.start, range.len()).into(),
        }))
    }

    fn template_reference_diagnostic(
        stage: &'static str,
        template: &str,
        reference: &str,
        message: String,
        sources: &BTreeMap<String, miette::NamedSource<String>>,
    ) -> Option<Self> {
        let source_code = sources.get(template)?;
        Some(Self::Detailed(Box::new(TemplateDiagnostic {
            stage,
            template: template.into(),
            message,
            source_code: source_code.clone(),
            label: "referenced parent template here".into(),
            span: parent_reference_span(source_code.inner(), reference),
        })))
    }
}

/// Locates the string literal in an `extends` directive without mistaking a
/// matching name in a comment or ordinary template text for the reference.
/// Tera's missing-parent error does not retain its own span, so this small
/// syntax-aware recovery preserves a useful authored location at that API
/// boundary.
fn parent_reference_span(source: &str, reference: &str) -> miette::SourceSpan {
    let mut remaining = source;
    let mut offset = 0;
    let mut directive_span = None;
    while let Some(open) = remaining.find("{%") {
        offset += open;
        let directive = &remaining[open + 2..];
        let directive_offset = offset + 2;
        let (directive, trimmed) = trim_template_whitespace(directive);
        let directive_offset = directive_offset + trimmed;
        let (directive, trim_marker) = match directive.strip_prefix('-') {
            Some(directive) => (directive, 1),
            None => (directive, 0),
        };
        let directive_offset = directive_offset + trim_marker;
        let (directive, trimmed) = trim_template_whitespace(directive);
        let directive_offset = directive_offset + trimmed;
        if let Some(after_extends) = directive.strip_prefix("extends")
            && after_extends
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            directive_span = Some(
                (
                    directive_offset,
                    directive.find("%}").unwrap_or(directive.len()),
                )
                    .into(),
            );
            let extends_length = directive.len() - after_extends.len();
            let (argument, trimmed) = trim_template_whitespace(after_extends);
            let argument_offset = directive_offset + extends_length + trimmed;
            if let Some((value, value_offset)) = quoted_template_string(argument)
                && value == reference
            {
                return (argument_offset + value_offset, value.len()).into();
            }
        }
        let consumed = open + 2;
        remaining = &remaining[consumed..];
        offset += 2;
    }
    directive_span.unwrap_or((0, 0).into())
}

fn trim_template_whitespace(value: &str) -> (&str, usize) {
    let trimmed = value.trim_start_matches(char::is_whitespace);
    (trimmed, value.len() - trimmed.len())
}

fn quoted_template_string(value: &str) -> Option<(&str, usize)> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value = &value[quote.len_utf8()..];
    let end = value.find(quote)?;
    Some((&value[..end], quote.len_utf8()))
}
