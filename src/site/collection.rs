//! Validated, deterministic locale-local page collections.
//!
//! This module resolves authored membership before rendering. It deliberately
//! does not create output: pagination and other generated-resource features
//! are separate consumers of this stable relationship.

use std::{cmp::Ordering, collections::BTreeMap, sync::Arc};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::Serialize;
use thiserror::Error;

use crate::{
    config::{CollectionDefinition, CollectionName, CollectionOrder, CollectionsConfig},
    locale::Locale,
};

use super::page::{CollectionMembership, MetadataDate, Page, PageId};

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
#[error("collection {collection:?} orders by date, but page {page:?} has no date")]
#[diagnostic(code(heine::collection::missing_date))]
pub(crate) struct MissingDate {
    page: String,
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("collection membership declared here; this collection orders by date")]
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
#[error("collection {collection:?} mixes date-only values and offset-aware date-times")]
#[diagnostic(code(heine::collection::mixed_date_precision))]
pub(crate) struct MixedDatePrecision {
    collection: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("this date has a different precision")]
    span: SourceSpan,
    #[related]
    related: Vec<MixedDatePrecisionRelation>,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first member with this date precision")]
struct FirstDatePrecision {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first date with this precision")]
    span: SourceSpan,
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
enum MixedDatePrecisionRelation {
    #[error(transparent)]
    #[diagnostic(transparent)]
    First(#[from] FirstDatePrecision),
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
    MissingDate(#[from] Box<MissingDate>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingWeight(#[from] Box<MissingWeight>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MixedDatePrecision(#[from] Box<MixedDatePrecision>),
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
            resolve_locale(pages, config).map(|collections| (locale.clone(), collections))
        })
        .collect()
}

fn resolve_locale(
    pages: &BTreeMap<PageId, Page>,
    config: &CollectionsConfig,
) -> Result<BTreeMap<CollectionName, ResolvedCollection>, Error> {
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
        collection
            .page_ids
            .sort_by(|left, right| compare_pages(definition.order(), left, right, pages));
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
        CollectionOrder::DateAsc | CollectionOrder::DateDesc => {
            let mut first: Option<(bool, &Page)> = None;
            for id in ids {
                let page = &pages[id];
                let membership = membership(page, name);
                let Some(date) = &page.metadata.date else {
                    return Err(Box::new(MissingDate {
                        page: id.to_string(),
                        collection: name.to_string(),
                        source_code: membership.source.clone(),
                        span: membership.span,
                        order: collection_order_origin(definition).into_iter().collect(),
                        help: "add a TOML date or offset-aware date-time, or use another collection order".into(),
                    })
                    .into());
                };
                match first {
                    Some((precision, first_page)) if precision != date.is_date_only() => {
                        return Err(Box::new(MixedDatePrecision {
                            collection: name.to_string(),
                            source_code: Arc::new(page.source.clone()),
                            span: page
                                .metadata
                                .date_span
                                .expect("present dates retain their authored spans"),
                            related: vec![MixedDatePrecisionRelation::First(
                                FirstDatePrecision {
                                source_code: Arc::new(first_page.source.clone()),
                                span: first_page
                                    .metadata
                                    .date_span
                                    .expect("present dates retain their authored spans"),
                                },
                            )]
                            .into_iter()
                            .chain(
                                collection_order_origin(definition)
                                    .map(MixedDatePrecisionRelation::Order),
                            )
                            .collect(),
                            help: "use TOML dates for every member, or offset-aware date-times for every member".into(),
                        })
                        .into());
                    }
                    None => first = Some((date.is_date_only(), page)),
                    _ => {}
                }
            }
        }
        CollectionOrder::WeightAsc | CollectionOrder::WeightDesc => {
            for id in ids {
                let page = &pages[id];
                if page.weight.is_none() {
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
    order: CollectionOrder,
    left: &PageId,
    right: &PageId,
    pages: &BTreeMap<PageId, Page>,
) -> Ordering {
    let primary = match order {
        CollectionOrder::DateAsc | CollectionOrder::DateDesc => compare_dates(
            pages[left]
                .metadata
                .date
                .as_ref()
                .expect("date ordering was validated"),
            pages[right]
                .metadata
                .date
                .as_ref()
                .expect("date ordering was validated"),
        ),
        CollectionOrder::WeightAsc | CollectionOrder::WeightDesc => pages[left]
            .weight
            .expect("weight ordering was validated")
            .cmp(&pages[right].weight.expect("weight ordering was validated")),
        CollectionOrder::IdAsc | CollectionOrder::IdDesc => left.cmp(right),
    };
    let primary = match order {
        CollectionOrder::DateDesc | CollectionOrder::WeightDesc | CollectionOrder::IdDesc => {
            primary.reverse()
        }
        _ => primary,
    };
    primary.then_with(|| left.cmp(right))
}

fn compare_dates(left: &MetadataDate, right: &MetadataDate) -> Ordering {
    match (left, right) {
        (MetadataDate::Date(left), MetadataDate::Date(right)) => left.cmp(right),
        (MetadataDate::OffsetDateTime(left), MetadataDate::OffsetDateTime(right)) => {
            left.cmp(right)
        }
        _ => unreachable!("mixed date precision was validated"),
    }
}
