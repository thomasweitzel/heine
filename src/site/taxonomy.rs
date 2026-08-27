//! Collection-derived taxonomies and their generated render tasks.
//!
//! A taxonomy never selects or orders pages itself. It partitions one resolved
//! collection, preserving that collection's order for every term listing.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::SystemTime};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use thiserror::Error;

use crate::{
    config::{BasePath, CollectionName, TaxonomiesConfig, TaxonomyDefinition, TaxonomyName},
    locale::Locale,
    output::OutputPath,
    template::TemplateName,
};

use super::{
    collation,
    collection::ResolvedCollection,
    page::{Page, PageId, Pager, TaxonomyTermMembership},
};

/// A resolved locale-local taxonomy. Its term members remain page IDs so page
/// facts have exactly one owner in the page catalog.
#[derive(Debug, Default, Clone)]
pub(crate) struct Catalog {
    locales: BTreeMap<Locale, BTreeMap<TaxonomyName, ResolvedTaxonomy>>,
    page_terms: BTreeMap<Locale, BTreeMap<PageId, BTreeMap<String, Vec<TermRoute>>>>,
    prefixes: BTreeMap<Locale, PathBuf>,
    template_references: Vec<TemplateReference>,
    term_translations: BTreeMap<Locale, BTreeMap<TaxonomyName, Arc<TranslationPaths>>>,
}

type TranslationPaths = BTreeMap<String, BTreeMap<Locale, TermRoute>>;
type TranslationClaim = (Arc<NamedSource<String>>, SourceSpan);

#[derive(Debug, Clone)]
struct TermRoute {
    name: String,
    output_path: OutputPath,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedTaxonomy {
    name: TaxonomyName,
    collection: CollectionName,
    root: OutputPath,
    index_template: TemplateName,
    term_template: TemplateName,
    route_origin: Origin,
    per_page_origin: Origin,
    per_page: usize,
    terms: Vec<ResolvedTerm>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedTerm {
    name: String,
    members: Vec<PageId>,
    source: Arc<NamedSource<String>>,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct Origin {
    source: Arc<NamedSource<String>>,
    span: SourceSpan,
}

/// One template required by a taxonomy definition. These references remain
/// separate from generated tasks because an empty taxonomy has no term task,
/// but its term template is still part of the authored contract.
#[derive(Debug, Clone)]
pub(super) struct TemplateReference {
    pub(super) template: TemplateName,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
}

/// One fully validated generated taxonomy output.
#[derive(Debug, Clone)]
pub(super) struct Task {
    pub(super) locale: Locale,
    pub(super) output_path: OutputPath,
    pub(super) template: TemplateName,
    pub(super) output_source: Arc<NamedSource<String>>,
    pub(super) output_span: SourceSpan,
    pub(super) latest_input: SystemTime,
    pub(super) taxonomy: View,
    pub(super) pagination: Option<Arc<Pager>>,
    term_paths: Arc<BTreeMap<String, OutputPath>>,
    term_translations: Arc<TranslationPaths>,
}

/// Deliberate render-facing taxonomy data. URLs are added at rendering time
/// because their relative form depends on the generated output directory.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct View {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    term: Option<TermView>,
    terms: Vec<TermView>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TermView {
    name: String,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_url: Option<String>,
    translations: BTreeMap<String, TermLinkView>,
}

#[derive(Debug, Clone, Serialize)]
struct TermLinkView {
    name: String,
    url: String,
    relative_url: String,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    OutsideCollection(#[from] Box<OutsideCollection>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Collision(#[from] Box<Collision>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingTranslationTerm(#[from] Box<MissingTranslationTerm>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    DuplicateTranslationTerm(#[from] Box<DuplicateTranslationTerm>),
}

#[derive(Debug, Diagnostic, Error)]
#[error(
    "page {page:?} assigns taxonomy {taxonomy:?}, but is not a member of its source collection {collection:?}"
)]
#[diagnostic(code(heine::taxonomy::outside_collection))]
pub(crate) struct OutsideCollection {
    page: String,
    taxonomy: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("taxonomy term assigned here")]
    span: SourceSpan,
    #[related]
    source_collection: Vec<SourceCollectionOrigin>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("taxonomy {taxonomy:?} has terms whose routes collide on portable filesystems")]
#[diagnostic(code(heine::taxonomy::term_collision))]
pub(crate) struct Collision {
    taxonomy: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this term collides with another term route")]
    span: SourceSpan,
    #[related]
    first: Vec<TermOrigin>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error(
    "taxonomy {taxonomy:?} translation group {group:?} refers to missing term {term:?} for locale {locale}"
)]
#[diagnostic(code(heine::taxonomy::missing_translation_term))]
pub(crate) struct MissingTranslationTerm {
    taxonomy: String,
    group: String,
    locale: String,
    term: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this term must exist in the locale-local taxonomy")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error(
    "taxonomy {taxonomy:?} term {term:?} for locale {locale} belongs to more than one translation group"
)]
#[diagnostic(code(heine::taxonomy::duplicate_translation_term))]
pub(crate) struct DuplicateTranslationTerm {
    taxonomy: String,
    locale: String,
    term: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this group also declares the term")]
    span: SourceSpan,
    #[related]
    first: Vec<TranslationTermOrigin>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("another translation group declares this term")]
