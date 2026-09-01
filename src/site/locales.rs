//! Discovery of the explicit locale-owned content trees.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use walkdir::DirEntry;

use crate::{
    config::{CollectionsConfig, EditorialTimeZone, I18nConfig, MarkdownConfig, TaxonomiesConfig},
    locale::Locale,
};

use super::{
    Error, index, loader,
    page::{Page, PageId, Task},
};

/// Locale-owned source inputs, loaded before cross-locale relationships are
/// resolved. Keeping this filesystem stage separate lets `Site` remain the
/// semantic and render orchestration boundary.
pub(super) struct LocalizedContent {
    pub(super) pages: BTreeMap<Locale, BTreeMap<PageId, Page>>,
    pub(super) siblings: BTreeMap<Locale, index::SiblingIndex>,
    pub(super) tasks: Vec<Task>,
    pub(super) entries: BTreeMap<Locale, Vec<DirEntry>>,
    pub(super) content_inputs: BTreeMap<Locale, BTreeSet<PathBuf>>,
    pub(super) shared_root: PathBuf,
    pub(super) shared_entries: Vec<DirEntry>,
}

pub(super) fn load(
    content_root: &Path,
    i18n: &I18nConfig,
    collections: &CollectionsConfig,
    taxonomies: &TaxonomiesConfig,
    time_zone: Option<&EditorialTimeZone>,
    sitemap_enabled: bool,
    markdown: &MarkdownConfig,
) -> Result<LocalizedContent, Error> {
    validate_root(content_root, i18n)?;
    let shared_root = content_root.join("assets");
    let shared_entries = if directory_exists(&shared_root)? {
        loader::content_entries(&shared_root)?
    } else {
        Vec::new()
    };
    let mut pages = BTreeMap::new();
    let mut siblings = BTreeMap::new();
    let mut tasks = Vec::new();
    let mut entries_by_locale = BTreeMap::new();
    let mut content_inputs = BTreeMap::new();

    for locale in i18n.locales() {
        let root = content_root.join(locale.to_string());
        if !directory_exists(&root)? {
            return Err(Error::MissingLocaleContent {
                locale: locale.clone(),
                path: root,
            });
        }
        let entries = loader::content_entries(&root)?;
        let descriptors = loader::load_descriptors(&root, &entries, collections, sitemap_enabled)?;
        let subdirectories = loader::subdirectories(&entries);
        let output_prefix = if locale == i18n.default_locale() {
            PathBuf::new()
        } else {
            PathBuf::from(locale.to_string())
        };
        let mut locale_pages = BTreeMap::new();
        let mut locale_inputs = BTreeSet::new();
        for entry in &entries {
            if !entry.file_type().is_file()
                || !entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "page")
            {
                continue;
            }
            let source = entry.path();
            let relative = loader::relative_path(&root, source)?;
            let loaded = loader::load_page(
                source,
                &relative,
                loader::PageLoadContext {
                    content_root: &root,
                    locale,
                    output_prefix: &output_prefix,
                    subdirectories: &subdirectories,
                    descriptors: &descriptors,
                    collections,
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
        entries_by_locale.insert(locale.clone(), entries);
        content_inputs.insert(locale.clone(), locale_inputs);
    }
    tasks.sort_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then(left.page_id.cmp(&right.page_id))
    });
    Ok(LocalizedContent {
        pages,
        siblings,
        tasks,
        entries: entries_by_locale,
        content_inputs,
        shared_root,
        shared_entries,
    })
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
    entries.sort_by_key(|entry| entry.file_name());
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
