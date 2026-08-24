//! Tera is contained in this module; the rest of the application uses only
//! render-facing serde values rather than template-engine types.

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
    markdown,
    site::{PageId, PageView},
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
    base_path: String,
    pages: BTreeMap<String, PathBuf>,
    highlight_css: Option<String>,
}

/// Values deliberately exposed to one template render. This is the boundary
/// between the resolved site model and Tera's serialization requirements.
#[derive(Serialize)]
pub(crate) struct RenderContext<'a> {
    pub(crate) page: &'a PageView,
    pub(crate) pages: &'a BTreeMap<PageId, PageView>,
    pub(crate) tags: &'a BTreeMap<String, Vec<PageId>>,
    pub(crate) assets: &'a [String],
    pub(crate) data: &'a serde_json::Value,
    pub(crate) root: &'a str,
}

impl TemplateResources {
    pub(crate) fn new(
        asset_links: AssetLinks,
        base_path: String,
        pages: BTreeMap<String, PathBuf>,
        highlight_css: Option<String>,
    ) -> Self {
        Self {
            asset_links,
            base_path,
            pages,
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
        let page_targets = resources.pages;
        let page_base = resources.base_path.clone();
        let asset_function_links = resources.asset_links.clone();
        let latest_links = resources.asset_links.clone();
        let latest_base = resources.base_path.clone();
        let dimensions_links = resources.asset_links.clone();
        let markdown_asset_links = resources.asset_links;
        let highlight_css = resources.highlight_css;
        let markdown_renderer_for_filter = markdown_renderer;
        tera.register_function(
            "highlight_css",
            move |_: tera::Kwargs, _: &tera::State| -> tera::TeraResult<tera::Value> {
                highlight_css.clone().map(Into::into).ok_or_else(|| {
                    tera::Error::message("syntax highlighting is not enabled; configure [markdown.highlighting] in heine.toml")
                })
            },
        );
        tera.register_function(
            "asset",
            move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
                let asset = asset_function_links
                    .resolve(kwargs.must_get::<&str>("id")?)
                    .map_err(|error| tera::Error::message(error.to_string()))?;
                let root = state.get::<String>("root")?.unwrap_or_default();
                let mut result = tera::Map::new();
                result.insert("url".into(), asset.url().into());
                result.insert(
                    "relative_url".into(),
                    format!("{root}{}", asset.path().to_string_lossy()).into(),
                );
                result.insert(
                    "integrity".into(),
                    asset.integrity().map(str::to_owned).into(),
                );
                let (width, height) = asset.dimensions().unzip();
                result.insert("width".into(), width.into());
                result.insert("height".into(), height.into());
                Ok(result.into())
            },
        );
        tera.register_function(
            "page",
            move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
                let id = kwargs.must_get::<&str>("id")?;
                let path = page_targets.get(id).ok_or_else(|| {
                    tera::Error::message(format!("rendered page {id:?} was not found"))
                })?;
                let root = state.get::<String>("root")?.unwrap_or_default();
                let mut result = tera::Map::new();
                result.insert(
                    "url".into(),
                    format!("{page_base}{}", path.to_string_lossy()).into(),
                );
                result.insert(
                    "relative_url".into(),
                    format!("{root}{}", path.to_string_lossy()).into(),
                );
                Ok(result.into())
            },
        );
        tera.register_filter(
            "latest",
            move |input: &str, _: tera::Kwargs, state: &tera::State| -> tera::TeraResult<String> {
                let page_dir = page_dir(state)?;
                let page_dir = Path::new(&page_dir);
                let (path, content_root_relative) = latest_links
                    .resolve_template_path(page_dir, input)
                    .map_err(|error| tera::Error::message(error.to_string()))?;
                let path = if content_root_relative {
                    crate::source_path::url(&path)
                } else {
                    crate::source_path::url(path.strip_prefix(page_dir).unwrap_or(&path))
                };
                Ok(if content_root_relative {
                    format!("{latest_base}{path}")
                } else {
                    path
                })
            },
        );
        tera.register_filter(
            "dimensions",
            move |input: &str,
                  _: tera::Kwargs,
                  state: &tera::State|
                  -> tera::TeraResult<tera::Value> {
                let page_dir = page_dir(state)?;
                let (key, _) = dimensions_links
                    .resolve_template_path(Path::new(&page_dir), input)
                    .map_err(|error| tera::Error::message(error.to_string()))?;
                let mut result = tera::Map::new();
                if let Some((width, height)) = dimensions_links.dimensions(&key) {
                    result.insert("width".into(), width.into());
                    result.insert("height".into(), height.into());
                }
                Ok(result.into())
            },
        );
        tera.register_filter(
            "markdown",
            move |input: &str, _: tera::Kwargs, _: &tera::State| -> tera::TeraResult<tera::Value> {
                markdown::render(
                    input,
                    &markdown_renderer_for_filter,
                    Some(&markdown_asset_links),
                )
                .map(|output| tera::Value::safe_string(&output))
                .map_err(|error| tera::Error::message(error.to_string()))
            },
        );
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
            span: named_reference_span(source_code.inner(), reference),
        })))
    }
}

fn named_reference_span(source: &str, reference: &str) -> miette::SourceSpan {
    source
        .find(reference)
        .map(|offset| (offset, reference.len()).into())
        .unwrap_or_else(|| (0, 0).into())
}

fn page_dir(state: &tera::State) -> tera::TeraResult<String> {
    Ok(state
        .get::<tera::Value>("page")?
        .and_then(|page| {
            page.get_from_path("dir")
                .and_then(tera::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default())
}
