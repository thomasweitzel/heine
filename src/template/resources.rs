//! Template built-ins that resolve authored pages and copied assets.

use std::path::Path;

use super::{arguments, runtime::TemplateRuntime};

const MAX_DIAGNOSTIC_NAMES: usize = 10;

pub(super) fn register(tera: &mut tera::Tera, runtime: &TemplateRuntime) {
    let asset_runtime = runtime.clone();
    tera.register_function(
        "asset",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("asset", &kwargs, &["id"])?;
            let locale = asset_runtime.locale(state)?;
            let asset = asset_runtime
                .assets()
                .with_locale(locale)
                .resolve(kwargs.must_get::<&str>("id")?)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = asset_runtime.root(state)?;
            let mut result = tera::Map::new();
            result.insert("url".into(), asset.url().into());
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
            Ok(result.into())
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
            let page = page_runtime
                .page_view(&target.locale, &target.id)
                .ok_or_else(|| tera::Error::message("Heine page catalog is inconsistent"))?;
            let root = page_runtime.root(state)?;
            let mut result = match tera::Value::try_from_serializable(&page)
                .map_err(|_| tera::Error::message("Heine page view is not serializable"))?
                .into_map()
            {
                Some(values) => values,
                None => return Err(tera::Error::message("Heine page view is not a map")),
            };
            result.insert("url".into(), target.url.into());
            result.insert(
                "relative_url".into(),
                format!("{root}{}", target.output_path.url_path().as_str()).into(),
            );
            Ok(result.into())
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
                Err(crate::site::PageResolveError::NotFound { .. }) => Ok(false.into()),
                Err(error) => Err(tera::Error::message(error.to_string())),
            }
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

    let tag_runtime = runtime.clone();
    tera.register_function(
        "tag",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("tag", &kwargs, &["name"])?;
            let name = kwargs.must_get::<&str>("name")?;
            let locale = tag_runtime.locale(state)?;
            tag_runtime
                .tag(&locale, name)
                .map(|tag| {
                    tera::Value::try_from_serializable(&tag)
                        .map_err(|_| tera::Error::message("Heine tag view is not serializable"))
                })
                .transpose()?
                .ok_or_else(|| {
                    let available = tag_runtime.tag_names(&locale);
                    let detail =
                        available_names("no tags are defined in this locale", "tags", &available);
                    tera::Error::message(format!("tag {name:?} was not found; {detail}"))
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
            tera::Value::try_from_serializable(
                &pager.links(pagination_runtime.pages().base_path(), &root),
            )
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
