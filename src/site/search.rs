//! Collection-derived, locale-local JSON search documents.
//!
//! Search owns stable source facts and generated output. Browser ranking and
//! presentation remain outside this module and outside Heine's template model.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    config::{BasePath, SearchConfig, SearchDefinition, SearchName, TaxonomiesConfig},
    locale::Locale,
    markdown,
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
};

use super::{PageId, collection::ResolvedCollection, page::Page};

/// Version of Heine's public search-document JSON contract.
///
/// Consumers use this to reject a document shape they do not understand.
const SEARCH_DOCUMENT_FORMAT: u8 = 3;

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    locales: BTreeMap<Locale, BTreeMap<String, Index>>,
}

#[derive(Debug)]
struct Index {
    name: String,
    path: OutputPath,
    json: String,
    claim: OutputClaim,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct View {
    pub(crate) name: String,
    pub(crate) count: usize,
    pub(crate) url: String,
    pub(crate) relative_url: String,
}

#[derive(Debug, Serialize)]
struct DocumentEnvelope<'a> {
    format: u8,
    locale: &'a str,
    documents: Vec<&'a Document>,
}

#[derive(Debug, Serialize)]
struct Document {
    id: String,
    url: String,
    draft: bool,
    title: Option<String>,
    description: Option<String>,
    summary: Option<String>,
    published: Option<String>,
    updated: Option<String>,
    taxonomies: BTreeMap<String, Vec<String>>,
    sections: Vec<markdown::SearchTextSection>,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not derive a versioned output path for search index {name:?}: {message}")]
    #[diagnostic(code(heine::search::output_path))]
    OutputPath {
        name: String,
        message: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this path cannot accommodate the versioned JSON filename")]
        span: SourceSpan,
    },
    #[error("could not serialize search index {name:?}: {source}")]
    Serialize {
        name: String,
        #[source]
        source: serde_json::Error,
    },
}

