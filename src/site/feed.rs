//! Collection-derived Atom 1.0 feeds and their template lookup catalog.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    path::PathBuf,
    sync::Arc,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use crate::{
    author::AuthorName,
    config::{BasePath, CanonicalUrls, FeedDefinition, FeedName, FeedsConfig},
    locale::Locale,
    localization::{Arguments, Localization},
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
};

use super::{
    PageId,
    collection::ResolvedCollection,
    page::{Page, PublicationTime},
};

#[derive(Debug)]
pub(crate) struct Catalog {
    locales: BTreeMap<Locale, BTreeMap<String, Feed>>,
}

#[derive(Debug, Clone)]
struct Feed {
    name: String,
    title: String,
    path: OutputPath,
    xml: String,
    claim: OutputClaim,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct View {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) absolute_url: Option<String>,
    pub(crate) relative_url: String,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("[feeds] requires site.origin so Heine can generate canonical Atom IDs and links")]
    #[diagnostic(code(heine::feed::missing_origin))]
    MissingOrigin,
    #[error("feed {feed:?} has an invalid Fluent {field}")]
    #[diagnostic(code(heine::feed::localization))]
    Localization {
        feed: String,
        field: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this message identifier could not be resolved")]
        span: SourceSpan,
        #[help]
        help: String,
    },
    #[error("feed {feed:?} includes page {page:?}, which has no {field}")]
    #[diagnostic(code(heine::feed::missing_entry_metadata))]
    MissingEntryMetadata {
        feed: String,
        page: String,
        field: &'static str,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this collection membership supplies the feed entry")]
        span: SourceSpan,
        #[help]
        help: String,
    },
    #[error("feed {feed:?} is empty for locale {locale} and has no {field}")]
    #[diagnostic(code(heine::feed::missing_empty_metadata))]
    MissingEmptyMetadata {
        feed: String,
        locale: Locale,
        field: &'static str,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("feed configured here")]
        span: SourceSpan,
        #[help]
        help: String,
    },
    #[error("feed {feed:?} cannot encode {field} as XML text: {character} is forbidden")]
    #[diagnostic(code(heine::feed::invalid_xml_text))]
    InvalidXmlText {
        feed: String,
        field: String,
        character: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this selects text for the Atom feed")]
        span: SourceSpan,
        #[help]
        help: String,
    },
}

impl Catalog {
    pub(super) fn resolve(inputs: Inputs<'_>) -> Result<Self, Error> {
        if inputs.config.definitions().is_empty() {
            return Ok(Self {
                locales: BTreeMap::new(),
            });
        }
        let urls = inputs.urls.ok_or(Error::MissingOrigin)?;
        let mut locales = BTreeMap::new();
        for (locale, locale_pages) in inputs.pages {
            let mut feeds = BTreeMap::new();
            for (name, definition) in inputs.config.definitions() {
                let path =
                    OutputPath::try_new(inputs.prefixes[locale].join(definition.path().as_path()))
                        .expect("locale prefix and checked feed path remain a checked output path");
                let title = message(
                    inputs.localization,
                    locale,
                    name,
                    definition,
                    "title_key",
                    definition.title_key(),
                    definition.title_key_origin(),
                )?;
                check_config_xml_text(name, "title_key", &title, definition.title_key_origin())?;
                let subtitle = definition
                    .subtitle_key()
                    .map(|key| {
                        message(
                            inputs.localization,
                            locale,
                            name,
                            definition,
                            "subtitle_key",
                            key,
                            definition.subtitle_key_origin(),
                        )
                    })
                    .transpose()?;
                if let Some(subtitle) = &subtitle {
                    check_config_xml_text(
                        name,
                        "subtitle_key",
                        subtitle,
                        definition.subtitle_key_origin(),
                    )?;
                }
                let collection = &inputs.collections[locale][definition.collection()];
                let mut selected =
                    BinaryHeap::with_capacity(collection.page_ids().len().min(definition.limit()));
                for id in collection.page_ids() {
                    selected.push(RankedEntry(entry(
                        name,
                        definition,
                        id,
                        &locale_pages[id],
                        urls,
                    )?));
                    if selected.len() > definition.limit() {
                        selected.pop();
                    }
                }
                let mut entries = selected
                    .into_iter()
                    .map(|entry| entry.0)
                    .collect::<Vec<_>>();
                entries.sort_by(entry_order);
                let (updated, empty_authors) = if let Some(updated) =
                    entries.iter().map(|entry| entry.updated.timestamp()).max()
                {
                    (updated, None)
                } else {
                    let updated = definition
                        .empty_updated()
                        .ok_or_else(|| empty_error(name, locale, definition, "empty_updated"))?;
                    let authors = definition
                        .empty_authors()
                        .ok_or_else(|| empty_error(name, locale, definition, "empty_authors"))?;
                    for author in authors {
                        check_config_xml_text(
                            name,
                            "empty_authors",
                            author.as_str(),
                            definition.empty_authors_origin(),
                        )?;
                    }
                    (updated, Some(authors))
                };
                let url = urls.output_url(&path);
                let xml = atom(
                    &locale.to_string(),
                    &title,
                    subtitle.as_deref(),
                    &url,
                    updated,
                    empty_authors,
                    &entries,
                );
                let claim = output_claim(name, definition, path.clone());
                feeds.insert(
                    name.to_string(),
                    Feed {
                        name: name.to_string(),
                        title,
                        path,
                        xml,
                        claim,
                    },
                );
            }
            locales.insert(locale.clone(), feeds);
        }
        Ok(Self { locales })
    }