pub(crate) struct TranslationTermOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("conflicting declaration")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("taxonomy source collection is configured here")]
pub(crate) struct SourceCollectionOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this taxonomy derives members from the collection")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first term producing this route")]
pub(crate) struct TermOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first term declared here")]
    span: SourceSpan,
}

pub(super) fn resolve(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    collections: &BTreeMap<Locale, BTreeMap<crate::config::CollectionName, ResolvedCollection>>,
    config: &TaxonomiesConfig,
    prefixes: BTreeMap<Locale, PathBuf>,
) -> Result<Catalog, Error> {
    let template_references = template_references(config);
    let mut locales = BTreeMap::new();
    let mut page_terms = BTreeMap::new();
    for (locale, locale_pages) in pages {
        let collation = collation::TextCollation::for_locale(locale);
        let mut taxonomies = BTreeMap::new();
        for (name, definition) in config.definitions() {
            let source = &collections[locale][definition.collection()];
            validate_scope(locale_pages, name, definition)?;
            let terms = resolve_terms(&collation, locale_pages, source, name)?;
            taxonomies.insert(name.clone(), resolved(name, definition, terms));
        }
        let mut locale_terms: BTreeMap<PageId, BTreeMap<String, Vec<TermRoute>>> = BTreeMap::new();
        for taxonomy in taxonomies.values() {
            for term in &taxonomy.terms {
                let route = TermRoute {
                    name: term.name.clone(),
                    output_path: term_output_path(
                        &localized_root(&prefixes, locale, &taxonomy.root),
                        &term.name,
                        1,
                    ),
                };
                for page in &term.members {
                    locale_terms
                        .entry(page.clone())
                        .or_default()
                        .entry(taxonomy.name.to_string())
                        .or_default()
                        .push(route.clone());
                }
            }
        }
        locales.insert(locale.clone(), taxonomies);
        page_terms.insert(locale.clone(), locale_terms);
    }
    let term_translations = resolve_term_translations(&locales, config, &prefixes)?;
    Ok(Catalog {
        locales,
        page_terms,
        prefixes,
        template_references,
        term_translations,
    })
}

