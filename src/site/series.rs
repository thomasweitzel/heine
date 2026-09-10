//! Checked, locale-local ordered reading sequences and their generated tasks.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::SystemTime,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use thiserror::Error;

use crate::{
    config::{BasePath, SeriesConfig, SeriesName},
    locale::Locale,
    localization::{Arguments, Localization},
    output::OutputPath,
    template::TemplateName,
};

use super::{
    PageCatalog, collation,
    index::OrderedPages,
    page::{Page, PageId, Pager, SeriesMembership},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Catalog {
    locales: BTreeMap<Locale, LocaleCatalog>,
    configured: bool,
    definitions: BTreeSet<SeriesName>,
    tasks: Vec<Task>,
    template_references: Vec<TemplateReference>,
}

#[derive(Debug, Clone, Default)]
struct LocaleCatalog {
    index: Index,
    series: BTreeMap<SeriesName, ResolvedSeries>,
    page_memberships: BTreeMap<PageId, Vec<PageMembership>>,
}

#[derive(Debug, Clone)]
struct PageMembership {
    name: SeriesName,
    position: usize,
}

#[derive(Debug, Clone, Default)]
struct Index {
    title: String,
    description: Option<String>,
    path: Option<OutputPath>,
}

#[derive(Debug, Clone)]
struct ResolvedSeries {
    name: SeriesName,
    title: String,
    description: Option<String>,
    path: OutputPath,
    members: OrderedPages,
}

#[derive(Debug, Clone)]
pub(super) struct Task {
    pub(super) locale: Locale,
    pub(super) output_path: OutputPath,
    pub(super) template: TemplateName,
    pub(super) output_source: Arc<NamedSource<String>>,
    pub(super) output_span: SourceSpan,
    pub(super) latest_input: SystemTime,
    pub(super) series_id: Option<SeriesName>,
    pub(super) pagination: Option<Arc<Pager>>,
}

#[derive(Debug, Clone)]
pub(super) struct TemplateReference {
    pub(super) template: TemplateName,
    pub(super) source: Arc<NamedSource<String>>,
    pub(super) span: SourceSpan,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct View {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    series: Vec<IndexEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    page_ids: Vec<PageId>,
}

#[derive(Debug, Clone, Serialize)]
struct IndexEntry {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    count: usize,
    url: String,
    relative_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MembershipView {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    position: usize,
    number: usize,
    total: usize,
    url: String,
    relative_url: String,
    previous: Option<PageLinkView>,
    next: Option<PageLinkView>,
}

#[derive(Debug, Clone, Serialize)]
struct PageLinkView {
    id: PageId,
    title: String,
    draft: bool,
    url: String,
    relative_url: String,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("page {page:?} joins Series {series:?}, but is not rendered")]
    #[diagnostic(code(heine::series::unrendered_member))]
    Unrendered {
        page: String,
        series: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("Series membership declared here")]
        span: SourceSpan,
    },
    #[error("page {page:?} joins Series {series:?}, but has no title")]
    #[diagnostic(code(heine::series::missing_title))]
    MissingTitle {
        page: String,
        series: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("Series membership declared here")]
        span: SourceSpan,
    },
    #[error("Series {series:?} uses position {position} more than once in locale {locale}")]
    #[diagnostic(code(heine::series::duplicate_position))]
    DuplicatePosition {
        series: String,
        position: usize,
        locale: Locale,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("duplicate position declared here")]
        span: SourceSpan,
        #[related]
        first: Vec<MembershipOrigin>,
    },
    #[error("Series localization for {series:?} is invalid")]
    #[diagnostic(code(heine::series::localization))]
    Localization {
        series: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this Fluent key could not be resolved")]
        span: SourceSpan,
        #[help]
        help: String,
    },
    #[error(
        "Series definitions {first:?} and {second:?} have routes that collide on portable filesystems"
    )]
    #[diagnostic(code(heine::series::route_collision))]
    DefinitionRouteCollision {
        first: String,
        second: String,
        #[source_code]
        source_code: Arc<NamedSource<String>>,
        #[label("this Series definition has the colliding route")]
        span: SourceSpan,
        #[related]
        first_origin: Vec<DefinitionOrigin>,
        #[help]
        help: String,
    },
}

#[derive(Debug, Diagnostic, Error)]
#[error("first membership using this position")]
pub(crate) struct MembershipOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first position declaration")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first Series definition with this route")]
pub(crate) struct DefinitionOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first colliding Series definition")]
    span: SourceSpan,
}