    pub(crate) fn output_claims(&self) -> Vec<OutputClaim> {
        self.locales
            .values()
            .flat_map(|feeds| feeds.values().map(|feed| feed.claim.clone()))
            .collect()
    }

    pub(crate) fn output_count(&self) -> usize {
        self.locales.values().map(BTreeMap::len).sum()
    }

    pub(crate) fn generated(&self, plan: &OutputPlan) -> Vec<GeneratedFile> {
        self.locales
            .values()
            .flat_map(|feeds| feeds.values())
            .map(|feed| {
                GeneratedFile::text(
                    plan.planned_path(&feed.path)
                        .expect("feed output paths were checked before rendering"),
                    feed.xml.clone(),
                )
            })
            .collect()
    }

    pub(crate) fn view(
        &self,
        locale: &Locale,
        name: &str,
        root: &str,
        base_path: &BasePath,
        canonical_urls: Option<&CanonicalUrls>,
    ) -> Option<View> {
        let feed = self.locales.get(locale)?.get(name)?;
        Some(View {
            name: feed.name.clone(),
            title: feed.title.clone(),
            url: base_path.join(&feed.path.url_path()),
            absolute_url: canonical_urls.map(|urls| urls.output_url(&feed.path)),
            relative_url: format!("{root}{}", feed.path.url_path().as_str()),
        })
    }

    pub(crate) fn names(&self, locale: &Locale) -> Vec<String> {
        self.locales
            .get(locale)
            .map(|feeds| feeds.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// Inputs assembled after all page relationships have resolved.
pub(super) struct Inputs<'a> {
    pub(super) pages: &'a BTreeMap<Locale, BTreeMap<PageId, Page>>,
    pub(super) collections:
        &'a BTreeMap<Locale, BTreeMap<crate::config::CollectionName, ResolvedCollection>>,
    pub(super) config: &'a FeedsConfig,
    pub(super) prefixes: &'a BTreeMap<Locale, PathBuf>,
    pub(super) urls: Option<&'a CanonicalUrls>,
    pub(super) localization: &'a Localization,
}

#[derive(Debug)]
struct Entry {
    page: PageId,
    title: String,
    summary: String,
    authors: Vec<AuthorName>,
    published: PublicationTime,
    updated: PublicationTime,
    url: String,
}

/// A max-heap keeps the least recent retained entry at its head. Feed limits
/// are normally small, so resolution retains only entries that can reach the
/// published document instead of materializing an entire large collection.
#[derive(Debug)]
struct RankedEntry(Entry);

impl Ord for RankedEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .published
            .cmp(&self.0.published)
            .then(self.0.page.cmp(&other.0.page))
    }
}

