//! Discovery of the explicit locale-owned content trees.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use walkdir::DirEntry;

use crate::{
    config::{
        CollectionsConfig, EditorialTimeZone, I18nConfig, MarkdownConfig, SeriesConfig,
        TaxonomiesConfig,
    },
    locale::Locale,
    localization::{Arguments, Localization},
};

use super::{
    Error, PublicationMode, index, loader,
    page::{Page, PageId, Task},
};

/// Locale-owned source inputs, loaded before cross-locale relationships are
/// resolved. Keeping this filesystem stage separate lets `Site` remain the
/// semantic and render orchestration boundary.
pub(super) struct LocalizedContent {
    pub(super) pages: BTreeMap<Locale, BTreeMap<PageId, Page>>,
    pub(super) excluded_drafts: BTreeMap<Locale, BTreeMap<PageId, PathBuf>>,
    pub(super) siblings: BTreeMap<Locale, index::SiblingIndex>,
    pub(super) tasks: Vec<Task>,
    pub(super) entries: BTreeMap<Locale, Vec<DirEntry>>,
    pub(super) content_inputs: BTreeSet<PathBuf>,
    pub(super) shared_root: PathBuf,
    pub(super) shared_entries: Vec<DirEntry>,
}

/// Inputs for locale-owned source discovery. Grouping the configuration that
/// controls page parsing keeps this filesystem stage from accumulating a long
/// positional argument list as checked relationships grow.
pub(super) struct Inputs<'a> {
    pub(super) content_root: &'a Path,
    pub(super) diagnostics: &'a crate::source_diagnostic::DiagnosticPaths,
    pub(super) i18n: &'a I18nConfig,
    pub(super) collections: &'a CollectionsConfig,
    pub(super) series: Option<&'a SeriesConfig>,
    pub(super) taxonomies: &'a TaxonomiesConfig,
    pub(super) time_zone: Option<&'a EditorialTimeZone>,
    pub(super) sitemap_enabled: bool,
    pub(super) markdown: &'a MarkdownConfig,
    pub(super) localization: &'a Localization,
    pub(super) publication: PublicationMode,
    pub(super) citation_inputs: &'a mut super::citation_inputs::InputCache,
}

pub(super) fn load(inputs: Inputs<'_>) -> Result<LocalizedContent, Error> {
    let Inputs {
        content_root,
        diagnostics,
        i18n,
        collections,
        series,
        taxonomies,
        time_zone,
        sitemap_enabled,
        markdown,
        localization,
        publication,
        citation_inputs,
    } = inputs;
    validate_root(content_root, i18n)?;
    let shared_root = content_root.join("assets");
    let shared_entries = if directory_exists(&shared_root)? {
        loader::content_entries(&shared_root)?
    } else {
        Vec::new()
    };
    let mut pages = BTreeMap::new();
    let mut excluded_drafts = BTreeMap::new();
    let mut siblings = BTreeMap::new();
    let mut tasks = Vec::new();
    let mut entries_by_locale = BTreeMap::new();
    let mut content_inputs = BTreeSet::new();
    let mut draft_uses_margin_footnotes = false;

    for locale in i18n.locales() {
        let root = content_root.join(locale.to_string());
        if !directory_exists(&root)? {
            return Err(Error::MissingLocaleContent {
                locale: locale.clone(),
                path: root,
            });
        }
        let entries = loader::content_entries(&root)?;
        let descriptors =
            loader::load_descriptors(&root, &entries, collections, sitemap_enabled, diagnostics)?;
        let subdirectories = loader::subdirectories(&entries);
        let output_prefix = if locale == i18n.default_locale() {
            PathBuf::new()
        } else {
            PathBuf::from(locale.to_string())
        };
        let mut locale_pages = BTreeMap::new();
        let mut locale_drafts = BTreeMap::new();
        let mut locale_inputs = BTreeSet::new();
        for entry in &entries {
            if !entry.file_type().is_file()
                || entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "page")
            {
                continue;
            }
            let source = entry.path();
            let relative = loader::relative_path(&root, source)?;
            let probe = loader::probe_page(source, &root, content_root, diagnostics)?;
            let draft = probe.is_draft();
            if draft && !publication.includes_drafts() {
                let id = PageId::from_logical_path(
                    crate::source_path::LogicalPath::from_path(&relative.with_extension(""))
                        .expect("discovered page IDs remain logical"),
                );
                // This path crosses the loading boundary only to explain a
                // later authored page reference. Keep the diagnostic name
                // here, rather than leaking the loader's physical path from
                // the resolved link catalog.
                locale_drafts.insert(id, diagnostics.display_path(source));
                locale_inputs.extend(probe.content_inputs().iter().cloned());
                draft_uses_margin_footnotes |=
                    loader::draft_uses_margin_footnotes(&probe, &relative, &descriptors);
                continue;
            }
            let loaded = loader::load_page(
                source,
                &relative,
                probe.into_text(),
                draft,
                loader::PageLoadContext {
                    project_content_root: content_root,
                    citation_inputs,
                    content_root: &root,
                    diagnostics,
                    locale,
                    output_prefix: &output_prefix,
                    subdirectories: &subdirectories,
                    descriptors: &descriptors,
                    collections,
                    series,
                    taxonomies,
                    time_zone,
                    sitemap_enabled,
                    markdown,
                },
            )?;
            if let Some(task) = loaded.task {
                tasks.push(task);
            }
            locale_inputs.extend(loaded.content_inputs);
            locale_pages.insert(loaded.page.id.clone(), loaded.page);
        }
        siblings.insert(locale.clone(), index::resolve_siblings(&locale_pages));
        pages.insert(locale.clone(), locale_pages);
        excluded_drafts.insert(locale.clone(), locale_drafts);
        entries_by_locale.insert(locale.clone(), entries);
        content_inputs.extend(locale_inputs);
    }
    validate_markdown_messages(
        &pages,
        markdown,
        localization,
        i18n,
        draft_uses_margin_footnotes,
    )?;
    tasks.sort_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then(left.page_id.cmp(&right.page_id))
    });
    Ok(LocalizedContent {
        pages,
        excluded_drafts,
        siblings,
        tasks,
        entries: entries_by_locale,
        content_inputs,
        shared_root,
        shared_entries,
    })
}