pub(super) fn resolve(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    config: Option<&SeriesConfig>,
    prefixes: &BTreeMap<Locale, PathBuf>,
    localization: &Localization,
) -> Result<Catalog, Error> {
    let Some(config) = config else {
        return Ok(Catalog::default());
    };
    check_definition_routes(config)?;
    let template_references = [
        (config.index_template(), config.index_template_origin()),
        (config.listing_template(), config.listing_template_origin()),
    ]
    .into_iter()
    .map(|(template, origin)| {
        let (source, span) =
            origin.expect("Series configuration sources are attached before resolution");
        TemplateReference {
            template: template.clone(),
            source,
            span,
        }
    })
    .collect();
    let mut locales = BTreeMap::new();
    for (locale, locale_pages) in pages {
        let prefix = &prefixes[locale];
        let index_path = output(prefix, &config.path().as_path().join("index.html"));
        let title = configured_message(
            localization,
            locale,
            "index",
            config.index_title_key(),
            config.index_title_key_origin(),
        )?;
        let description = config
            .index_description_key()
            .map(|key| {
                configured_message(
                    localization,
                    locale,
                    "index",
                    key,
                    config.index_description_key_origin(),
                )
            })
            .transpose()?;
        let mut groups: BTreeMap<SeriesName, Vec<(&PageId, &Page, &SeriesMembership)>> =
            BTreeMap::new();
        let mut page_memberships: BTreeMap<PageId, Vec<PageMembership>> = BTreeMap::new();
        for (id, page) in locale_pages {
            if !page.series.is_empty() {
                page_memberships.insert(
                    id.clone(),
                    page.series
                        .iter()
                        .map(|membership| PageMembership {
                            name: membership.name.clone(),
                            position: membership.position,
                        })
                        .collect(),
                );
            }
            for membership in &page.series {
                if page.output_path.is_none() {
                    return Err(Error::Unrendered {
                        page: id.to_string(),
                        series: membership.name.to_string(),
                        source_code: membership.source.clone(),
                        span: membership.span,
                    });
                }
                if page.metadata.title.is_none() {
                    return Err(Error::MissingTitle {
                        page: id.to_string(),
                        series: membership.name.to_string(),
                        source_code: membership.source.clone(),
                        span: membership.span,
                    });
                }
                groups
                    .entry(membership.name.clone())
                    .or_default()
                    .push((id, page, membership));
            }
        }
        let mut resolved = BTreeMap::new();
        for (name, mut members) in groups {
            members.sort_by_key(|(_, _, membership)| membership.position);
            for pair in members.windows(2) {
                if pair[0].2.position == pair[1].2.position {
                    return Err(Error::DuplicatePosition {
                        series: name.to_string(),
                        position: pair[1].2.position,
                        locale: locale.clone(),
                        source_code: pair[1].2.source.clone(),
                        span: pair[1].2.position_span,
                        first: vec![MembershipOrigin {
                            source_code: pair[0].2.source.clone(),
                            span: pair[0].2.position_span,
                        }],
                    });
                }
            }
            let definition = &config.definitions()[&name];
            let title = definition_message(
                localization,
                locale,
                &name,
                definition.title_key(),
                definition.title_key_origin(),
            )?;
            let description = definition
                .description_key()
                .map(|key| {
                    definition_message(
                        localization,
                        locale,
                        &name,
                        key,
                        definition.description_key_origin(),
                    )
                })
                .transpose()?;
            let path = output(
                prefix,
                &config
                    .path()
                    .as_path()
                    .join(name.to_string())
                    .join("index.html"),
            );
            let members =
                OrderedPages::new(members.into_iter().map(|(id, _, _)| id.clone()).collect());
            resolved.insert(
                name.clone(),
                ResolvedSeries {
                    name,
                    title,
                    description,
                    path,
                    members,
                },
            );
        }
        locales.insert(
            locale.clone(),
            LocaleCatalog {
                index: Index {
                    title,
                    description,
                    path: Some(index_path),
                },
                series: resolved,
                page_memberships,
            },
        );
    }
    let mut tasks = Vec::new();
    for (locale, catalog) in &locales {
        let (source, span) = config
            .path_origin()
            .expect("Series configuration sources are attached before resolution");
        tasks.push(Task {
            locale: locale.clone(),
            output_path: catalog
                .index
                .path
                .clone()
                .expect("configured Series index has a route"),
            template: config.index_template().clone(),
            output_source: source,
            output_span: span,
            latest_input: SystemTime::UNIX_EPOCH,
            series_id: None,
            pagination: None,
        });
        for series in catalog.series.values() {
            let page_count = series.members.len().div_ceil(config.per_page()).max(1);
            let paths = Arc::new(
                std::iter::once(series.path.clone())
                    .chain((2..=page_count).map(|number| listing_page_path(&series.path, number)))
                    .collect::<Vec<_>>(),
            );
            for page_number in 1..=page_count {
                let (source, span) = if page_number == 1 {
                    config.path_origin()
                } else {
                    config.per_page_origin()
                }
                .expect("Series configuration sources are attached before resolution");
                let start = (page_number - 1) * config.per_page();
                let end = (start + config.per_page()).min(series.members.len());
                tasks.push(Task {
                    locale: locale.clone(),
                    output_path: paths[page_number - 1].clone(),
                    template: config.listing_template().clone(),
                    output_source: source,
                    output_span: span,
                    latest_input: SystemTime::UNIX_EPOCH,
                    series_id: Some(series.name.clone()),
                    pagination: Some(Arc::new(Pager {
                        collection: series.name.to_string(),
                        page_ids: series.members.page_ids()[start..end].to_vec(),
                        page_number,
                        page_count,
                        item_count: series.members.len(),
                        per_page: config.per_page(),
                        navigation_window: 1,
                        paths: paths.clone(),
                    })),
                });
            }
        }
    }
    tasks.sort_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then(left.output_path.cmp(&right.output_path))
    });
    Ok(Catalog {
        locales,
        configured: true,
        definitions: config.definitions().keys().cloned().collect(),
        tasks,
        template_references,
    })
}

