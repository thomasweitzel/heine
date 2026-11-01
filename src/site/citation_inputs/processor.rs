//! Format a page's checked citation sequence with Hayagriva.

use std::{ops::Range, sync::OnceLock};

use hayagriva::{
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
    LocatorPayload, SpecificLocator,
    citationberg::{Collapse, Locale, LocaleCode, Style, StyleClass, taxonomy::Locator},
};
use thiserror::Error;

use super::{PagePlan, output};
use crate::markdown::citation::LocatorKind;

#[derive(Debug, Clone)]
pub(super) struct FormattedPage {
    pub(super) citations: Vec<(Range<usize>, String, String)>,
    pub(super) bibliography: String,
}

#[derive(Debug, Error)]
pub(in crate::site) enum Error {
    #[error("cannot convert bibliography for CSL: {0}")]
    Conversion(biblatex::TypeError),
    #[error("note-based CSL styles are not supported")]
    NoteStyle,
    #[error("the selected CSL style has no bibliography")]
    NoBibliography,
    #[error("{0}")]
    Position(#[from] super::style::Error),
    #[error("this style can strip periods from a citation locator")]
    LocatorPeriods { span: Range<usize> },
    #[error("citation group has a locator that cannot yet be formatted reliably")]
    GroupedLocator { span: Range<usize> },
    #[error("citation key {key:?} could not be converted")]
    MissingKey { key: String, span: Range<usize> },
    #[error("unsupported CSL output: {0}")]
    Output(&'static str),
}

impl Error {
    pub(in crate::site) fn bibliography_span(&self) -> Option<Range<usize>> {
        match self {
            Self::Conversion(error) => Some(error.span.clone()),
            _ => None,
        }
    }

    pub(in crate::site) fn citation_span(&self) -> Option<Range<usize>> {
        match self {
            Self::LocatorPeriods { span }
            | Self::GroupedLocator { span }
            | Self::MissingKey { span, .. } => Some(span.clone()),
            _ => None,
        }
    }
}

fn locales() -> &'static [Locale] {
    static LOCALES: OnceLock<Vec<Locale>> = OnceLock::new();
    LOCALES.get_or_init(hayagriva::archive::locales)
}

fn convert_bibliography(
    bibliography: &biblatex::Bibliography,
) -> Result<hayagriva::Library, biblatex::TypeError> {
    bibliography.iter().map(TryInto::try_into).collect()
}

pub(super) fn format(
    plan: &PagePlan,
    source: &str,
    page_locale: &str,
) -> Result<FormattedPage, Error> {
    let style = match (&plan.inputs.style, &plan.inputs.parent) {
        (Style::Independent(style), _) | (Style::Dependent(_), Some(Style::Independent(style))) => {
            style
        }
        _ => return Err(Error::NoBibliography),
    };
    let style_facts = plan
        .inputs
        .positions
        .get_or_init(|| super::style::check(style))
        .as_ref()
        .map_err(|error| Error::Position(error.clone()))?;
    if style.settings.class == StyleClass::Note {
        return Err(Error::NoteStyle);
    }
    if style.bibliography.is_none() {
        return Err(Error::NoBibliography);
    }
    let library = plan
        .inputs
        .bibliography
        .converted
        .get_or_init(|| convert_bibliography(&plan.inputs.bibliography.resolved))
        .as_ref()
        .map_err(|error| Error::Conversion(error.clone()))?;
    let locale = Some(LocaleCode(
        plan.locale
            .as_ref()
            .map_or_else(|| page_locale.to_owned(), ToString::to_string),
    ));
    let mut driver = BibliographyDriver::new();
    let expanded = (style.citation.collapse == Some(Collapse::CitationNumber)
        && plan
            .groups
            .iter()
            .any(|group| group.keys.len() > 1 && group.locators.iter().any(Option::is_some)))
    .then(|| {
        let mut expanded = style.clone();
        expanded.citation.collapse = None;
        expanded
    });
    for group in &plan.groups {
        if style_facts.strips_locator_periods
            && group
                .locators
                .iter()
                .flatten()
                .any(|locator| locator.value.contains('.'))
        {
            let span = group
                .locators
                .iter()
                .flatten()
                .find(|locator| locator.value.contains('.'))
                .map(|locator| locator.span.clone())
                .unwrap_or_else(|| group.span.clone());
            return Err(Error::LocatorPeriods { span });
        }
        let grouped_locator = group.keys.len() > 1 && group.locators.iter().any(Option::is_some);
        let request_style = if grouped_locator {
            match style.citation.collapse {
                Some(Collapse::CitationNumber) => expanded.as_ref().unwrap_or(style),
                None => style,
                _ => {
                    return Err(Error::GroupedLocator {
                        span: group.span.clone(),
                    });
                }
            }
        } else {
            style
        };
        let mut items = Vec::with_capacity(group.keys.len());
        for (span, locator) in group.keys.iter().zip(&group.locators) {
            let key = &source[span.clone()];
            let canonical = plan
                .inputs
                .bibliography
                .resolved
                .get(key)
                .map_or(key, |entry| entry.key.as_str());
            let entry = library.get(canonical).ok_or_else(|| Error::MissingKey {
                key: key.to_owned(),
                span: span.clone(),
            })?;
            let locator = locator.as_ref().map(|locator| {
                SpecificLocator(
                    match locator.kind {
                        LocatorKind::Page => Locator::Page,
                        LocatorKind::Chapter => Locator::Chapter,
                    },
                    LocatorPayload::Str(&locator.value),
                )
            });
            items.push(CitationItem::with_locator(entry, locator));
        }
        driver.citation(CitationRequest::new(
            items,
            request_style,
            locale.clone(),
            locales(),
            None,
        ));
    }
    let result = driver.finish(BibliographyRequest::new(style, locale, locales()));
    let bibliography =
        output::render_bibliography(&result.bibliography.ok_or(Error::NoBibliography)?)
            .map_err(Error::Output)?;
    let bibliography = format!(
        "<section id=\"__heine-bibliography\" role=\"doc-bibliography\">{bibliography}</section>"
    );
    let citations = plan
        .groups
        .iter()
        .zip(&result.citations)
        .map(|(group, result)| {
            let mut events = Vec::new();
            output::nodes(&result.citation.0, &mut events, false).map_err(Error::Output)?;
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, events.into_iter());
            if html.contains("<a ") {
                return Err(Error::Output(
                    "citation output contains a link that cannot be nested",
                ));
            }
            let mut plain = String::new();
            result
                .citation
                .write_buf(&mut plain, BufWriteFormat::Plain)
                .map_err(|_| Error::Output("cannot format citation as text"))?;
            Ok((
                group.span.clone(),
                format!("<a href=\"#__heine-bibliography\" role=\"doc-biblioref\">{html}</a>"),
                plain,
            ))
        })
        .collect::<Result<_, Error>>()?;
    Ok(FormattedPage {
        citations,
        bibliography,
    })
}

#[cfg(test)]
mod tests {
    use super::convert_bibliography;

    #[test]
    fn streaming_conversion_matches_hayagriva_for_all_entries() {
        let bibliography = biblatex::Bibliography::parse(
            "@book{first,title={A book},author={Smith, Jane},year={2020}}\n\
             @inproceedings{second,title={A paper},booktitle={Proceedings},year={2021}}\n",
        )
        .expect("bibliography");
        let expected = hayagriva::io::from_biblatex(&bibliography).expect("library conversion");
        let actual = convert_bibliography(&bibliography).expect("streaming conversion");
        assert_eq!(actual, expected);
    }

    #[test]
    fn streaming_conversion_preserves_the_first_error_in_an_unused_entry() {
        let bibliography = biblatex::Bibliography::parse(
            "@book{used,title={Good}}\n@book{unused,title={Bad},maintitle={Set},volume={1},volumes={not-a-number}}\n",
        )
        .expect("bibliography parses");
        let expected =
            hayagriva::io::from_biblatex(&bibliography).expect_err("invalid volume count");
        let actual = convert_bibliography(&bibliography).expect_err("invalid volume count");
        assert_eq!(actual.span, expected[0].span);
        assert_eq!(actual.to_string(), expected[0].to_string());
    }
}
