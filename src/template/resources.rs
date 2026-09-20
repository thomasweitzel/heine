//! Template built-ins that resolve authored pages and copied assets.

use std::path::Path;

use super::{arguments, runtime::TemplateRuntime};

const MAX_DIAGNOSTIC_NAMES: usize = 10;

pub(super) fn register(tera: &mut tera::Tera, runtime: &TemplateRuntime) {
    let asset_runtime = runtime.clone();
    tera.register_function(
        "asset",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            asset_value(&asset_runtime, kwargs, state, false)
        },
    );

    // Kit sources rewrite their own `asset()` calls to this same-length
    // internal name before Tera parses them. Root sources cannot spell it,
    // and their ordinary `asset()` calls reject private kit assets.
    let kit_asset_runtime = runtime.clone();
    tera.register_function(
        "__a__",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            asset_value(&kit_asset_runtime, kwargs, state, true)
        },
    );

    let attribution_runtime = runtime.clone();
    tera.register_function(
        "attributions",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("attributions", &kwargs, &[])?;
            let root = attribution_runtime.root(state)?;
            tera::Value::try_from_serializable(&attribution_runtime.assets().attributions(&root))
                .map_err(|_| tera::Error::message("Heine attribution view is not serializable"))
        },
    );

    let page_runtime = runtime.clone();
    tera.register_function(
        "page",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("page", &kwargs, &["id", "locale"])?;
            let id = kwargs.must_get::<&str>("id")?;
            let locale = kwargs.get::<&str>("locale")?;
            let target = page_runtime
                .pages()
                .with_locale(page_runtime.locale(state)?)
                .resolve(id, locale)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = page_runtime.root(state)?;
            page_value(&page_runtime, target, &root)
        },
    );

    let page_exists_runtime = runtime.clone();
    tera.register_function(
        "page_exists",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("page_exists", &kwargs, &["id", "locale"])?;
            let id = kwargs.must_get::<&str>("id")?;
            let result = page_exists_runtime
                .pages()
                .with_locale(page_exists_runtime.locale(state)?)
                .resolve(id, kwargs.get::<&str>("locale")?);
            match result {
                Ok(_) => Ok(true.into()),
                Err(
                    crate::site::PageResolveError::NotFound { .. }
                    | crate::site::PageResolveError::DraftExcluded { .. },
                ) => Ok(false.into()),
                Err(error) => Err(tera::Error::message(error.to_string())),
            }
        },
    );

    let feed_runtime = runtime.clone();
    tera.register_function(
        "feed",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("feed", &kwargs, &["name", "locale"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = feed_runtime
                .pages()
                .with_locale(feed_runtime.locale(state)?)
                .resolve_locale(kwargs.get::<&str>("locale")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = feed_runtime.root(state)?;
            feed_runtime
                .feed(&locale, name, &root)
                .map(|feed| {
                    tera::Value::try_from_serializable(&feed)
                        .map_err(|_| tera::Error::message("Heine feed view is not serializable"))
                })
                .transpose()?
                .ok_or_else(|| {
                    let available = feed_runtime.feed_names(&locale);
                    let detail = available_names("no feeds are configured", "feeds", &available);
                    tera::Error::message(format!("feed {name:?} was not found; {detail}"))
                })
        },
    );

    let search_runtime = runtime.clone();
    tera.register_function(
        "search",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("search", &kwargs, &["name", "locale"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = search_runtime
                .pages()
                .with_locale(search_runtime.locale(state)?)
                .resolve_locale(kwargs.get::<&str>("locale")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = search_runtime.root(state)?;
            search_runtime
                .search(&locale, name, &root)
                .map(|index| {
                    tera::Value::try_from_serializable(&index)
                        .map_err(|_| tera::Error::message("Heine search view is not serializable"))
                })
                .transpose()?
                .ok_or_else(|| {
                    let available = search_runtime.search_names(&locale);
                    let detail = available_names(
                        "no search indexes are configured",
                        "search indexes",
                        &available,
                    );
                    tera::Error::message(format!("search index {name:?} was not found; {detail}"))
                })
        },
    );

    let collection_runtime = runtime.clone();
    tera.register_function(
        "collection",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("collection", &kwargs, &["name"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = collection_runtime.locale(state)?;
            collection_runtime
                .collection(&locale, name)
                .map(|collection| {
                    tera::Value::try_from_serializable(&collection).map_err(|_| {
                        tera::Error::message("Heine collection view is not serializable")
                    })
                })
                .transpose()?
                .ok_or_else(|| {
                    let available = collection_runtime.collection_names(&locale);
                    let detail =
                        available_names("no collections are configured", "collections", &available);
                    tera::Error::message(format!("collection {name:?} was not found; {detail}"))
                })
        },
    );

    let series_runtime = runtime.clone();
    tera.register_function(
        "series",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("series", &kwargs, &["id", "locale"])?;
            if !series_runtime.series().configured() {
                return Err(tera::Error::message(
                    "series() requires a [series] table in heine.toml",
                ));
            }
            let locale = series_runtime
                .pages()
                .with_locale(series_runtime.locale(state)?)
                .resolve_locale(kwargs.get::<&str>("locale")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = series_runtime.root(state)?;
            match kwargs.get::<&str>("id")? {
                None => series_runtime.series_index(&locale, &root),
                Some(id) => {
                    let name = crate::config::SeriesName::parse(id.to_owned())
                        .map_err(tera::Error::message)?;
                    if !series_runtime.series().definition_exists(&name) {
                        return Err(tera::Error::message(format!(
                            "series {id:?} is not configured"
                        )));
                    }
                    Some(series_runtime
                        .series_view(&locale, &name, &root)
                        .ok_or_else(|| {
                            tera::Error::message(format!(
                                "series {id:?} has no members for locale {locale}; use series_has_members() before an optional lookup"
                            ))
                        })?)
                }
            }
            .map(|value| {
                tera::Value::try_from_serializable(&value)
                    .map_err(|_| tera::Error::message("Heine Series view is not serializable"))
            })
            .transpose()?
            .ok_or_else(|| {
                let available = series_runtime.series().names(&locale);
                let detail = available_names("no Series have local members", "Series", &available);
                tera::Error::message(format!("requested Series was not found; {detail}"))
            })
        },
    );

    let series_has_members_runtime = runtime.clone();
    tera.register_function(
        "series_has_members",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("series_has_members", &kwargs, &["id", "locale"])?;
            if !series_has_members_runtime.series().configured() {
                return Err(tera::Error::message(
                    "series_has_members() requires a [series] table in heine.toml",
                ));
            }
            let id = kwargs.must_get::<&str>("id")?;
            let name =
                crate::config::SeriesName::parse(id.to_owned()).map_err(tera::Error::message)?;
            let locale = series_has_members_runtime
                .pages()
                .with_locale(series_has_members_runtime.locale(state)?)
                .resolve_locale(kwargs.get::<&str>("locale")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            if !series_has_members_runtime.series().definition_exists(&name) {
                return Err(tera::Error::message(format!(
                    "series {id:?} is not configured"
                )));
            }
            Ok(series_has_members_runtime
                .series()
                .has_members(&locale, &name)
                .into())
        },
    );

    let taxonomy_runtime = runtime.clone();
    tera.register_function(
        "taxonomy",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("taxonomy", &kwargs, &["name", "locale"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = taxonomy_runtime
                .pages()
                .with_locale(taxonomy_runtime.locale(state)?)
                .resolve_locale(kwargs.get::<&str>("locale")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = taxonomy_runtime.root(state)?;
            taxonomy_runtime
                .taxonomy_index(&locale, name, &root)
                .map_err(tera::Error::message)?
                .map(|taxonomy| {
                    tera::Value::try_from_serializable(&taxonomy).map_err(|_| {
                        tera::Error::message("Heine taxonomy index view is not serializable")
                    })
                })
                .transpose()?
                .ok_or_else(|| {
                    let available = taxonomy_runtime.taxonomy_names(&locale);
                    let detail =
                        available_names("no taxonomies are configured", "taxonomies", &available);
                    tera::Error::message(format!("taxonomy {name:?} was not found; {detail}"))
                })
        },
    );

    let pagination_runtime = runtime.clone();
    tera.register_function(
        "pagination_links",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("pagination_links", &kwargs, &[])?;
            let pager = pagination_runtime.pager(state)?.ok_or_else(|| {
                tera::Error::message(
                    "pagination_links() is available only while rendering a paginated listing",
                )
            })?;
            let root = pagination_runtime.root(state)?;
            tera::Value::try_from_serializable(&pager.links(
                pagination_runtime.pages().base_path(),
                pagination_runtime.pages().canonical_urls(),
                &root,
            ))
            .map_err(|_| tera::Error::message("Heine pagination links are not serializable"))
        },
    );

    let siblings_runtime = runtime.clone();
    tera.register_function(
        "siblings",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("siblings", &kwargs, &["id", "locale"])?;
            let current_locale = siblings_runtime.locale(state)?;
            let requested_locale = kwargs.get::<&str>("locale")?;
            let (locale, directory, page) = match kwargs.get::<&str>("id")? {
                Some(id) => {
                    let target = siblings_runtime
                        .pages()
                        .with_locale(current_locale)
                        .resolve(id, requested_locale)
                        .map_err(|error| tera::Error::message(error.to_string()))?;
                    let page = siblings_runtime
                        .page_view(&target.locale, &target.id)
                        .ok_or_else(|| {
                            tera::Error::message("Heine page catalog is inconsistent")
                        })?;
                    (target.locale, page.directory().to_owned(), target.id)
                }
                None if requested_locale.is_some() => {
                    return Err(tera::Error::message(
                        "siblings() accepts locale= only together with id=",
                    ));
                }
                None => (
                    current_locale,
                    siblings_runtime.page_dir(state)?,
                    siblings_runtime.page_id(state)?,
                ),
            };
            tera::Value::try_from_serializable(
                &siblings_runtime.siblings(&locale, &directory, &page),
            )
            .map_err(|_| tera::Error::message("Heine sibling view is not serializable"))
        },
    );

    let sibling_neighbors_runtime = runtime.clone();
    tera.register_function(
        "sibling_neighbors",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("sibling_neighbors", &kwargs, &[])?;
            let locale = sibling_neighbors_runtime.locale(state)?;
            let directory = sibling_neighbors_runtime.page_dir(state)?;
            let page = sibling_neighbors_runtime.page_id(state)?;
            let neighbors = sibling_neighbors_runtime
                .sibling_neighbors(&locale, &directory, &page)
                .ok_or_else(|| {
                    tera::Error::message(format!(
                        "page {page:?} does not participate in sibling ordering; give it a numeric weight"
                    ))
                })?;
            let root = sibling_neighbors_runtime.root(state)?;
            neighbor_value(&sibling_neighbors_runtime, &locale, neighbors, &root)
        },
    );

    let collection_neighbors_runtime = runtime.clone();
    tera.register_function(
        "collection_neighbors",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("collection_neighbors", &kwargs, &["name"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = collection_neighbors_runtime.locale(state)?;
            if collection_neighbors_runtime.collection(&locale, name).is_none() {
                let available = collection_neighbors_runtime.collection_names(&locale);
                let detail = available_names("no collections are configured", "collections", &available);
                return Err(tera::Error::message(format!(
                    "collection {name:?} was not found; {detail}"
                )));
            }
            let page = collection_neighbors_runtime.page_id(state)?;
            let neighbors = collection_neighbors_runtime
                .collection_neighbors(&locale, name, &page)
                .ok_or_else(|| {
                    tera::Error::message(format!(
                        "page {page:?} is not a member of collection {name:?}; add it to the collection before requesting its neighbors"
                    ))
                })?;
            let root = collection_neighbors_runtime.root(state)?;
            neighbor_value(&collection_neighbors_runtime, &locale, neighbors, &root)
        },
    );

    let latest_runtime = runtime.clone();
    tera.register_filter(
        "latest",
        move |input: &str, kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<String> {
            arguments::only("latest", &kwargs, &[])?;
            let page_dir = latest_runtime.page_dir(state)?;
            let page_dir = Path::new(&page_dir);
            let (asset, content_root_relative) = latest_runtime
                .assets()
                .with_locale(latest_runtime.locale(state)?)
                .resolve_template_path(page_dir, input)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            if content_root_relative {
                Ok(latest_runtime
                    .pages()
                    .base_path()
                    .join(&asset.output.url_path()))
            } else {
                Ok(asset.relative_url_path(page_dir).as_str().to_owned())
            }
        },
    );

    let dimensions_runtime = runtime.clone();
    tera.register_filter(
        "dimensions",
        move |input: &str,
              kwargs: tera::Kwargs,
              state: &tera::State|
              -> tera::TeraResult<tera::Value> {
            arguments::only("dimensions", &kwargs, &[])?;
            let page_dir = dimensions_runtime.page_dir(state)?;
            let links = dimensions_runtime
                .assets()
                .with_locale(dimensions_runtime.locale(state)?);
            let (asset, _) = links
                .resolve_template_path(Path::new(&page_dir), input)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let mut result = tera::Map::new();
            if let Some((width, height)) = links.dimensions(&asset.output) {
                result.insert("width".into(), width.into());
                result.insert("height".into(), height.into());
            }
            Ok(result.into())
        },
    );
}

fn asset_value(
    runtime: &TemplateRuntime,
    kwargs: tera::Kwargs,
    state: &tera::State,
    include_private_assets: bool,
) -> tera::TeraResult<tera::Value> {
    arguments::only("asset", &kwargs, &["id"])?;
    let locale = runtime.locale(state)?;
    let assets = runtime.assets().with_locale(locale);
    let id = kwargs.must_get::<&str>("id")?;
    let asset = if include_private_assets {
        assets.resolve_for_kit(id)
    } else {
        assets.resolve(id)
    }
    .map_err(|error| tera::Error::message(error.to_string()))?;
    let root = runtime.root(state)?;
    let mut result = tera::Map::new();
    result.insert("url".into(), asset.url().into());
    if let Some(url) = asset.absolute_url() {
        result.insert("absolute_url".into(), url.into());
    }
    result.insert(
        "relative_url".into(),
        format!("{root}{}", asset.path().url_path().as_str()).into(),
    );
    result.insert(
        "integrity".into(),
        asset.integrity().map(str::to_owned).into(),
    );
    let (width, height) = asset.dimensions().unzip();
    result.insert("width".into(), width.into());
    result.insert("height".into(), height.into());
    result.insert(
        "licensing".into(),
        asset
            .licensing_view(
                &root,
                runtime.assets().base_path(),
                runtime.assets().canonical_urls(),
            )
            .map(|value| tera::Value::try_from_serializable(&value))
            .transpose()
            .map_err(|_| tera::Error::message("Heine asset licensing view is not serializable"))?
            .into(),
    );
    Ok(result.into())
}

fn neighbor_value(
    runtime: &TemplateRuntime,
    locale: &crate::locale::Locale,
    neighbors: crate::site::Neighbors,
    root: &str,
) -> tera::TeraResult<tera::Value> {
    let mut value = tera::Map::new();
    for (name, id) in [("before", neighbors.before), ("after", neighbors.after)] {
        let page = id
            .map(|id| page_target(runtime, locale, id))
            .transpose()?
            .map(|target| page_value(runtime, target, root))
            .transpose()?;
        value.insert(name.into(), page.into());
    }
    Ok(value.into())
}

fn page_target(
    runtime: &TemplateRuntime,
    locale: &crate::locale::Locale,
    id: crate::site::PageId,
) -> tera::TeraResult<crate::site::PageLinkTarget> {
    runtime
        .pages()
        .target(locale, &id)
        .ok_or_else(|| tera::Error::message("Heine page relationship is inconsistent"))
}

fn page_value(
    runtime: &TemplateRuntime,
    target: crate::site::PageLinkTarget,
    root: &str,
) -> tera::TeraResult<tera::Value> {
    let page = runtime
        .page_view(&target.locale, &target.id)
        .ok_or_else(|| tera::Error::message("Heine page catalog is inconsistent"))?;
    let mut result = tera::Value::try_from_serializable(&page)
        .map_err(|_| tera::Error::message("Heine page view is not serializable"))?
        .into_map()
        .ok_or_else(|| tera::Error::message("Heine page view is not a map"))?;
    result.insert("url".into(), target.url.into());
    result.insert(
        "relative_url".into(),
        format!("{root}{}", target.output_path.url_path().as_str()).into(),
    );
    result.insert(
        "taxonomies".into(),
        tera::Value::try_from_serializable(&runtime.page_terms(&target.locale, &target.id, root))
            .map_err(|_| tera::Error::message("Heine taxonomy terms are not serializable"))?,
    );
    Ok(result.into())
}

fn available_names(empty: &str, kind: &str, names: &[String]) -> String {
    if names.is_empty() {
        return empty.into();
    }
    let shown = names
        .iter()
        .take(MAX_DIAGNOSTIC_NAMES)
        .map(String::as_str)
        .collect::<Vec<_>>();
    let omitted = names.len().saturating_sub(shown.len());
    let suffix = (omitted > 0).then(|| format!("; and {omitted} more"));
    format!(
        "available {kind}: {}{}",
        shown.join(", "),
        suffix.unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_available_names_without_hiding_that_more_exist() {
        let names = (1..=12)
            .map(|number| format!("tag-{number}"))
            .collect::<Vec<_>>();
        let message = available_names("no tags are defined in this locale", "tags", &names);

        assert!(message.contains("tag-1"));
        assert!(message.contains("tag-10"));
        assert!(!message.contains("tag-11"));
        assert!(message.contains("and 2 more"));
    }
}
