//! Map resolved bibliography entries into template-facing publication views.

use std::{collections::BTreeMap, sync::Arc};

use super::BibliographyInputs;

#[derive(Debug, Clone)]
pub(crate) struct PublicationCatalog {
    bibliography: Arc<BibliographyInputs>,
    extra_fields: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct PublicationMappingError {
    pub(crate) message: String,
    pub(crate) location: Option<(String, String, std::ops::Range<usize>)>,
}

#[derive(serde::Serialize)]
pub(crate) struct PublicationView {
    key: String,
    #[serde(rename = "type")]
    kind: String,
    title: Option<String>,
    authors: Vec<String>,
    editors: Vec<PublicationEditors>,
    date: Option<PublicationDate>,
    year: Option<i32>,
    journal: Option<String>,
    book_title: Option<String>,
    publisher: Vec<String>,
    doi: Option<String>,
    url: Option<String>,
    extra: BTreeMap<String, String>,
}

#[derive(serde::Serialize)]
struct PublicationEditors {
    role: String,
    names: Vec<String>,
}

#[derive(serde::Serialize)]
struct PublicationDate {
    text: String,
}

impl PublicationCatalog {
    pub(super) fn new(bibliography: Arc<BibliographyInputs>, extra_fields: Vec<String>) -> Self {
        Self {
            bibliography,
            extra_fields,
        }
    }

    fn mapping_error(
        &self,
        entry: &biblatex::Entry,
        field: &str,
        error: &dyn std::fmt::Display,
        span: Option<std::ops::Range<usize>>,
    ) -> PublicationMappingError {
        let message = format!("publication {:?} has invalid {field}: {error}", entry.key);
        let mut location = None;
        if let Some(span) = span {
            let mut offset = 0usize;
            for source in &self.bibliography.sources {
                let end = offset.saturating_add(source.text.len());
                if span.start >= offset && span.start <= span.end && span.end <= end {
                    let local = span.start - offset;
                    if source.text.is_char_boundary(local) {
                        location = Some((
                            self.bibliography
                                .root
                                .join(&source.path)
                                .display()
                                .to_string(),
                            source.text.clone(),
                            local..span.end - offset,
                        ));
                    }
                    break;
                }
                offset = end.saturating_add(1);
            }
        }
        PublicationMappingError { message, location }
    }

    pub(crate) fn get(
        &self,
        key: &str,
    ) -> Result<Option<PublicationView>, PublicationMappingError> {
        self.bibliography
            .resolved
            .get(key)
            .map(|entry| self.view(entry))
            .transpose()
    }

    pub(crate) fn all(&self) -> Result<Vec<PublicationView>, PublicationMappingError> {
        self.bibliography
            .resolved
            .iter()
            .map(|entry| self.view(entry))
            .collect()
    }

    fn view(&self, entry: &biblatex::Entry) -> Result<PublicationView, PublicationMappingError> {
        use biblatex::ChunksExt;
        let text = |name: &str| entry.get(name).map(|v| v.format_verbatim());
        let retrieval_error = |field: &str, error: &biblatex::RetrievalError| match error {
            biblatex::RetrievalError::TypeError(error) => {
                self.mapping_error(entry, field, &error.kind, Some(error.span.clone()))
            }
            _ => self.mapping_error(entry, field, error, None),
        };
        let authors = if entry.get("author").is_some() {
            entry
                .author()
                .map_err(|e| retrieval_error("author", &e))?
                .iter()
                .map(ToString::to_string)
                .collect()
        } else {
            Vec::new()
        };
        let editors = entry
            .editors()
            .map_err(|e| self.mapping_error(entry, "editor", &e.kind, Some(e.span.clone())))?
            .into_iter()
            .map(|(names, role)| PublicationEditors {
                role: role.to_string(),
                names: names.iter().map(ToString::to_string).collect(),
            })
            .collect();
        let date_text = text("date").or_else(|| text("year"));
        let year = if date_text.is_some() {
            match entry.date().map_err(|e| retrieval_error("date", &e))? {
                biblatex::PermissiveType::Typed(biblatex::Date {
                    value: biblatex::DateValue::At(date),
                    ..
                }) => Some(date.year),
                _ => None,
            }
        } else {
            None
        };
        let publisher = if entry.get("publisher").is_some() {
            entry
                .publisher()
                .map_err(|e| retrieval_error("publisher", &e))?
                .iter()
                .map(|v| v.format_verbatim())
                .collect()
        } else {
            Vec::new()
        };
        Ok(PublicationView {
            key: entry.key.clone(),
            kind: match &entry.entry_type {
                biblatex::EntryType::Unknown(value) => value.clone(),
                kind => kind.to_string(),
            },
            title: text("title"),
            authors,
            editors,
            date: date_text.map(|text| PublicationDate { text }),
            year,
            journal: entry.journal().ok().map(|v| v.format_verbatim()),
            book_title: text("booktitle"),
            publisher,
            doi: text("doi"),
            url: text("url"),
            extra: self
                .extra_fields
                .iter()
                .filter_map(|name| text(name).map(|value| (name.clone(), value)))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn publication_mapping_errors_locate_only_authored_spans() {
        let dir = tempfile::tempdir().unwrap();
        #[derive(serde::Deserialize)]
        struct Selection {
            bibliography: Vec<toml::Spanned<String>>,
        }
        let setting: Selection = toml::from_str("bibliography=['works.bib']\n").unwrap();
        fs::write(
            dir.path().join("works.bib"),
            "@book{work,\n  title={Example}\n}\n",
        )
        .unwrap();
        let mut cache = BTreeMap::new();
        let bibliography =
            super::super::load_bibliography(dir.path(), &setting.bibliography, &mut cache).unwrap();
        let catalog = PublicationCatalog {
            bibliography,
            extra_fields: Vec::new(),
        };
        let entry = catalog.bibliography.resolved.get("work").unwrap();
        let location = entry.get("title").unwrap()[0].span.clone();
        let message = catalog.mapping_error(entry, "title", &"bad value", Some(location));
        assert!(
            message
                .location
                .as_ref()
                .is_some_and(|(path, _, _)| path.ends_with("works.bib")),
            "{message}"
        );
        let message =
            catalog.mapping_error(entry, "title", &"bad value", Some(usize::MAX..usize::MAX));
        assert!(message.location.is_none(), "{message}");
    }
}