fn check_definition_routes(config: &SeriesConfig) -> Result<(), Error> {
    let mut routes: BTreeMap<String, (&SeriesName, &crate::config::SeriesDefinition)> =
        BTreeMap::new();
    for (name, definition) in config.definitions() {
        let route = OutputPath::try_new(
            config
                .path()
                .as_path()
                .join(name.to_string())
                .join("index.html"),
        )
        .expect("checked Series route components remain safe")
        .collision_key();
        if let Some((first_name, first_definition)) = routes.get(&route) {
            if first_name != &name {
                let (source_code, span) = definition
                    .name_origin()
                    .expect("Series configuration sources are attached before resolution");
                let (first_source, first_span) = first_definition
                    .name_origin()
                    .expect("Series configuration sources are attached before resolution");
                return Err(Error::DefinitionRouteCollision {
                    first: first_name.to_string(),
                    second: name.to_string(),
                    source_code,
                    span,
                    first_origin: vec![DefinitionOrigin {
                        source_code: first_source,
                        span: first_span,
                    }],
                    help: "choose Series IDs with distinct portable spellings; Heine does not choose one route for ambiguous definitions".into(),
                });
            }
        } else {
            routes.insert(route, (name, definition));
        }
    }
    Ok(())
}

fn listing_page_path(first: &OutputPath, number: usize) -> OutputPath {
    let parent = first
        .as_path()
        .parent()
        .expect("Series listing has a parent");
    OutputPath::try_new(parent.join("page").join(format!("{number}.html")))
        .expect("fixed Series pager path remains safe")
}

fn output(prefix: &std::path::Path, path: &std::path::Path) -> OutputPath {
    OutputPath::try_new(prefix.join(path)).expect("checked Series output components remain safe")
}

fn configured_message(
    localization: &Localization,
    locale: &Locale,
    series: &str,
    key: &str,
    origin: Option<(Arc<NamedSource<String>>, SourceSpan)>,
) -> Result<String, Error> {
    localization
        .format(locale, key, None, &Arguments::new())
        .map_err(|help| {
            let (source_code, span) =
                origin.expect("Series configuration sources are attached before resolution");
            Error::Localization {
                series: series.into(),
                source_code,
                span,
                help,
            }
        })
}

fn definition_message(
    localization: &Localization,
    locale: &Locale,
    name: &SeriesName,
    key: &str,
    origin: Option<(Arc<NamedSource<String>>, SourceSpan)>,
) -> Result<String, Error> {
    configured_message(localization, locale, &name.to_string(), key, origin)
}

impl Catalog {
    pub(crate) fn configured(&self) -> bool {
        self.configured
    }

    pub(super) fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub(super) fn template_references(&self) -> &[TemplateReference] {
        &self.template_references
    }

    /// Gives generated Series views the same quick-build freshness contract as
    /// other relationship-derived listings. A listing can expose facts from
    /// any of its members, so every member contributes to its newest input.
    pub(super) fn update_latest_inputs(
        &mut self,
        latest_inputs: &BTreeMap<(Locale, PageId), SystemTime>,
        locale_latest: &BTreeMap<Locale, SystemTime>,
    ) {
        let mut latest_by_series = BTreeMap::new();
        let mut latest_by_locale = BTreeMap::new();
        for (locale, catalog) in &self.locales {
            let mut latest = SystemTime::UNIX_EPOCH;
            for (name, series) in &catalog.series {
                let series_latest = latest_for_members(&series.members, locale, latest_inputs);
                latest = latest.max(series_latest);
                latest_by_series.insert((locale.clone(), name.clone()), series_latest);
            }
            latest_by_locale.insert(locale.clone(), latest);
        }
        for task in &mut self.tasks {
            let member_latest = match &task.series_id {
                Some(id) => latest_by_series[&(task.locale.clone(), id.clone())],
                None => latest_by_locale[&task.locale],
            };
            task.latest_input = member_latest.max(locale_latest[&task.locale]);
        }
    }