impl PartialOrd for RankedEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RankedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0.published == other.0.published && self.0.page == other.0.page
    }
}

impl Eq for RankedEntry {}

fn entry_order(left: &Entry, right: &Entry) -> Ordering {
    right
        .published
        .cmp(&left.published)
        .then(left.page.cmp(&right.page))
}

fn entry(
    feed: &FeedName,
    definition: &FeedDefinition,
    id: &PageId,
    page: &Page,
    urls: &CanonicalUrls,
) -> Result<Entry, Error> {
    let membership = page
        .collections
        .iter()
        .find(|membership| membership.name == *definition.collection())
        .expect("resolved feed collection members retain their membership");
    let missing = |field| Error::MissingEntryMetadata {
        feed: feed.to_string(),
        page: id.to_string(),
        field,
        source_code: membership.source.clone(),
        span: membership.span,
        help: format!(
            "add `{field}` to page {id:?}, or remove it from collection {:?}",
            definition.collection().to_string()
        ),
    };
    let title = page
        .metadata
        .title
        .clone()
        .ok_or_else(|| missing("title"))?;
    let page_source = Arc::new(page.source.clone());
    check_entry_xml_text(
        feed,
        definition,
        id,
        "title",
        &title,
        &page_source,
        page.metadata.title_span.unwrap_or(membership.span),
    )?;
    let summary = page
        .metadata
        .summary
        .clone()
        .ok_or_else(|| missing("summary"))?;
    check_entry_xml_text(
        feed,
        definition,
        id,
        "summary",
        &summary,
        &page_source,
        page.metadata.summary_span.unwrap_or(membership.span),
    )?;
    let authors = page
        .metadata
        .authors
        .clone()
        .ok_or_else(|| missing("authors"))?;
    let (authors_source, authors_span) = authors.origin(&page.source);
    for author in authors.names() {
        check_entry_xml_text(
            feed,
            definition,
            id,
            "authors",
            author.as_str(),
            &authors_source,
            authors_span,
        )?;
    }
    let published = page
        .metadata
        .published
        .ok_or_else(|| missing("published"))?;
    let path = page
        .output_path
        .as_ref()
        .expect("collection only contains rendered pages");
    Ok(Entry {
        page: id.clone(),
        title,
        summary,
        authors: authors.names().to_vec(),
        published,
        updated: page.metadata.updated.unwrap_or(published),
        url: urls.output_url(path),
    })
}

fn message(
    localization: &Localization,
    locale: &Locale,
    feed: &FeedName,
    _definition: &FeedDefinition,
    field: &str,
    key: &str,
    origin: Option<(Arc<NamedSource<String>>, SourceSpan)>,
) -> Result<String, Error> {
    localization
        .format(locale, key, None, &Arguments::new())
        .map_err(|help| {
            let (source, span) =
                origin.expect("feed configuration sources are attached before resolution");
            Error::Localization {
                feed: feed.to_string(),
                field: field.into(),
                source_code: source,
                span,
                help,
            }
        })
}

fn empty_error(
    feed: &FeedName,
    locale: &Locale,
    definition: &FeedDefinition,
    field: &'static str,
) -> Error {
    let (source, span) = definition
        .declaration_origin()
        .expect("feed configuration sources are attached before resolution");
    Error::MissingEmptyMetadata {
        feed: feed.to_string(),
        locale: locale.clone(),
        field,
        source_code: source,
        span,
        help: format!(
            "add `{field}` to [feeds.{feed}] so an empty {locale} feed has complete Atom metadata"
        ),
    }
}

