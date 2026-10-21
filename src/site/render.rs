//! Template-engine setup and rendering of resolved site tasks.
//!
//! This module owns the render-facing boundary. The parent module retains
//! resolved-site construction and the stable `Site` model.

use std::{collections::BTreeSet, path::Path, time::SystemTime};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error as ThisError;

use crate::{
    markdown::Renderer as MarkdownRenderer,
    output::{GeneratedFile, OutputPath},
    template::{
        KitView, LocaleView, RenderContext, RuntimeContext, TemplateEngine, TemplateIndexes,
        TemplateRelationships, TemplateResources,
    },
};

use super::{BuildMode, BuildOptions, Error, Site, page::Task, sitemap};

/// A semantic error tied to the precise source range that referenced a
/// template. It is boxed by the site error to keep the common error path
/// compact.
#[derive(Debug, Diagnostic, ThisError)]
#[error("template {template:?} was not found")]
#[diagnostic(code(heine::template::not_found))]
pub(crate) struct MissingTemplate {
    template: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("template referenced here")]
    span: SourceSpan,
    #[help]
    help: String,
}

impl Site {
    pub(crate) fn render(
        &self,
        templates: &Path,
        selected_kit: Option<&crate::kit::Selected>,
        existing_output: &Path,
        options: &BuildOptions,
        markdown_renderer: &MarkdownRenderer,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) -> Result<Vec<GeneratedFile>, Error> {
        let base_path = options.config().site().base_path().to_owned();
        let canonical_urls = options.config().site().canonical_urls();
        let integrities = self
            .assets
            .integrity_values(options.config().assets().integrity())?;
        let resources = TemplateResources::new(
            self.assets.links(
                self.i18n.default_locale().clone(),
                base_path.clone(),
                canonical_urls.clone(),
                integrities,
            ),
            self.renderer_attributions(),
            TemplateRelationships::new(
                self.pages.clone(),
                self.i18n.default_locale().clone(),
                base_path.clone(),
                canonical_urls.clone(),
                TemplateIndexes {
                    collections: self.collections.clone(),
                    siblings: self.siblings.clone(),
                    collection_neighbors: self.collection_neighbors.clone(),
                    taxonomies: self.taxonomies.clone(),
                    pagers: self.pagers.clone(),
                    feeds: self.feeds.clone(),
                    search: self.search.clone(),
                    series: self.series.clone(),
                },
            ),
            self.localization.clone(),
            markdown_renderer
                .stylesheet_path()
                .map(|path| base_path.join(&path.url_path())),
            options
                .config()
                .markdown()
                .tables()
                .map(|tables| tables.scroll_message().to_owned()),
            options.config().site().time_zone().cloned(),
        );
        let engine = TemplateEngine::load(
            templates,
            selected_kit,
            resources,
            markdown_renderer.clone(),
            diagnostics,
        )?;
        self.validate_templates(&engine)?;
        let kit = selected_kit.map(KitView::from);
        // Every known render task produces at most one file. Reserve the
        // structural outputs too; optional generated resources grow this only
        // by a small, bounded amount.
        let generated_capacity = usize::from(markdown_renderer.stylesheet().is_some())
            + self
                .sitemap
                .as_ref()
                .map_or(0, sitemap::Sitemap::output_count)
            + self.feeds.output_count()
            + self.search.output_count();
        let mut rendered = Vec::with_capacity(
            self.tasks.len()
                + self.taxonomy_tasks.len()
                + self.series.tasks().len()
                + generated_capacity,
        );
        for task in &self.tasks {
            if !needs_render(task, existing_output, options.mode())? {
                continue;
            }
            let page = self
                .pages
                .view(&task.locale, &task.page_id)
                .expect("tasks refer to loaded pages");
            let page_dir = page.directory();
            let root = root_for(task.output_path.as_path());
            let render_page = crate::template::RenderedPage {
                page,
                taxonomies: self.taxonomies.page_terms(
                    &task.locale,
                    &task.page_id,
                    &base_path,
                    canonical_urls.as_ref(),
                    &root,
                ),
                series: self.series.page_memberships(
                    &task.locale,
                    &task.page_id,
                    &self.pages,
                    &base_path,
                    canonical_urls.as_ref(),
                    &root,
                ),
            };
            let pagination = task.pagination.as_ref().map(|pager| {
                pager.view(
                    &self.pages,
                    &task.locale,
                    &base_path,
                    canonical_urls.as_ref(),
                    &root,
                )
            });
            let rendering = task.pagination.as_ref().map_or_else(
                || {
                    format!(
                        "page {} ({}) to {}",
                        task.page_id,
                        task.locale,
                        task.output_path.as_path().display()
                    )
                },
                |pager| {
                    format!(
                        "pagination page {} of listing {} ({}) to {}",
                        pager.page_number,
                        task.page_id,
                        task.locale,
                        task.output_path.as_path().display()
                    )
                },
            );
            let contents = engine.render(
                &task.template,
                RenderContext {
                    site: crate::template::SiteView::from_canonical_urls(canonical_urls.as_ref()),
                    kit: kit.as_ref(),
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: Some(render_page),
                    taxonomy: None,
                    series: None,
                    pagination,
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir,
                        page_id: Some(&task.page_id),
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &rendering,
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("page task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        for task in &self.taxonomy_tasks {
            if !needs_render_generated(
                task.latest_input,
                &task.output_path,
                existing_output,
                options.mode(),
            )? {
                continue;
            }
            let root = root_for(task.output_path.as_path());
            let pagination = task.pagination.as_ref().map(|pager| {
                pager.view(
                    &self.pages,
                    &task.locale,
                    &base_path,
                    canonical_urls.as_ref(),
                    &root,
                )
            });
            let contents = engine.render(
                &task.template,
                RenderContext {
                    site: crate::template::SiteView::from_canonical_urls(canonical_urls.as_ref()),
                    kit: kit.as_ref(),
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: None,
                    taxonomy: Some(task.taxonomy_view(&base_path, canonical_urls.as_ref(), &root)),
                    series: None,
                    pagination,
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir: task
                            .output_path
                            .as_path()
                            .parent()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                        page_id: None,
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &format!(
                    "taxonomy {} ({}) to {}",
                    task.taxonomy.name,
                    task.locale,
                    task.output_path.as_path().display()
                ),
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("taxonomy task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        for task in self.series.tasks() {
            if !needs_render_generated(
                task.latest_input,
                &task.output_path,
                existing_output,
                options.mode(),
            )? {
                continue;
            }
            let root = root_for(task.output_path.as_path());
            let series = match &task.series_id {
                Some(id) => {
                    self.series
                        .view(&task.locale, id, &base_path, canonical_urls.as_ref(), &root)
                }
                None => self
                    .series
                    .index(&task.locale, &base_path, canonical_urls.as_ref(), &root),
            }
            .expect("resolved Series task keeps its corresponding view");
            let contents = engine.render(
                &task.template,
                RenderContext {
                    site: crate::template::SiteView::from_canonical_urls(canonical_urls.as_ref()),
                    kit: kit.as_ref(),
                    locale: LocaleView {
                        code: task.locale.to_string(),
                        direction: self.i18n.direction(&task.locale).as_str().into(),
                    },
                    page: None,
                    taxonomy: None,
                    series: Some(series),
                    pagination: task.pagination.as_ref().map(|pager| {
                        pager.view(
                            &self.pages,
                            &task.locale,
                            &base_path,
                            canonical_urls.as_ref(),
                            &root,
                        )
                    }),
                    data: self.data.value(),
                    locale_data: self.locale_data[&task.locale].value(),
                    root: &root,
                    runtime: RuntimeContext {
                        locale: task.locale.to_string(),
                        page_dir: task
                            .output_path
                            .as_path()
                            .parent()
                            .and_then(|path| path.to_str())
                            .unwrap_or(""),
                        page_id: None,
                        output_path: task
                            .output_path
                            .as_path()
                            .to_str()
                            .expect("validated output paths are UTF-8"),
                        root: &root,
                        pagination_page: task.pagination.as_ref().map(|pager| pager.page_number),
                    },
                },
                &format!(
                    "Series ({}) to {}",
                    task.locale,
                    task.output_path.as_path().display()
                ),
            )?;
            let path = self
                .output_plan
                .planned_path(&task.output_path)
                .expect("Series task output was validated in the output plan");
            rendered.push(GeneratedFile::text(path, contents));
        }
        if let (Some(stylesheet), Some(path)) = (
            markdown_renderer.stylesheet(),
            markdown_renderer.stylesheet_path(),
        ) {
            let path = self
                .output_plan
                .planned_path(path)
                .expect("highlighting stylesheet was validated in the output plan");
            rendered.push(GeneratedFile::text(path, stylesheet));
        }
        if options.mode() == BuildMode::Full
            && let Some(sitemap) = &self.sitemap
        {
            rendered.extend(sitemap.generated(&self.output_plan));
        }
        if options.mode() == BuildMode::Full {
            rendered.extend(self.feeds.generated(&self.output_plan));
            rendered.extend(self.search.generated(&self.output_plan));
        }
        for resource in &self.renderer_resources {
            let path = self
                .output_plan
                .planned_path(&resource.output_path)
                .expect("renderer resource output was validated in the output plan");
            rendered.push(GeneratedFile {
                path,
                bytes: resource.bytes.clone(),
            });
        }
        Ok(rendered)
    }

    fn validate_templates(&self, engine: &TemplateEngine) -> Result<(), Error> {
        // A template declaration is a site-wide fact. Rechecking a shared
        // template for every page only repeats work and cannot improve the
        // first-reference diagnostic.
        let mut checked = BTreeSet::new();
        for task in &self.tasks {
            if !checked.insert(task.template.clone()) {
                continue;
            }
            validate_template_reference(
                engine,
                &task.template,
                task.template_source.as_ref().clone(),
                task.template_span,
            )?;
        }
        for reference in self.taxonomies.template_references() {
            if !checked.insert(reference.template.clone()) {
                continue;
            }
            validate_template_reference(
                engine,
                &reference.template,
                reference.source.as_ref().clone(),
                reference.span,
            )?;
        }
        for reference in self.series.template_references() {
            if !checked.insert(reference.template.clone()) {
                continue;
            }
            validate_template_reference(
                engine,
                &reference.template,
                reference.source.as_ref().clone(),
                reference.span,
            )?;
        }
        Ok(())
    }
}

fn validate_template_reference(
    engine: &TemplateEngine,
    template: &crate::template::TemplateName,
    source_code: NamedSource<String>,
    span: SourceSpan,
) -> Result<(), Error> {
    if engine.contains(template) {
        return Ok(());
    }
    Err(Box::new(MissingTemplate {
        template: template.to_string(),
        source_code,
        span,
        help: format!("add {template} under templates/, or correct this reference"),
    })
    .into())
}

fn root_for(output: &Path) -> String {
    let levels = output
        .parent()
        .map(|parent| parent.components().count())
        .unwrap_or(0);
    if levels == 0 {
        "./".into()
    } else {
        "../".repeat(levels)
    }
}

fn needs_render(task: &Task, output_root: &Path, mode: BuildMode) -> Result<bool, Error> {
    needs_render_generated(task.latest_input, &task.output_path, output_root, mode)
}

fn needs_render_generated(
    latest_input: SystemTime,
    output_path: &OutputPath,
    output_root: &Path,
    mode: BuildMode,
) -> Result<bool, Error> {
    if mode == BuildMode::Full {
        return Ok(true);
    }
    let target = output_root.join(output_path.as_path());
    if !target.exists() {
        return Ok(true);
    }
    let modified = std::fs::metadata(&target)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    Ok(modified <= latest_input)
}