fn resolve_term_translations(
    locales: &BTreeMap<Locale, BTreeMap<TaxonomyName, ResolvedTaxonomy>>,
    config: &TaxonomiesConfig,
    prefixes: &BTreeMap<Locale, PathBuf>,
) -> Result<BTreeMap<Locale, BTreeMap<TaxonomyName, Arc<TranslationPaths>>>, Error> {
    let mut translations = BTreeMap::new();
    for (taxonomy_name, definition) in config.definitions() {
        let mut by_locale: BTreeMap<Locale, TranslationPaths> = BTreeMap::new();
        let mut claimed: BTreeMap<(Locale, String), TranslationClaim> = BTreeMap::new();
        for (group, members) in definition.translations() {
            let mut resolved = Vec::new();
            for (locale, member) in members.terms() {
                let (source_code, span) = definition
                    .origin(member.span())
                    .expect("loaded taxonomy configuration retains source origins");
                let taxonomy = &locales[locale][taxonomy_name];
                if !taxonomy.terms.iter().any(|term| term.name == member.term()) {
                    return Err(Box::new(MissingTranslationTerm {
                        taxonomy: taxonomy_name.to_string(),
                        group: group.clone(),
                        locale: locale.to_string(),
                        term: member.term().into(),
                        source_code,
                        span,
                    })
                    .into());
                }
                let key = (locale.clone(), member.term().to_owned());
                if let Some((first_source, first_span)) =
                    claimed.insert(key, (source_code.clone(), span))
                {
                    return Err(Box::new(DuplicateTranslationTerm {
                        taxonomy: taxonomy_name.to_string(),
                        locale: locale.to_string(),
                        term: member.term().into(),
                        source_code,
                        span,
                        first: vec![TranslationTermOrigin {
                            source_code: first_source,
                            span: first_span,
                        }],
                    })
                    .into());
                }
                let output_path = term_output_path(
                    &localized_root(prefixes, locale, &taxonomy.root),
                    member.term(),
                    1,
                );
                resolved.push((locale.clone(), member.term().to_owned(), output_path));
            }
            for (locale, term, _) in &resolved {
                let targets = resolved
                    .iter()
                    .filter(|(target_locale, _, _)| target_locale != locale)
                    .map(|(target_locale, target_term, output_path)| {
                        (
                            target_locale.clone(),
                            TermRoute {
                                name: target_term.clone(),
                                output_path: output_path.clone(),
                            },
                        )
                    })
                    .collect();
                by_locale
                    .entry(locale.clone())
                    .or_default()
                    .insert(term.clone(), targets);
            }
        }
        for locale in locales.keys() {
            translations
                .entry(locale.clone())
                .or_insert_with(BTreeMap::new)
                .insert(
                    taxonomy_name.clone(),
                    Arc::new(by_locale.remove(locale).unwrap_or_default()),
                );
        }
    }
    Ok(translations)
}

fn template_references(config: &TaxonomiesConfig) -> Vec<TemplateReference> {
    config
        .definitions()
        .values()
        .flat_map(|definition| {
            let (index_source, index_span) = definition
                .index_template_origin()
                .expect("loaded taxonomy configuration retains source origins");
            let (term_source, term_span) = definition
                .term_template_origin()
                .expect("loaded taxonomy configuration retains source origins");
            [
                TemplateReference {
                    template: definition.index_template().clone(),
                    source: index_source,
                    span: index_span,
                },
                TemplateReference {
                    template: definition.term_template().clone(),
                    source: term_source,
                    span: term_span,
                },
            ]
        })
        .collect()
}

fn validate_scope(
    pages: &BTreeMap<PageId, Page>,
    taxonomy: &TaxonomyName,
    definition: &TaxonomyDefinition,
) -> Result<(), Error> {
    for (id, page) in pages {
        let Some(membership) = page.taxonomies.iter().find(|item| &item.name == taxonomy) else {
            continue;
        };
        if page
            .collections
            .iter()
            .any(|item| &item.name == definition.collection())
        {
            continue;
        }
        let term = membership
            .terms
            .first()
            .expect("loaded taxonomy memberships have a term");
        return Err(Box::new(OutsideCollection {
            page: id.to_string(),
            taxonomy: taxonomy.to_string(),
            collection: definition.collection().to_string(),
            source_code: term.source.clone(),
            span: term.span,
            source_collection: definition
                .collection_origin()
                .map(|(source_code, span)| SourceCollectionOrigin { source_code, span })
                .into_iter()
                .collect(),
            help: format!(
                "add this page to collection {:?}, or remove its {:?} terms",
                definition.collection().to_string(),
                taxonomy.to_string()
            ),
        })
        .into());
    }
    Ok(())
}