fn validate_markdown_messages(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    markdown: &MarkdownConfig,
    localization: &Localization,
    i18n: &I18nConfig,
    draft_uses_margin_footnotes: bool,
) -> Result<(), Error> {
    validate_margin_footnote_messages(pages, markdown, localization, draft_uses_margin_footnotes)?;
    validate_table_scroll_messages(markdown, localization, i18n)
}

fn validate_margin_footnote_messages(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    markdown: &MarkdownConfig,
    localization: &Localization,
    draft_uses_margin_footnotes: bool,
) -> Result<(), Error> {
    let Some(config) = markdown.margin_footnotes() else {
        return Ok(());
    };
    let Some((source, span)) = config.marker_message_origin() else {
        return Ok(());
    };
    let effective_locales = pages
        .iter()
        .filter_map(|(locale, locale_pages)| {
            let count = locale_pages
                .values()
                .filter_map(|page| page.footnotes.as_ref())
                .map(crate::markdown::MarginFootnotePlan::number_count)
                .max()?;
            Some((locale, count.max(1)))
        })
        .collect::<BTreeMap<_, _>>();
    if effective_locales.is_empty() && !draft_uses_margin_footnotes {
        return Err(Error::MarkdownMessageConfiguration(Box::new(
            loader::MarkdownMessageConfigurationDiagnostic::unused(
                source.clone(),
                span,
                "margin-footnote marker-message",
                "remove [markdown.margin_footnotes], or select margin footnotes for a rendered page"
                    .into(),
            ),
        )));
    }
    for (locale, highest_number) in effective_locales {
        for number in 1..=highest_number {
            let mut arguments = Arguments::new();
            arguments.set("number", number as i64);
            let formatted = localization.format(locale, config.marker_message(), None, &arguments);
            let problem = match formatted {
                Ok(value) if value.trim().is_empty() => Some(format!(
                    "message formats to empty text for footnote {number}"
                )),
                Ok(_) => None,
                Err(message) => Some(message),
            };
            if let Some(message) = problem {
                return Err(Error::MarkdownMessageConfiguration(Box::new(
                    loader::MarkdownMessageConfigurationDiagnostic::localization(
                        source.clone(),
                        span,
                        "margin-footnote marker-message",
                        locale,
                        message,
                        "define a non-empty message that accepts $number".into(),
                        localization.message_origin(locale, config.marker_message()),
                    ),
                )));
            }
        }
    }
    Ok(())
}

fn validate_table_scroll_messages(
    markdown: &MarkdownConfig,
    localization: &Localization,
    i18n: &I18nConfig,
) -> Result<(), Error> {
    let Some(config) = markdown.tables() else {
        return Ok(());
    };
    let Some((source, span)) = config.scroll_message_origin() else {
        return Ok(());
    };
    for locale in i18n.locales() {
        let formatted =
            localization.format(locale, config.scroll_message(), None, &Arguments::new());
        let problem = match formatted {
            Ok(value) if value.trim().is_empty() => Some("message formats to empty text".into()),
            Ok(_) => None,
            Err(message) => Some(message),
        };
        if let Some(message) = problem {
            return Err(Error::MarkdownMessageConfiguration(Box::new(
                loader::MarkdownMessageConfigurationDiagnostic::localization(
                    source.clone(),
                    span,
                    "table-scroll message",
                    locale,
                    message,
                    "define a non-empty message".into(),
                    localization.message_origin(locale, config.scroll_message()),
                ),
            )));
        }
    }
    Ok(())
}

fn validate_root(content_root: &Path, i18n: &I18nConfig) -> Result<(), Error> {
    let mut entries = std::fs::read_dir(content_root)
        .map_err(|source| Error::Io {
            path: content_root.into(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: content_root.into(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if name == "assets"
            || i18n
                .locales()
                .any(|locale| name == std::ffi::OsStr::new(&locale.to_string()))
        {
            continue;
        }
        return Err(Error::UnexpectedContentRoot { path: entry.path() });
    }
    Ok(())
}

/// Inspects a declared content directory without following symbolic links.
/// Missing shared assets are allowed; callers decide whether a missing locale
/// directory is an error.
fn directory_exists(path: &Path) -> Result<bool, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::ContentSymlink { path: path.into() })
        }
        Ok(metadata) if !metadata.is_dir() => Err(Error::ContentNotDirectory { path: path.into() }),
        Ok(_) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            path: path.into(),
            source,
        }),
    }
}
