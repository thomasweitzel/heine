//! Resolved generated listing tasks for collection pagination.
//!
//! Pagination consumes the already validated collection relationship. It owns
//! no page identity and writes no output; it only expands authored listing
//! pages into deterministic render tasks that the output domain later checks.

use std::{collections::BTreeMap, sync::Arc};

use serde::Serialize;

use crate::{
    config::{BasePath, CanonicalUrls},
    locale::Locale,
    output::OutputPath,
};

use super::{
    PageCatalog, PageView,
    collection::ResolvedCollection,
    page::{Page, PageId, Pager, Task},
};

/// Indexed pager data used by the exceptional `pagination_links()` template
/// function. It is held at the template boundary rather than serialized into
/// every render context.
#[derive(Debug, Clone, Default)]
pub(crate) struct Catalog {
    pagers: Arc<BTreeMap<(Locale, OutputPath), Arc<Pager>>>,
}

impl Catalog {
    pub(super) fn from_tasks(
        tasks: &[Task],
        taxonomy_tasks: &[super::taxonomy::Task],
        series_tasks: &[super::series::Task],
    ) -> Self {
        Self {
            pagers: Arc::new(
                tasks
                    .iter()
                    .filter_map(|task| {
                        task.pagination.as_ref().map(|pager| {
                            (
                                (task.locale.clone(), task.output_path.clone()),
                                pager.clone(),
                            )
                        })
                    })
                    .chain(taxonomy_tasks.iter().filter_map(|task| {
                        task.pagination.as_ref().map(|pager| {
                            (
                                (task.locale.clone(), task.output_path.clone()),
                                pager.clone(),
                            )
                        })
                    }))
                    .chain(series_tasks.iter().filter_map(|task| {
                        task.pagination.as_ref().map(|pager| {
                            (
                                (task.locale.clone(), task.output_path.clone()),
                                pager.clone(),
                            )
                        })
                    }))
                    .collect(),
            ),
        }
    }

    pub(crate) fn pager(&self, locale: &Locale, output_path: &OutputPath) -> Option<&Pager> {
        self.pagers
            .get(&(locale.clone(), output_path.clone()))
            .map(AsRef::as_ref)
    }
}

