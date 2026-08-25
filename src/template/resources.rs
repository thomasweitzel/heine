//! Template built-ins that resolve authored pages and copied assets.

use std::path::Path;

use super::runtime::TemplateRuntime;

pub(super) fn register(tera: &mut tera::Tera, runtime: &TemplateRuntime) {
    let asset_runtime = runtime.clone();
    tera.register_function(
        "asset",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
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
                format!("{root}{}", asset.path().to_string_lossy()).into(),
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
            let id = kwargs.must_get::<&str>("id")?;
            let locale = kwargs.get::<&str>("locale")?;
            let target = page_runtime
                .pages()
                .with_locale(page_runtime.locale(state)?)
                .resolve(id, locale)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let root = page_runtime.root(state)?;
            let mut result = tera::Map::new();
            result.insert("url".into(), target.url.into());
            result.insert(
                "relative_url".into(),
                format!("{root}{}", target.output_path.to_string_lossy()).into(),
            );
            Ok(result.into())
        },
    );

    let latest_runtime = runtime.clone();
    tera.register_filter(
        "latest",
        move |input: &str, _: tera::Kwargs, state: &tera::State| -> tera::TeraResult<String> {
            let page_dir = latest_runtime.page_dir(state)?;
            let page_dir = Path::new(&page_dir);
            let (path, content_root_relative) = latest_runtime
                .assets()
                .with_locale(latest_runtime.locale(state)?)
                .resolve_template_path(page_dir, input)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let path = if content_root_relative {
                crate::source_path::url(&path)
            } else {
                crate::source_path::url(path.strip_prefix(page_dir).unwrap_or(&path))
            };
            Ok(if content_root_relative {
                format!("{}{}", latest_runtime.pages().base_path(), path)
            } else {
                path
            })
        },
    );

    let dimensions_runtime = runtime.clone();
    tera.register_filter(
        "dimensions",
        move |input: &str, _: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            let page_dir = dimensions_runtime.page_dir(state)?;
            let links = dimensions_runtime
                .assets()
                .with_locale(dimensions_runtime.locale(state)?);
            let (key, _) = links
                .resolve_template_path(Path::new(&page_dir), input)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            let mut result = tera::Map::new();
            if let Some((width, height)) = links.dimensions(&key) {
                result.insert("width".into(), width.into());
                result.insert("height".into(), height.into());
            }
            Ok(result.into())
        },
    );
}
