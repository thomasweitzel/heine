//! Validated, deterministic locale-local page collections.
//!
//! This module resolves authored membership before rendering. It deliberately
//! does not create output: pagination and other generated-resource features
//! use this stable relationship separately.

use std::{cmp::Ordering, collections::BTreeMap, sync::Arc};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use thiserror::Error;

use crate::{
    config::{CollectionDefinition, CollectionName, CollectionOrder, CollectionsConfig},
    locale::Locale,
};

use super::{
    collation,
    index::{Neighbors, OrderedPages},
    page::{CollectionMembership, Page, PageId, PublicationTime},
};

/// The resolved domain representation. It retains IDs rather than cloned
/// pages, keeping the resolved page map the only source of page facts.
#[derive(Debug)]
pub(super) struct ResolvedCollection {
    name: CollectionName,
    page_ids: Vec<PageId>,
}

impl ResolvedCollection {
    pub(super) fn view(&self) -> CollectionView {
        CollectionView {
            name: self.name.to_string(),
            count: self.page_ids.len(),
            page_ids: self.page_ids.clone(),
        }
    }

    pub(super) fn page_ids(&self) -> &[PageId] {
        &self.page_ids
    }
}

/// Deliberate template-facing collection data.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CollectionView {
    name: String,
    count: usize,
    page_ids: Vec<PageId>,
}

/// Locale-local adjacent-member indexes derived from resolved collection
/// order. This remains separate from the serializable collection view because
/// it is a render-time lookup aid, not template data by itself.
#[derive(Debug, Clone, Default)]
pub(crate) struct CollectionNeighborIndex {
    collections: BTreeMap<Locale, BTreeMap<String, OrderedPages>>,
}

impl CollectionNeighborIndex {
    pub(crate) fn neighbors(
        &self,
        locale: &Locale,
        name: &str,
        page: &PageId,
    ) -> Option<Neighbors> {
        self.collections.get(locale)?.get(name)?.neighbors(page)
    }
}

pub(super) fn resolve_neighbors(
    collections: &BTreeMap<Locale, BTreeMap<CollectionName, ResolvedCollection>>,
) -> CollectionNeighborIndex {
    CollectionNeighborIndex {
        collections: collections
            .iter()
            .map(|(locale, collections)| {
                (
                    locale.clone(),
                    collections
                        .iter()
                        .map(|(name, collection)| {
                            (
                                name.to_string(),
                                OrderedPages::new(collection.page_ids.clone()),
                            )
                        })
                        .collect(),
                )
            })
            .collect(),
    }
}

#[derive(Debug, Diagnostic, Error)]
#[error("page {page:?} is not rendered and cannot belong to collection {collection:?}")]
#[diagnostic(code(heine::collection::unrendered_member))]
pub(crate) struct UnrenderedMember {
    page: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("collection membership declared here")]
    span: SourceSpan,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error(
    "collection {collection:?} orders by publication time, but page {page:?} has no `published` value"
)]
#[diagnostic(code(heine::collection::missing_published))]
pub(crate) struct MissingPublished {
    page: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("collection membership declared here; this collection orders by publication time")]
    span: SourceSpan,
    #[related]
    order: Vec<CollectionOrderOrigin>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("collection {collection:?} orders by title, but page {page:?} has no title")]
#[diagnostic(code(heine::collection::missing_title))]
pub(crate) struct MissingTitle {
    page: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("collection membership declared here; this collection orders by title")]
    span: SourceSpan,
    #[related]
    order: Vec<CollectionOrderOrigin>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("collection {collection:?} orders by weight, but page {page:?} has no numeric weight")]
#[diagnostic(code(heine::collection::missing_weight))]
pub(crate) struct MissingWeight {
    page: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    label: String,
    #[label("{label}")]
    span: SourceSpan,
    #[related]
    related: Vec<MissingWeightRelation>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("collection ordering is declared here")]
