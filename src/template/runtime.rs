//! Private, validated services shared by Tera built-ins.

use std::sync::Arc;

use crate::{asset::AssetLinks, locale::Locale, localization::Localization, site::PageLinks};

use super::TemplateResources;

/// The only bridge from Tera's dynamic render state back to validated Heine
/// values. Built-ins use this instead of independently decoding context data.
#[derive(Clone)]
pub(super) struct TemplateRuntime {
    assets: AssetLinks,
    pages: PageLinks,
    localization: Arc<Localization>,
    highlight_css: Option<String>,
}

impl From<TemplateResources> for TemplateRuntime {
    fn from(resources: TemplateResources) -> Self {
        Self {
            assets: resources.asset_links,
            pages: resources.page_links,
            localization: resources.localization,
            highlight_css: resources.highlight_css,
        }
    }
}

impl TemplateRuntime {
    pub(super) fn assets(&self) -> &AssetLinks {
        &self.assets
    }

    pub(super) fn pages(&self) -> &PageLinks {
        &self.pages
    }

    pub(super) fn localization(&self) -> &Localization {
        &self.localization
    }

    pub(super) fn highlight_css(&self) -> Option<&str> {
        self.highlight_css.as_deref()
    }

    pub(super) fn locale(&self, state: &tera::State) -> tera::TeraResult<Locale> {
        let code = self.runtime_string(state, "locale")?;
        code.parse()
            .map_err(|_| tera::Error::message("Heine render context has an invalid locale code"))
    }

    pub(super) fn page_dir(&self, state: &tera::State) -> tera::TeraResult<String> {
        self.runtime_string(state, "page_dir")
    }

    pub(super) fn root(&self, state: &tera::State) -> tera::TeraResult<String> {
        self.runtime_string(state, "root")
    }

    fn runtime_string(&self, state: &tera::State, field: &str) -> tera::TeraResult<String> {
        state
            .get::<tera::Value>("__heine_runtime")?
            .and_then(|runtime| runtime.get_from_path(field)?.as_str().map(str::to_owned))
            .ok_or_else(|| {
                tera::Error::message(format!("Heine internal render context has no {field}"))
            })
    }
}