    pub(crate) fn names(&self, locale: &Locale) -> Vec<String> {
        self.locales
            .get(locale)
            .map(|catalog| catalog.series.keys().map(ToString::to_string).collect())
            .unwrap_or_default()
    }

    pub(crate) fn has_members(&self, locale: &Locale, id: &SeriesName) -> bool {
        self.locales
            .get(locale)
            .and_then(|catalog| catalog.series.get(id))
            .is_some()
    }

    pub(crate) fn definition_exists(&self, id: &SeriesName) -> bool {
        self.definitions.contains(id)
    }

    pub(crate) fn index(&self, locale: &Locale, base_path: &BasePath, root: &str) -> Option<View> {
        let catalog = self.locales.get(locale)?;
        let path = catalog.index.path.as_ref()?;
        let collation = collation::TextCollation::for_locale(locale);
        let mut entries = catalog
            .series
            .values()
            .map(|series| IndexEntry {
                id: series.name.to_string(),
                title: series.title.clone(),
                description: series.description.clone(),
                count: series.members.len(),
                url: base_path.join(&series.path.url_path()),
                relative_url: format!("{root}{}", series.path.url_path().as_str()),
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            collation
                .compare(&left.title, &right.title)
                .then(left.id.cmp(&right.id))
        });
        Some(View {
            id: "index".into(),
            title: catalog.index.title.clone(),
            description: catalog.index.description.clone(),
            count: entries.len(),
            url: Some(base_path.join(&path.url_path())),
            relative_url: Some(format!("{root}{}", path.url_path().as_str())),
            series: entries,
            page_ids: Vec::new(),
        })
    }

    pub(crate) fn view(
        &self,
        locale: &Locale,
        id: &SeriesName,
        base_path: &BasePath,
        root: &str,
    ) -> Option<View> {
        let series = self.locales.get(locale)?.series.get(id)?;
        Some(View {
            id: series.name.to_string(),
            title: series.title.clone(),
            description: series.description.clone(),
            count: series.members.len(),
            url: Some(base_path.join(&series.path.url_path())),
            relative_url: Some(format!("{root}{}", series.path.url_path().as_str())),
            series: Vec::new(),
            page_ids: series.members.page_ids().to_vec(),
        })
    }

    pub(crate) fn page_memberships(
        &self,
        locale: &Locale,
        page: &PageId,
        pages: &PageCatalog,
        base_path: &BasePath,
        root: &str,
    ) -> Vec<MembershipView> {
        let Some(catalog) = self.locales.get(locale) else {
            return Vec::new();
        };
        catalog
            .page_memberships
            .get(page)
            .into_iter()
            .flatten()
            .map(|membership| {
                let series = &catalog.series[&membership.name];
                let neighbors = series
                    .members
                    .neighbors(page)
                    .expect("resolved Series membership has an ordered member");
                MembershipView {
                    id: series.name.to_string(),
                    title: series.title.clone(),
                    description: series.description.clone(),
                    position: membership.position,
                    number: series
                        .members
                        .position(page)
                        .expect("resolved Series membership has an ordered member")
                        + 1,
                    total: series.members.len(),
                    url: base_path.join(&series.path.url_path()),
                    relative_url: format!("{root}{}", series.path.url_path().as_str()),
                    previous: neighbors
                        .before
                        .and_then(|id| page_link(locale, &id, pages, base_path, root)),
                    next: neighbors
                        .after
                        .and_then(|id| page_link(locale, &id, pages, base_path, root)),
                }
            })
            .collect()
    }
}

fn latest_for_members(
    members: &OrderedPages,
    locale: &Locale,
    latest_inputs: &BTreeMap<(Locale, PageId), SystemTime>,
) -> SystemTime {
    members
        .page_ids()
        .iter()
        .filter_map(|page| latest_inputs.get(&(locale.clone(), page.clone())).copied())
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn page_link(
    locale: &Locale,
    id: &PageId,
    pages: &PageCatalog,
    base_path: &BasePath,
    root: &str,
) -> Option<PageLinkView> {
    let page = pages.view(locale, id)?;
    let path = pages.output_path(locale, id)?;
    Some(PageLinkView {
        id: id.clone(),
        title: page.title()?.to_owned(),
        draft: page.is_draft(),
        url: base_path.join(&path.url_path()),
        relative_url: format!("{root}{}", path.url_path().as_str()),
    })
}