impl Catalog {
    pub(super) fn resolve(inputs: Inputs<'_>) -> Result<Self, Error> {
        if inputs.config.definitions().is_empty() {
            return Ok(Self::default());
        }
        let mut locales = BTreeMap::new();
        for (locale, pages) in inputs.pages {
            let selections = inputs
                .config
                .definitions()
                .iter()
                .map(|(name, definition)| {
                    (
                        name.clone(),
                        selected_ids_for(definition, inputs.collections, locale),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let selected = selections
                .values()
                .flat_map(|ids| ids.iter().cloned())
                .collect::<BTreeSet<_>>();
            let catalog = selected
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        document(
                            &pages[id],
                            inputs.base_path,
                            inputs.markdown,
                            inputs.taxonomies,
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let mut indexes = BTreeMap::new();
            for (name, definition) in inputs.config.definitions() {
                let selected = &selections[name];
                let documents = selected.iter().map(|id| &catalog[id]).collect::<Vec<_>>();
                let locale_text = locale.to_string();
                let json = serde_json::to_string(&DocumentEnvelope {
                    format: SEARCH_DOCUMENT_FORMAT,
                    locale: &locale_text,
                    documents,
                })
                .map_err(|source| Error::Serialize {
                    name: name.to_string(),
                    source,
                })?;
                let path = localized_path(locale, inputs.prefixes, definition, &json, name)?;
                let claim = output_claim(name, definition, path.clone());
                indexes.insert(
                    name.to_string(),
                    Index {
                        name: name.to_string(),
                        path,
                        count: selected.len(),
                        json,
                        claim,
                    },
                );
            }
            locales.insert(locale.clone(), indexes);
        }
        Ok(Self { locales })
    }

    pub(crate) fn output_claims(&self) -> Vec<OutputClaim> {
        self.locales
            .values()
            .flat_map(|indexes| indexes.values().map(|index| index.claim.clone()))
            .collect()
    }

    pub(crate) fn generated(&self, plan: &OutputPlan) -> Vec<GeneratedFile> {
        self.locales
            .values()
            .flat_map(|indexes| indexes.values())
            .map(|index| {
                let path = plan
                    .planned_path(&index.path)
                    .expect("search output paths were checked before rendering");
                GeneratedFile::text(path, index.json.clone())
            })
            .collect()
    }

    pub(crate) fn view(
        &self,
        locale: &Locale,
        name: &str,
        root: &str,
        base_path: &BasePath,
    ) -> Option<View> {
        let index = self.locales.get(locale)?.get(name)?;
        Some(View {
            name: index.name.clone(),
            count: index.count,
            url: base_path.join(&index.path.url_path()),
            relative_url: format!("{root}{}", index.path.url_path().as_str()),
        })
    }

    pub(crate) fn names(&self, locale: &Locale) -> Vec<String> {
        self.locales
            .get(locale)
            .map(|indexes| indexes.keys().cloned().collect())
            .unwrap_or_default()
    }
}

pub(super) struct Inputs<'a> {
    pub(super) pages: &'a BTreeMap<Locale, BTreeMap<PageId, Page>>,
    pub(super) collections:
        &'a BTreeMap<Locale, BTreeMap<crate::config::CollectionName, ResolvedCollection>>,
    pub(super) config: &'a SearchConfig,
    pub(super) prefixes: &'a BTreeMap<Locale, PathBuf>,
    pub(super) base_path: &'a BasePath,
    pub(super) markdown: &'a crate::config::MarkdownConfig,
    pub(super) taxonomies: &'a TaxonomiesConfig,
}

fn selected_ids_for(
    definition: &SearchDefinition,
    collections: &BTreeMap<Locale, BTreeMap<crate::config::CollectionName, ResolvedCollection>>,
    locale: &Locale,
) -> BTreeSet<PageId> {
    definition
        .collections()
        .iter()
        .flat_map(|name| collections[locale][name].page_ids().iter().cloned())
        .collect()
}

fn document(
    page: &Page,
    base_path: &BasePath,
    markdown_config: &crate::config::MarkdownConfig,
    taxonomy_config: &TaxonomiesConfig,
) -> Document {
    Document {
        id: page.id.to_string(),
        url: page
            .output_path
            .as_ref()
            .map(|path| base_path.join(&path.url_path()))
            .expect("collections contain only rendered pages"),
        draft: page.draft,
        title: page.metadata.title.clone(),
        description: page.metadata.description.clone(),
        summary: page.metadata.summary.clone(),
        published: page.metadata.published.as_ref().map(|value| value.render()),
        updated: page.metadata.updated.as_ref().map(|value| value.render()),
        taxonomies: taxonomy_config
            .definitions()
            .keys()
            .map(|name| {
                let terms = page
                    .taxonomies
                    .iter()
                    .find(|membership| membership.name == *name)
                    .map(|membership| {
                        membership
                            .terms
                            .iter()
                            .map(|term| term.term.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                (name.to_string(), terms)
            })
            .collect(),
        sections: source_sections(page, markdown_config),
    }
}

fn source_sections(
    page: &Page,
    markdown_config: &crate::config::MarkdownConfig,
) -> Vec<markdown::SearchTextSection> {
    page.markdown_sources
        .iter()
        .flat_map(|(name, source)| {
            let fragments = page
                .toc
                .as_ref()
                .filter(|toc| toc.content() == name)
                .map(crate::markdown::TocPlan::anchors);
            markdown::search_sections(&source.source, markdown_config, fragments)
        })
        .collect()
}

fn localized_path(
    locale: &Locale,
    prefixes: &BTreeMap<Locale, PathBuf>,
    definition: &SearchDefinition,
    json: &str,
    name: &SearchName,
) -> Result<OutputPath, Error> {
    let path = if definition.versioned() {
        versioned_path(definition.path(), json)
    } else {
        Ok(definition.path().clone())
    }
    .and_then(|path| OutputPath::try_new(prefixes[locale].join(path.as_path())));
    path.map_err(|message| {
        let (source_code, span) = definition
            .path_origin()
            .expect("search configuration sources are attached before resolution");
        Error::OutputPath {
            name: name.to_string(),
            message,
            source_code,
            span,
        }
    })
}

fn versioned_path(path: &OutputPath, json: &str) -> Result<OutputPath, String> {
    let digest_bytes = Sha256::digest(json.as_bytes());
    let mut digest = String::with_capacity(digest_bytes.len() * 2);
    for byte in digest_bytes {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        digest.push(HEX[(byte >> 4) as usize] as char);
        digest.push(HEX[(byte & 0x0f) as usize] as char);
    }
    let filename = path
        .as_path()
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "configured path must have a UTF-8 filename".to_owned())?;
    let stem = filename
        .strip_suffix(".json")
        .ok_or_else(|| "configured path must end in .json".to_owned())?;
    let mut derived = path.as_path().to_path_buf();
    derived.set_file_name(format!("{stem}.{digest}.json"));
    OutputPath::try_new(derived)
}

fn output_claim(name: &SearchName, definition: &SearchDefinition, path: OutputPath) -> OutputClaim {
    let (source, span) = definition
        .path_origin()
        .expect("search configuration sources are attached before resolution");
    OutputClaim::authored(
        path,
        format!("search index {name}"),
        source.as_ref().clone(),
        span,
    )
}