fn resolve_terms(
    collation: &collation::TextCollation,
    pages: &BTreeMap<PageId, Page>,
    collection: &ResolvedCollection,
    taxonomy: &TaxonomyName,
) -> Result<Vec<ResolvedTerm>, Error> {
    let mut terms: BTreeMap<String, ResolvedTerm> = BTreeMap::new();
    let mut routes: BTreeMap<String, &TaxonomyTermMembership> = BTreeMap::new();
    for id in collection.page_ids() {
        let page = &pages[id];
        let Some(membership) = page.taxonomies.iter().find(|item| &item.name == taxonomy) else {
            continue;
        };
        for term in &membership.terms {
            let route = term_route_identity(&term.term);
            if let Some(first) = routes.get(&route) {
                if first.term != term.term {
                    return Err(Box::new(Collision {
                        taxonomy: taxonomy.to_string(),
                        source_code: term.source.clone(),
                        span: term.span,
                        first: vec![TermOrigin { source_code: first.source.clone(), span: first.span }],
                        help: "use terms with distinct portable spelling; Heine does not choose one route for ambiguous names".into(),
                    })
                    .into());
                }
            } else {
                routes.insert(route, term);
            }
            let entry = terms
                .entry(term.term.clone())
                .or_insert_with(|| ResolvedTerm {
                    name: term.term.clone(),
                    members: Vec::new(),
                    source: term.source.clone(),
                    span: term.span,
                });
            entry.members.push(id.clone());
        }
    }
    let mut terms = terms.into_values().collect::<Vec<_>>();
    sort_terms(collation, &mut terms);
    Ok(terms)
}

fn sort_terms(collation: &collation::TextCollation, terms: &mut [ResolvedTerm]) {
    terms.sort_by(|left, right| collation.compare(&left.name, &right.name));
}

fn term_route_identity(term: &str) -> String {
    OutputPath::try_new(PathBuf::from(format!("{term}.html")))
        .expect("taxonomy terms are validated as portable output components during page loading")
        .collision_key()
}

fn resolved(
    name: &TaxonomyName,
    definition: &TaxonomyDefinition,
    terms: Vec<ResolvedTerm>,
) -> ResolvedTaxonomy {
    let (route_source, route_span) = definition
        .path_origin()
        .expect("loaded configuration retains source origins");
    let (per_page_source, per_page_span) = definition
        .per_page_origin()
        .expect("loaded configuration retains source origins");
    ResolvedTaxonomy {
        name: name.clone(),
        collection: definition.collection().clone(),
        root: definition.path().clone(),
        index_template: definition.index_template().clone(),
        term_template: definition.term_template().clone(),
        route_origin: Origin {
            source: route_source,
            span: route_span,
        },
        per_page_origin: Origin {
            source: per_page_source,
            span: per_page_span,
        },
        per_page: definition.per_page(),
        terms,
    }
}

impl Catalog {
    pub(super) fn template_references(&self) -> &[TemplateReference] {
        &self.template_references
    }

    /// Resolves a configured taxonomy's locale-local index as a small,
    /// render-facing link. Unlike a term listing, an index exists even when
    /// the taxonomy currently has no terms.
    pub(crate) fn index(
        &self,
        locale: &Locale,
        name: &TaxonomyName,
        base_path: &BasePath,
        root: &str,
    ) -> Option<IndexView> {
        let taxonomy = self.locales.get(locale)?.get(name)?;
        let root_path = localized_root(&self.prefixes, locale, &taxonomy.root);
        let output_path = index_output_path(&root_path);
        Some(IndexView {
            name: taxonomy.name.to_string(),
            url: base_path.join(&output_path.url_path()),
            relative_url: format!("{root}{}", output_path.url_path().as_str()),
        })
    }