struct CollectionOrderOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this order requires the reported value")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("collection membership is declared here")]
struct CollectionMembershipOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this page belongs to the collection")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
enum MissingWeightRelation {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Membership(#[from] CollectionMembershipOrigin),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Order(#[from] CollectionOrderOrigin),
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Unrendered(#[from] Box<UnrenderedMember>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingPublished(#[from] Box<MissingPublished>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingTitle(#[from] Box<MissingTitle>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingWeight(#[from] Box<MissingWeight>),
}

/// Resolves every configured collection for every locale. Empty collections
/// are inserted deliberately, so templates never need to distinguish an empty
/// definition from an absent one.
pub(super) fn resolve(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    config: &CollectionsConfig,
) -> Result<BTreeMap<Locale, BTreeMap<CollectionName, ResolvedCollection>>, Error> {
    pages
        .iter()
        .map(|(locale, pages)| {
            resolve_locale(locale, pages, config).map(|collections| (locale.clone(), collections))
        })
        .collect()
}

fn resolve_locale(
    locale: &Locale,
    pages: &BTreeMap<PageId, Page>,
    config: &CollectionsConfig,
) -> Result<BTreeMap<CollectionName, ResolvedCollection>, Error> {
    let collation = collation::TextCollation::for_locale(locale);
    let mut collections = config
        .definitions()
        .keys()
        .cloned()
        .map(|name| {
            (
                name.clone(),
                ResolvedCollection {
                    name,
                    page_ids: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (id, page) in pages {
        for membership in &page.collections {
            if page.path.is_none() {
                return Err(Box::new(UnrenderedMember {
                    page: id.to_string(),
                    collection: membership.name.to_string(),
                    source_code: membership.source.clone(),
                    span: membership.span,
                    help: "give this page a template, or remove it from the collection".into(),
                })
                .into());
            }
            collections
                .get_mut(&membership.name)
                .expect("memberships are validated against configured collections")
                .page_ids
                .push(id.clone());
        }
    }

    for (name, collection) in &mut collections {
        let definition = &config.definitions()[name];
        validate_ordering_data(name, definition, &collection.page_ids, pages)?;
        collection.page_ids.sort_by(|left, right| {
            compare_pages(&collation, definition.order(), left, right, pages)
        });
    }
    Ok(collections)
}

fn validate_ordering_data(
    name: &CollectionName,
    definition: &CollectionDefinition,
    ids: &[PageId],
    pages: &BTreeMap<PageId, Page>,
) -> Result<(), Error> {
    match definition.order() {
        CollectionOrder::PublishedAsc | CollectionOrder::PublishedDesc => {
            for id in ids {
                let page = &pages[id];
                if page.metadata.published.is_none() {
                    let membership = membership(page, name);
                    return Err(Box::new(MissingPublished {
                        page: id.to_string(),
                        collection: name.to_string(),
                        source_code: membership.source.clone(),
                        span: membership.span,
                        order: collection_order_origin(definition).into_iter().collect(),
                        help: "add a TOML local date-time as `published`, or use another collection order".into(),
                    })
                    .into());
                }
            }
        }
        CollectionOrder::TitleAsc | CollectionOrder::TitleDesc => {
            for id in ids {
                let page = &pages[id];
                if page.metadata.title.is_none() {
                    let membership = membership(page, name);
                    return Err(Box::new(MissingTitle {
                        page: id.to_string(),
                        collection: name.to_string(),
                        source_code: membership.source.clone(),
                        span: membership.span,
                        order: collection_order_origin(definition).into_iter().collect(),
                        help: "add a title, or use another collection order".into(),
                    })
                    .into());
                }
            }
        }
        CollectionOrder::WeightAsc | CollectionOrder::WeightDesc => {
            for id in ids {
                let page = &pages[id];
                if page.weight.ordered().is_none() {
                    let membership = membership(page, name);
                    let (source_code, span, label, related) = match page.weight_span {
                        Some(span) => (
                            Arc::new(page.source.clone()),
                            span,
                            "this value is not a numeric collection weight".into(),
                            vec![MissingWeightRelation::Membership(
                                CollectionMembershipOrigin {
                                    source_code: membership.source.clone(),
                                    span: membership.span,
                                },
                            )],
                        ),
                        None => (
                            membership.source.clone(),
                            membership.span,
                            "collection membership declared here; this collection orders by weight"
                                .into(),
                            Vec::new(),
                        ),
                    };
                    return Err(Box::new(MissingWeight {
                        page: id.to_string(),
                        collection: name.to_string(),
                        source_code,
                        label,
                        span,
                        related: related
                            .into_iter()
                            .chain(
                                collection_order_origin(definition)
                                    .map(MissingWeightRelation::Order),
                            )
                            .collect(),
                        help: "set a numeric weight, or use another collection order".into(),
                    })
                    .into());
                }
            }
        }
        CollectionOrder::IdAsc | CollectionOrder::IdDesc => {}
    }
    Ok(())
}

fn collection_order_origin(definition: &CollectionDefinition) -> Option<CollectionOrderOrigin> {
    definition
        .order_origin()
        .map(|(source_code, span)| CollectionOrderOrigin { source_code, span })
}

fn membership<'a>(page: &'a Page, name: &CollectionName) -> &'a CollectionMembership {
    page.collections
        .iter()
        .find(|membership| &membership.name == name)
        .expect("collection member has the corresponding membership")
}

fn compare_pages(
    collation: &collation::TextCollation,
    order: CollectionOrder,
    left: &PageId,
    right: &PageId,
    pages: &BTreeMap<PageId, Page>,
) -> Ordering {
    let primary = match order {
        CollectionOrder::PublishedAsc | CollectionOrder::PublishedDesc => {
            compare_publication_times(
                pages[left]
                    .metadata
                    .published
                    .as_ref()
                    .expect("published ordering was validated"),
                pages[right]
                    .metadata
                    .published
                    .as_ref()
                    .expect("published ordering was validated"),
            )
        }
        CollectionOrder::TitleAsc | CollectionOrder::TitleDesc => collation.compare(
            pages[left]
                .metadata
                .title
                .as_deref()
                .expect("title ordering was validated"),
            pages[right]
                .metadata
                .title
                .as_deref()
                .expect("title ordering was validated"),
        ),
        CollectionOrder::WeightAsc | CollectionOrder::WeightDesc => pages[left]
            .weight
            .ordered()
            .expect("weight ordering was validated")
            .cmp(
                &pages[right]
                    .weight
                    .ordered()
                    .expect("weight ordering was validated"),
            ),
        CollectionOrder::IdAsc | CollectionOrder::IdDesc => left.cmp(right),
    };
    let primary = match order {
        CollectionOrder::PublishedDesc
        | CollectionOrder::TitleDesc
        | CollectionOrder::WeightDesc
        | CollectionOrder::IdDesc => primary.reverse(),
        _ => primary,
    };
    primary.then_with(|| left.cmp(right))
}

fn compare_publication_times(left: &PublicationTime, right: &PublicationTime) -> Ordering {
    left.cmp(right)
}
