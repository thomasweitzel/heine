//! Deterministic page indexes and relationship validation.

use std::collections::BTreeMap;

use crate::site::page::{Page, PageId};

pub(super) fn fix_siblings(pages: &mut BTreeMap<PageId, Page>) {
    let members: BTreeMap<String, Vec<PageId>> =
        pages
            .iter()
            .fold(BTreeMap::new(), |mut directories, (id, page)| {
                directories
                    .entry(page.dir.clone())
                    .or_default()
                    .push(id.clone());
                directories
            });
    let eligible: BTreeMap<String, Vec<PageId>> = pages
        .iter()
        .filter(|(_, page)| page.path.is_some() && page.weight.is_some())
        .fold(BTreeMap::new(), |mut dirs, (id, page)| {
            dirs.entry(page.dir.clone()).or_default().push(id.clone());
            dirs
        });
    for (directory, ids) in eligible {
        let mut ids = ids.clone();
        ids.sort_by_key(|id| (pages[id].weight, id.clone()));
        for id in &members[&directory] {
            let page = &mut pages.get_mut(id).expect("page exists");
            if let Some(index) = ids.iter().position(|other| other == id) {
                page.siblings = ids.iter().filter(|other| *other != id).cloned().collect();
                page.lighter = index.checked_sub(1).map(|index| ids[index].clone());
                page.heavier = ids.get(index + 1).cloned();
            } else {
                page.siblings = ids.clone();
                page.lighter = None;
                page.heavier = None;
            }
        }
    }
}

pub(super) fn tags_for(pages: &BTreeMap<PageId, Page>) -> BTreeMap<String, Vec<PageId>> {
    let mut tags: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (id, page) in pages {
        for tag in &page.tags {
            tags.entry(tag.clone()).or_default().push(id.clone());
        }
    }
    for ids in tags.values_mut() {
        ids.sort_by_key(|id| (pages[id].weight, id.clone()));
    }
    tags
}