/// Expands every validated authored pagination declaration after collections
/// have been resolved. The original task becomes pager one; all later pagers
/// are additional generated tasks with the same authored page identity.
pub(super) fn expand(
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    collections: &BTreeMap<Locale, BTreeMap<crate::config::CollectionName, ResolvedCollection>>,
    tasks: &mut Vec<Task>,
) {
    let latest_inputs = tasks
        .iter()
        .map(|task| {
            (
                (task.locale.clone(), task.page_id.clone()),
                task.latest_input,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut expanded = Vec::new();

    for task in tasks.iter_mut() {
        let page = &pages[&task.locale][&task.page_id];
        let Some(declaration) = &page.pagination else {
            continue;
        };
        let members = collections[&task.locale][&declaration.collection].page_ids();
        let page_count = members.len().div_ceil(declaration.per_page).max(1);
        let paths = Arc::new(
            std::iter::once(task.output_path.clone())
                .chain((2..=page_count).map(|number| {
                    output_path(
                        task.output_path.as_path(),
                        task.page_id.is_index(),
                        declaration.segment.as_str(),
                        number,
                    )
                }))
                .collect::<Vec<_>>(),
        );
        let latest_input = members.iter().fold(task.latest_input, |latest, id| {
            latest.max(latest_inputs[&(task.locale.clone(), id.clone())])
        });
        task.latest_input = latest_input;
        task.pagination = Some(Arc::new(pager(
            declaration,
            members,
            1,
            page_count,
            paths.clone(),
        )));
        for number in 2..=page_count {
            let mut generated = task.clone();
            generated.output_path = paths[number - 1].clone();
            generated.output_span = declaration.route_span;
            generated.latest_input = latest_input;
            generated.pagination = Some(Arc::new(pager(
                declaration,
                members,
                number,
                page_count,
                paths.clone(),
            )));
            expanded.push(generated);
        }
    }
    tasks.extend(expanded);
    tasks.sort_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then(left.page_id.cmp(&right.page_id))
            .then(left.output_path.cmp(&right.output_path))
    });
}

fn pager(
    declaration: &super::page::Pagination,
    members: &[PageId],
    page_number: usize,
    page_count: usize,
    paths: Arc<Vec<OutputPath>>,
) -> Pager {
    let start = (page_number - 1) * declaration.per_page;
    let end = (start + declaration.per_page).min(members.len());
    Pager {
        collection: declaration.collection.to_string(),
        page_ids: members[start..end].to_vec(),
        page_number,
        page_count,
        item_count: members.len(),
        per_page: declaration.per_page,
        navigation_window: declaration.navigation_window,
        paths,
    }
}

fn output_path(base: &std::path::Path, is_index: bool, segment: &str, number: usize) -> OutputPath {
    let parent = base.parent().unwrap_or(std::path::Path::new(""));
    let parent = if is_index {
        parent.into()
    } else {
        parent.join(
            base.file_stem()
                .expect("rendered page outputs always have a validated filename"),
        )
    };
    let extension = base
        .extension()
        .expect("rendered page outputs always have a validated suffix")
        .to_string_lossy();
    OutputPath::try_new(parent.join(segment).join(format!("{number}.{extension}")))
        .expect("pagination routes derive from validated output and segment components")
}

#[derive(Debug, Serialize)]
pub(crate) struct View<'a> {
    collection: String,
    pages: Vec<ListingPage<'a>>,
    item_count: usize,
    per_page: usize,
    first_item_number: usize,
    page_number: usize,
    page_count: usize,
    current: Link,
    first: Link,
    last: Link,
    previous: Option<Link>,
    next: Option<Link>,
    navigation: Vec<NavigationItem>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Link {
    page_number: usize,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    absolute_url: Option<String>,
    relative_url: String,
    current: bool,
}

/// A selected page view augmented with URLs relative to the current pager.
/// `PageView` retains its own canonical address, but a generated listing owns
/// the base-path and relative links appropriate to the listing output.
#[derive(Debug, Serialize)]
struct ListingPage<'a> {
    #[serde(flatten)]
    page: &'a PageView,
    url: String,
    relative_url: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum NavigationItem {
    Link(Link),
    Gap,
}

impl Pager {
    pub(crate) fn view<'a>(
        &self,
        catalog: &'a PageCatalog,
        locale: &Locale,
        base_path: &BasePath,
        canonical_urls: Option<&CanonicalUrls>,
        root: &str,
    ) -> View<'a> {
        let pages = self
            .page_ids
            .iter()
            .map(|id| {
                let page = catalog
                    .view(locale, id)
                    .expect("resolved collection IDs remain in page catalog");
                let path = catalog
                    .output_path(locale, id)
                    .expect("collection members are validated as rendered pages");
                ListingPage {
                    page,
                    url: base_path.join(&path.url_path()),
                    relative_url: format!("{root}{}", path.url_path().as_str()),
                }
            })
            .collect();
        let navigation = navigation_links(
            self.page_number,
            self.page_count,
            self.navigation_window.neighbors(),
        )
        .into_iter()
        .map(|number| match number {
            Some(number) => {
                NavigationItem::Link(self.link(number, base_path, canonical_urls, root))
            }
            None => NavigationItem::Gap,
        })
        .collect();
        View {
            collection: self.collection.clone(),
            pages,
            item_count: self.item_count,
            per_page: self.per_page,
            first_item_number: if self.item_count == 0 {
                0
            } else {
                (self.page_number - 1) * self.per_page + 1
            },
            page_number: self.page_number,
            page_count: self.page_count,
            current: self.link(self.page_number, base_path, canonical_urls, root),
            first: self.link(1, base_path, canonical_urls, root),
            last: self.link(self.page_count, base_path, canonical_urls, root),
            previous: self
                .page_number
                .checked_sub(2)
                .map(|index| self.link(index + 1, base_path, canonical_urls, root)),
            next: (self.page_number < self.page_count)
                .then(|| self.link(self.page_number + 1, base_path, canonical_urls, root)),
            navigation,
        }
    }

    pub(crate) fn links(
        &self,
        base_path: &BasePath,
        canonical_urls: Option<&CanonicalUrls>,
        root: &str,
    ) -> Vec<Link> {
        (1..=self.page_count)
            .map(|number| self.link(number, base_path, canonical_urls, root))
            .collect()
    }

    fn link(
        &self,
        page_number: usize,
        base_path: &BasePath,
        canonical_urls: Option<&CanonicalUrls>,
        root: &str,
    ) -> Link {
        let path = &self.paths[page_number - 1];
        Link {
            page_number,
            url: base_path.join(&path.url_path()),
            absolute_url: canonical_urls.map(|urls| urls.output_url(path)),
            relative_url: format!("{root}{}", path.url_path().as_str()),
            current: page_number == self.page_number,
        }
    }
}

fn navigation_links(current: usize, total: usize, window: usize) -> Vec<Option<usize>> {
    let mut retained = std::collections::BTreeSet::new();
    retained.insert(1);
    retained
        .extend(current.saturating_sub(window).max(1)..=current.saturating_add(window).min(total));
    retained.insert(total);
    let mut result = Vec::new();
    let mut previous = None;
    for number in retained {
        if previous.is_some_and(|previous| number > previous + 1) {
            result.push(None);
        }
        result.push(Some(number));
        previous = Some(number);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::navigation_links;

    #[test]
    fn navigation_merges_small_ranges_without_duplicate_links_or_gaps() {
        assert_eq!(navigation_links(1, 1, 1), vec![Some(1)]);
        assert_eq!(navigation_links(1, 2, 1), vec![Some(1), Some(2)]);
        assert_eq!(
            navigation_links(2, 4, 1),
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
    }

    #[test]
    fn navigation_keeps_edges_and_the_current_window() {
        assert_eq!(
            navigation_links(101, 1202, 1),
            vec![
                Some(1),
                None,
                Some(100),
                Some(101),
                Some(102),
                None,
                Some(1202)
            ]
        );
        assert_eq!(
            navigation_links(101, 1202, 2),
            vec![
                Some(1),
                None,
                Some(99),
                Some(100),
                Some(101),
                Some(102),
                Some(103),
                None,
                Some(1202)
            ]
        );
        assert_eq!(
            navigation_links(101, 1202, 3),
            vec![
                Some(1),
                None,
                Some(98),
                Some(99),
                Some(100),
                Some(101),
                Some(102),
                Some(103),
                Some(104),
                None,
                Some(1202)
            ]
        );
    }
}