fn check_config_xml_text(
    feed: &FeedName,
    field: &str,
    value: &str,
    origin: Option<(Arc<NamedSource<String>>, SourceSpan)>,
) -> Result<(), Error> {
    let Some(character) = forbidden_xml_character(value) else {
        return Ok(());
    };
    let (source_code, span) =
        origin.expect("feed configuration sources are attached before resolution");
    let character = code_point(character);
    Err(Error::InvalidXmlText {
        feed: feed.to_string(),
        field: field.into(),
        character: character.clone(),
        source_code,
        span,
        help: format!(
            "remove {character} from feed {feed:?} {field}; Atom 1.0 output uses XML 1.0 text"
        ),
    })
}

fn check_entry_xml_text(
    feed: &FeedName,
    definition: &FeedDefinition,
    id: &PageId,
    field: &str,
    value: &str,
    source_code: &Arc<NamedSource<String>>,
    span: SourceSpan,
) -> Result<(), Error> {
    let Some(character) = forbidden_xml_character(value) else {
        return Ok(());
    };
    let character = code_point(character);
    Err(Error::InvalidXmlText {
        feed: feed.to_string(),
        field: format!("page {id:?} {field}"),
        character: character.clone(),
        source_code: source_code.clone(),
        span,
        help: format!(
            "remove {character} from page {id:?} {field}, or remove the page from collection {:?}",
            definition.collection().to_string()
        ),
    })
}

fn forbidden_xml_character(value: &str) -> Option<char> {
    value.chars().find(|character| {
        !matches!(character, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
    })
}

fn code_point(character: char) -> String {
    format!("U+{:04X}", character as u32)
}

fn output_claim(name: &FeedName, definition: &FeedDefinition, path: OutputPath) -> OutputClaim {
    let (source, span) = definition
        .path_origin()
        .expect("feed configuration sources are attached before resolution");
    OutputClaim::authored(path, format!("Atom feed {name}"), source, span)
}

fn atom(
    locale: &str,
    title: &str,
    subtitle: Option<&str>,
    url: &str,
    updated: jiff::Timestamp,
    empty_authors: Option<&[AuthorName]>,
    entries: &[Entry],
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:lang=\"");
    push_escaped(&mut xml, locale);
    xml.push_str("\">\n  <title type=\"text\">");
    push_escaped(&mut xml, title);
    xml.push_str("</title>\n");
    if let Some(subtitle) = subtitle {
        xml.push_str("  <subtitle type=\"text\">");
        push_escaped(&mut xml, subtitle);
        xml.push_str("</subtitle>\n");
    }
    xml.push_str("  <id>");
    push_escaped(&mut xml, url);
    xml.push_str("</id>\n  <link rel=\"self\" type=\"application/atom+xml\" href=\"");
    push_escaped(&mut xml, url);
    xml.push_str("\"/>\n  <updated>");
    xml.push_str(&updated.to_string());
    xml.push_str("</updated>\n");
    if let Some(authors) = empty_authors {
        push_authors(&mut xml, authors, "  ");
    }
    for entry in entries {
        xml.push_str("  <entry>\n    <title type=\"text\">");
        push_escaped(&mut xml, &entry.title);
        xml.push_str("</title>\n    <id>");
        push_escaped(&mut xml, &entry.url);
        xml.push_str("</id>\n    <link rel=\"alternate\" href=\"");
        push_escaped(&mut xml, &entry.url);
        xml.push_str("\"/>\n    <published>");
        xml.push_str(&entry.published.render());
        xml.push_str("</published>\n    <updated>");
        xml.push_str(&entry.updated.render());
        xml.push_str("</updated>\n");
        push_authors(&mut xml, &entry.authors, "    ");
        xml.push_str("    <summary type=\"text\">");
        push_escaped(&mut xml, &entry.summary);
        xml.push_str("</summary>\n  </entry>\n");
    }
    xml.push_str("</feed>\n");
    xml
}

fn push_authors(xml: &mut String, authors: &[AuthorName], indent: &str) {
    for author in authors {
        xml.push_str(indent);
        xml.push_str("<author><name>");
        push_escaped(xml, author.as_str());
        xml.push_str("</name></author>\n");
    }
}

fn push_escaped(xml: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '"' => xml.push_str("&quot;"),
            '\'' => xml.push_str("&apos;"),
            _ => xml.push(character),
        }
    }
}