    pub(crate) fn names(&self, locale: &Locale) -> Vec<String> {
        self.locales
            .get(locale)
            .map(|taxonomies| taxonomies.keys().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    pub(crate) fn page_terms(
        &self,
        locale: &Locale,
        page: &PageId,
        base_path: &BasePath,
        root: &str,
    ) -> BTreeMap<String, Vec<PageTermView>> {
        self.page_terms
            .get(locale)
            .and_then(|pages| pages.get(page))
            .map(|taxonomies| {
                taxonomies
                    .iter()
                    .map(|(name, terms)| {
                        (
                            name.clone(),
                            terms
                                .iter()
                                .map(|term| PageTermView {
                                    name: term.name.clone(),
                                    url: base_path.join(&term.output_path.url_path()),
                                    relative_url: format!(
                                        "{root}{}",
                                        term.output_path.url_path().as_str()
                                    ),
                                })
                                .collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn tasks(
        &self,
        latest_inputs: &BTreeMap<(Locale, PageId), SystemTime>,
        locale_latest: &BTreeMap<Locale, SystemTime>,
    ) -> Vec<Task> {
        let mut tasks = Vec::new();
        for (locale, taxonomies) in &self.locales {
            for taxonomy in taxonomies.values() {
                let root = localized_root(&self.prefixes, locale, &taxonomy.root);
                let term_paths: Arc<BTreeMap<String, OutputPath>> = Arc::new(
                    taxonomy
                        .terms
                        .iter()
                        .map(|term| (term.name.clone(), term_output_path(&root, &term.name, 1)))
                        .collect(),
                );
                let index_output = index_output_path(&root);
                let term_translations = self.term_translations[locale][&taxonomy.name].clone();
                tasks.push(Task {
                    locale: locale.clone(),
                    output_path: index_output,
                    template: taxonomy.index_template.clone(),
                    output_source: taxonomy.route_origin.source.clone(),
                    output_span: taxonomy.route_origin.span,
                    latest_input: latest_for_terms(locale, &taxonomy.terms, latest_inputs)
                        .max(locale_latest[locale]),
                    taxonomy: View {
                        name: taxonomy.name.to_string(),
                        term: None,
                        terms: taxonomy.terms.iter().map(term_view).collect(),
                    },
                    pagination: None,
                    term_paths: term_paths.clone(),
                    term_translations: term_translations.clone(),
                });
                let resources = TermTaskResources {
                    latest_inputs,
                    locale_latest: locale_latest[locale],
                    term_paths,
                    term_translations,
                };
                for term in &taxonomy.terms {
                    tasks.extend(term_tasks(locale, &root, taxonomy, term, &resources));
                }
            }
        }
        tasks
    }
}

/// A taxonomy index link resolved for the current template locale and output
/// directory. It deliberately mirrors the URL shape of `page()` and `asset()`
/// without exposing the taxonomy's internal resolved model.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct IndexView {
    name: String,
    url: String,
    relative_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PageTermView {
    name: String,
    url: String,
    relative_url: String,
}

struct TermTaskResources<'a> {
    latest_inputs: &'a BTreeMap<(Locale, PageId), SystemTime>,
    locale_latest: SystemTime,
    term_paths: Arc<BTreeMap<String, OutputPath>>,
    term_translations: Arc<TranslationPaths>,
}

fn term_tasks(
    locale: &Locale,
    root: &OutputPath,
    taxonomy: &ResolvedTaxonomy,
    term: &ResolvedTerm,
    resources: &TermTaskResources<'_>,
) -> Vec<Task> {
    let page_count = term.members.len().div_ceil(taxonomy.per_page).max(1);
    let paths = Arc::new(
        (1..=page_count)
            .map(|number| term_output_path(root, &term.name, number))
            .collect::<Vec<_>>(),
    );
    let latest_input = term
        .members
        .iter()
        .filter_map(|id| {
            resources
                .latest_inputs
                .get(&(locale.clone(), id.clone()))
                .copied()
        })
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .max(resources.locale_latest);
    (1..=page_count)
        .map(|number| Task {
            locale: locale.clone(),
            output_path: paths[number - 1].clone(),
            template: taxonomy.term_template.clone(),
            output_source: if number == 1 {
                term.source.clone()
            } else {
                taxonomy.per_page_origin.source.clone()
            },
            output_span: if number == 1 {
                term.span
            } else {
                taxonomy.per_page_origin.span
            },
            latest_input,
            taxonomy: View {
                name: taxonomy.name.to_string(),
                term: Some(term_view(term)),
                terms: Vec::new(),
            },
            pagination: Some(Arc::new(Pager {
                collection: taxonomy.collection.to_string(),
                page_ids: page_members(&term.members, taxonomy.per_page, number),
                page_number: number,
                page_count,
                item_count: term.members.len(),
                per_page: taxonomy.per_page,
                navigation_window: 1,
                paths: paths.clone(),
            })),
            term_paths: resources.term_paths.clone(),
            term_translations: resources.term_translations.clone(),
        })
        .collect()
}

fn localized_root(
    prefixes: &BTreeMap<Locale, PathBuf>,
    locale: &Locale,
    root: &OutputPath,
) -> OutputPath {
    OutputPath::try_new(prefixes[locale].join(root.as_path()))
        .expect("locale prefixes and taxonomy roots are validated output paths")
}

impl Task {
    pub(super) fn taxonomy_view(&self, base_path: &BasePath, root: &str) -> View {
        let mut view = self.taxonomy.clone();
        for term in view.terms.iter_mut().chain(view.term.iter_mut()) {
            let path = &self.term_paths[&term.name];
            term.url = Some(base_path.join(&path.url_path()));
            term.relative_url = Some(format!("{root}{}", path.url_path().as_str()));
            term.translations = self
                .term_translations
                .get(&term.name)
                .into_iter()
                .flat_map(|targets| targets.iter())
                .map(|(locale, target)| {
                    (
                        locale.to_string(),
                        TermLinkView {
                            name: target.name.clone(),
                            url: base_path.join(&target.output_path.url_path()),
                            relative_url: format!(
                                "{root}{}",
                                target.output_path.url_path().as_str()
                            ),
                        },
                    )
                })
                .collect();
        }
        view
    }
}

fn term_output_path(root: &OutputPath, term: &str, page_number: usize) -> OutputPath {
    let filename = format!("{term}.html");
    let path = if page_number == 1 {
        root.as_path().join(filename)
    } else {
        root.as_path()
            .join(term)
            .join("page")
            .join(format!("{page_number}.html"))
    };
    OutputPath::try_new(path).expect("taxonomy routes derive from validated components")
}

fn index_output_path(root: &OutputPath) -> OutputPath {
    OutputPath::try_new(root.as_path().join("index.html"))
        .expect("taxonomy roots are validated output paths")
}

fn page_members(ids: &[PageId], per_page: usize, page_number: usize) -> Vec<PageId> {
    let start = (page_number - 1) * per_page;
    let end = (start + per_page).min(ids.len());
    ids[start..end].to_vec()
}

fn latest_for_terms(
    locale: &Locale,
    terms: &[ResolvedTerm],
    latest: &BTreeMap<(Locale, PageId), SystemTime>,
) -> SystemTime {
    terms
        .iter()
        .flat_map(|term| term.members.iter())
        .filter_map(|id| latest.get(&(locale.clone(), id.clone())).copied())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn term_view(term: &ResolvedTerm) -> TermView {
    TermView {
        name: term.name.clone(),
        count: term.members.len(),
        url: None,
        relative_url: None,
        translations: BTreeMap::new(),
    }
}
