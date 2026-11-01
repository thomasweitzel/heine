//! Inheritance for page and directory citation settings.

use std::{ops::Range, path::Path};

use thiserror::Error;

use super::Settings;

/// A complete setting replaces the previous one. A switch is accepted only
/// to disable an inherited setting, as with the existing footnote policy.
#[derive(Debug, Clone)]
pub(in crate::site) enum Policy {
    Switch(toml::Spanned<bool>),
    Enabled(Settings),
}

pub(in crate::site) struct Declaration<'a> {
    pub(in crate::site) policy: &'a Policy,
    pub(in crate::site) path: &'a Path,
    pub(in crate::site) source: &'a str,
}

pub(in crate::site) struct Effective<'a> {
    pub(in crate::site) settings: &'a Settings,
    pub(in crate::site) path: &'a Path,
    pub(in crate::site) source: &'a str,
}

#[derive(Debug, Error)]
pub(in crate::site) enum Error {
    #[error("citation settings accept only false or a complete table")]
    TrueSwitch { path: Box<Path>, span: Range<usize> },
    #[error("citations = false requires an inherited citation setting")]
    NothingToDisable { path: Box<Path>, span: Range<usize> },
}

/// Apply declarations from the outermost directory through the page. The
/// effective value retains its author's source, even when a page inherits it.
pub(in crate::site) fn resolve<'a>(
    declarations: impl IntoIterator<Item = Declaration<'a>>,
) -> Result<Option<Effective<'a>>, Error> {
    let mut current = None;
    for declaration in declarations {
        match declaration.policy {
            Policy::Enabled(settings) => {
                current = Some(Effective {
                    settings,
                    path: declaration.path,
                    source: declaration.source,
                });
            }
            Policy::Switch(value) if *value.get_ref() => {
                return Err(Error::TrueSwitch {
                    path: declaration.path.into(),
                    span: value.span(),
                });
            }
            Policy::Switch(value) => {
                if current.take().is_none() {
                    return Err(Error::NothingToDisable {
                        path: declaration.path.into(),
                        span: value.span(),
                    });
                }
            }
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct EnabledPage {
        citations: Settings,
    }

    #[derive(Deserialize)]
    struct SwitchPage {
        citations: toml::Spanned<bool>,
    }

    #[derive(Deserialize)]
    struct Directory {
        pages: EnabledPage,
    }

    fn enabled(source: &str) -> Policy {
        Policy::Enabled(toml::from_str::<EnabledPage>(source).unwrap().citations)
    }

    fn switch(source: &str) -> Policy {
        Policy::Switch(toml::from_str::<SwitchPage>(source).unwrap().citations)
    }

    fn declaration<'a>(policy: &'a Policy, path: &'a str, source: &'a str) -> Declaration<'a> {
        Declaration {
            policy,
            path: Path::new(path),
            source,
        }
    }

    #[test]
    fn nearest_complete_setting_wins_and_retains_its_source() {
        let outer_text =
            "[citations]\ncontent='main'\nbibliography=['outer.bib']\nstyle='outer.csl'\n";
        let inner_text =
            "[citations]\ncontent='article'\nbibliography=['inner.bib']\nstyle='inner.csl'\n";
        let outer = enabled(outer_text);
        let inner = enabled(inner_text);
        let selected = resolve([
            declaration(&outer, "outer/_directory.toml", outer_text),
            declaration(&inner, "inner/_directory.toml", inner_text),
        ])
        .unwrap()
        .unwrap();
        assert_eq!(selected.path, Path::new("inner/_directory.toml"));
        assert_eq!(selected.source, inner_text);
        assert_eq!(selected.settings.content.get_ref(), "article");
        let span = selected.settings.bibliography[0].span();
        assert_eq!(&selected.source[span], "'inner.bib'");

        let directory_text =
            "[pages.citations]\ncontent='main'\nbibliography=['shared.bib']\nstyle='shared.csl'\n";
        let directory = toml::from_str::<Directory>(directory_text).unwrap();
        let declaration_policy = Policy::Enabled(directory.pages.citations);
        let selected = resolve([declaration(
            &declaration_policy,
            "_directory.toml",
            directory_text,
        )])
        .unwrap()
        .unwrap();
        assert_eq!(
            &selected.source[selected.settings.bibliography[0].span()],
            "'shared.bib'"
        );
    }

    #[test]
    fn disable_requires_an_effective_ancestor_and_can_be_followed_by_an_override() {
        let enabled_text = "[citations]\ncontent='main'\nbibliography=['a.bib']\nstyle='a.csl'\n";
        let disabled_text = "citations=false\n";
        let enabled = enabled(enabled_text);
        let disabled = switch(disabled_text);
        assert!(
            resolve([
                declaration(&enabled, "outer", enabled_text),
                declaration(&disabled, "inner", disabled_text),
            ])
            .unwrap()
            .is_none()
        );
        assert_eq!(
            resolve([
                declaration(&enabled, "outer", enabled_text),
                declaration(&disabled, "inner", disabled_text),
                declaration(&enabled, "page", enabled_text),
            ])
            .unwrap()
            .unwrap()
            .path,
            Path::new("page")
        );
        assert!(
            matches!(resolve([declaration(&disabled, "page", disabled_text)]),
            Err(Error::NothingToDisable { path, span }) if path.as_ref() == Path::new("page") && &disabled_text[span.clone()] == "false")
        );
        assert!(matches!(
            resolve([
                declaration(&enabled, "outer", enabled_text),
                declaration(&disabled, "inner", disabled_text),
                declaration(&disabled, "page", disabled_text),
            ]),
            Err(Error::NothingToDisable { .. })
        ));
    }

    #[test]
    fn true_switch_is_an_error_at_the_authored_value() {
        let source = "citations=true\n";
        let page = switch(source);
        assert!(matches!(resolve([declaration(&page, "page", source)]),
            Err(Error::TrueSwitch { path, span }) if path.as_ref() == Path::new("page") && &source[span.clone()] == "true"));
    }
}
