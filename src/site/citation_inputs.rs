//! Local files used by the citation processor.
//!
//! Page settings select checked local inputs and share parsed data across pages.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use hayagriva::citationberg::Style;
use thiserror::Error;

use crate::source_path::LogicalPath;

pub(super) mod bibliography;
mod output;
pub(super) mod policy;
pub(super) mod processor;
mod style;

/// A complete page or directory selection. Spans remain relative to the TOML
/// file that supplied the setting, including when a directory setting is used.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Settings {
    content: toml::Spanned<String>,
    bibliography: Vec<toml::Spanned<String>>,
    style: toml::Spanned<String>,
    parent_style: Option<toml::Spanned<String>>,
    locale: Option<toml::Spanned<String>>,
}

#[derive(Debug)]
pub(super) struct Source {
    path: String,
    text: String,
}

/// Style-specific inputs share a bibliography independently of their style.
#[derive(Debug)]
pub(super) struct Inputs {
    bibliography: Arc<BibliographyInputs>,
    positions: OnceLock<Result<style::StyleFacts, style::Error>>,
    style: Style,
    parent: Option<Style>,
}

#[derive(Debug)]
struct BibliographyInputs {
    // Retain authored text so conversion errors can identify their source.
    sources: Vec<Source>,
    root: PathBuf,
    resolved: biblatex::Bibliography,
    converted: OnceLock<Result<hayagriva::Library, biblatex::TypeError>>,
}

/// A page retains only the chosen source name and shared bibliography inputs.
#[derive(Debug, Clone)]
pub(super) struct PagePlan {
    pub(super) content: String,
    inputs: Arc<Inputs>,
    pub(super) groups: Vec<crate::markdown::citation::Group>,
    formatted: Option<processor::FormattedPage>,
    locale: Option<crate::locale::Locale>,
    pub(super) declaration_path: PathBuf,
    pub(super) declaration_source: Arc<str>,
    pub(super) declaration_span: std::ops::Range<usize>,
    pub(super) style_span: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct InputKey {
    root: PathBuf,
    bibliography: Vec<String>,
    style: String,
    parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BibliographyKey {
    root: PathBuf,
    files: Vec<String>,
}

/// One cache belongs to one build. Paths retain their authored order because
/// biblatex resolves repeated abbreviation definitions in that order.
#[derive(Default)]
pub(super) struct InputCache {
    entries: BTreeMap<InputKey, Arc<Inputs>>,
    bibliographies: BTreeMap<BibliographyKey, Arc<BibliographyInputs>>,
    declarations: BTreeMap<PathBuf, Arc<str>>,
}

#[derive(Debug, Error)]
pub(super) enum PlanError {
    #[error("citation content {name:?} must name one unsegmented Markdown document")]
    Content {
        name: String,
        span: std::ops::Range<usize>,
    },
    #[error(transparent)]
    Inputs(#[from] SettingsError),
}

#[derive(Debug, Clone)]
pub(crate) struct CitationView {
    pub(crate) content: String,
    pub(crate) replacements: Vec<crate::markdown::citation::Replacement>,
    pub(crate) bibliography: String,
}

impl PagePlan {
    pub(super) fn format(&mut self, source: &str, locale: &str) -> Result<(), processor::Error> {
        self.formatted = Some(processor::format(self, source, locale)?);
        Ok(())
    }

    pub(super) fn bibliography_source(
        &self,
        span: std::ops::Range<usize>,
    ) -> Option<(PathBuf, &str, std::ops::Range<usize>)> {
        let mut offset = 0usize;
        for source in &self.inputs.bibliography.sources {
            let end = offset.checked_add(source.text.len())?;
            if span.start <= end && span.end <= end && span.start >= offset {
                return Some((
                    self.inputs.bibliography.root.join(&source.path),
                    &source.text,
                    span.start - offset..span.end - offset,
                ));
            }
            offset = end.checked_add(1)?;
        }
        None
    }

    pub(super) fn heading_texts(
        &self,
        headings: &[Vec<(std::ops::Range<usize>, String)>],
    ) -> Vec<String> {
        let citations: BTreeMap<_, _> = self
            .formatted
            .iter()
            .flat_map(|value| &value.citations)
            .map(|(span, _, plain)| (span.start, (span.end, plain.as_str())))
            .collect();
        headings
            .iter()
            .map(|parts| {
                let mut text = String::new();
                for (span, part) in parts {
                    if let Some((&start, &(end, plain))) =
                        citations.range(..=span.start).next_back()
                        && span.end <= end
                        && span.start < end
                    {
                        if span.start == start {
                            text.push_str(plain);
                        }
                    } else {
                        text.push_str(part);
                    }
                }
                text
            })
            .collect()
    }

    pub(super) fn rebase(
        &mut self,
        offsets: &crate::markdown::rebase::Rebase,
    ) -> Result<(), std::ops::Range<usize>> {
        if let Some(formatted) = &mut self.formatted {
            for (span, _, _) in &mut formatted.citations {
                *span = offsets.range(span, false)?;
            }
        }
        Ok(())
    }

    pub(super) fn into_view(self) -> Option<CitationView> {
        self.formatted.map(|formatted| CitationView {
            content: self.content,
            replacements: formatted
                .citations
                .into_iter()
                .map(|(span, html, _)| crate::markdown::citation::Replacement { span, html })
                .collect(),
            bibliography: formatted.bibliography,
        })
    }

    pub(super) fn contains_key(&self, key: &str) -> bool {
        self.inputs.bibliography.resolved.get(key).is_some()
    }
}

impl InputCache {
    pub(super) fn plan(
        &mut self,
        settings: &Settings,
        root: &Path,
        content: &BTreeMap<String, serde_json::Value>,
        markdown_names: &BTreeSet<String>,
        declaration: (&Path, &str),
    ) -> Result<PagePlan, PlanError> {
        let name = settings.content.get_ref();
        if !markdown_names.contains(name)
            || !content.get(name).is_some_and(serde_json::Value::is_string)
        {
            return Err(PlanError::Content {
                name: name.clone(),
                span: settings.content.span(),
            });
        }
        let locale = settings
            .locale
            .as_ref()
            .map(|locale| {
                locale
                    .get_ref()
                    .parse::<crate::locale::Locale>()
                    .map_err(|error| SettingsError::Locale {
                        span: locale.span(),
                        message: error.to_string(),
                    })
            })
            .transpose()?;
        let key = InputKey {
            root: root.to_path_buf(),
            bibliography: settings
                .bibliography
                .iter()
                .map(|item| item.get_ref().clone())
                .collect(),
            style: settings.style.get_ref().clone(),
            parent: settings
                .parent_style
                .as_ref()
                .map(|item| item.get_ref().clone()),
        };
        let inputs = if let Some(inputs) = self.entries.get(&key) {
            Arc::clone(inputs)
        } else {
            let inputs = Arc::new(settings.load_cached(root, &mut self.bibliographies)?);
            self.entries.insert(key, Arc::clone(&inputs));
            inputs
        };
        Ok(PagePlan {
            content: name.clone(),
            groups: Vec::new(),
            formatted: None,
            declaration_path: declaration.0.to_path_buf(),
            declaration_source: Arc::clone(
                self.declarations
                    .entry(declaration.0.to_path_buf())
                    .or_insert_with(|| Arc::from(declaration.1)),
            ),
            declaration_span: settings.content.span(),
            style_span: if matches!(inputs.style, Style::Dependent(_)) {
                settings
                    .parent_style
                    .as_ref()
                    .map_or_else(|| settings.style.span(), toml::Spanned::span)
            } else {
                settings.style.span()
            },
            inputs,
            locale,
        })
    }
}

#[derive(Debug, Error)]
pub(super) enum SettingsError {
    #[error("citation content must be a simple declared content name")]
    Content { span: std::ops::Range<usize> },
    #[error("invalid citation locale: {message}")]
    Locale {
        span: std::ops::Range<usize>,
        message: String,
    },
    #[error("at least one bibliography file is required")]
    EmptyBibliography,
    #[error("cannot load citation input")]
    Input {
        span: std::ops::Range<usize>,
        #[source]
        source: InputError,
    },
    #[error(transparent)]
    Bibliography(#[from] bibliography::Error),
    #[error("invalid CSL style in {path}: {message}")]
    Style {
        path: String,
        message: String,
        span: std::ops::Range<usize>,
    },
    #[error("a dependent CSL style requires its independent parent")]
    MissingParent,
    #[error("an independent CSL style cannot select a parent")]
    UnexpectedParent,
    #[error("the selected CSL parent must be independent")]
    DependentParent,
    #[error("the selected CSL parent does not match the dependent style")]
    ParentMismatch,
}

impl Settings {
    #[cfg(test)]
    pub(super) fn load(&self, root: &Path) -> Result<Inputs, SettingsError> {
        self.load_cached(root, &mut BTreeMap::new())
    }

    fn load_cached(
        &self,
        root: &Path,
        cache: &mut BTreeMap<BibliographyKey, Arc<BibliographyInputs>>,
    ) -> Result<Inputs, SettingsError> {
        if !crate::source_path::is_simple_identifier(self.content.get_ref()) {
            return Err(SettingsError::Content {
                span: self.content.span(),
            });
        }
        if self.bibliography.is_empty() {
            return Err(SettingsError::EmptyBibliography);
        }
        let load_source = |selection: &toml::Spanned<String>| {
            read(root, selection.get_ref())
                .map(|text| Source {
                    path: selection.get_ref().clone(),
                    text,
                })
                .map_err(|source| SettingsError::Input {
                    span: selection.span(),
                    source,
                })
        };
        let key = BibliographyKey {
            root: root.to_path_buf(),
            files: self
                .bibliography
                .iter()
                .map(|item| item.get_ref().clone())
                .collect(),
        };
        let bibliography = if let Some(inputs) = cache.get(&key) {
            Arc::clone(inputs)
        } else {
            let sources = self
                .bibliography
                .iter()
                .map(load_source)
                .collect::<Result<Vec<_>, _>>()?;
            let resolved = bibliography::resolve(&sources)?;
            let inputs = Arc::new(BibliographyInputs {
                sources,
                root: root.to_path_buf(),
                resolved,
                converted: OnceLock::new(),
            });
            cache.insert(key, Arc::clone(&inputs));
            inputs
        };
        let parse_style = |selection: &toml::Spanned<String>| {
            let source = load_source(selection)?;
            Style::from_xml(&source.text).map_err(|error| SettingsError::Style {
                path: source.path,
                message: error.to_string(),
                span: selection.span(),
            })
        };
        let style = parse_style(&self.style)?;
        let parent = self.parent_style.as_ref().map(parse_style).transpose()?;
        match (&style, &parent) {
            (Style::Independent(_), None) => {}
            (Style::Independent(_), Some(_)) => return Err(SettingsError::UnexpectedParent),
            (Style::Dependent(_), None) => return Err(SettingsError::MissingParent),
            (Style::Dependent(_), Some(Style::Dependent(_))) => {
                return Err(SettingsError::DependentParent);
            }
            (Style::Dependent(child), Some(Style::Independent(parent)))
                if child.parent_link.href != parent.info.id =>
            {
                return Err(SettingsError::ParentMismatch);
            }
            (Style::Dependent(_), Some(Style::Independent(_))) => {}
        }
        Ok(Inputs {
            bibliography,
            positions: OnceLock::new(),
            style,
            parent,
        })
    }
}

#[derive(Debug, Error)]
pub(super) enum InputError {
    #[error("invalid citation input path {value:?}")]
    InvalidPath { value: String },
    #[error("citation input path contains a symbolic link: {path}")]
    Symlink { path: PathBuf },
    #[error("cannot read citation input {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("citation input is not a regular file: {path}")]
    NotFile { path: PathBuf },
}

/// Reads one authored input without following symbolic links within its root.
/// The caller retains the authored setting and its span for diagnostics.
pub(super) fn read(root: &Path, value: &str) -> Result<String, InputError> {
    let logical = LogicalPath::parse(value).ok_or_else(|| InputError::InvalidPath {
        value: value.to_owned(),
    })?;
    let mut path = root.to_path_buf();
    let mut metadata = fs::symlink_metadata(&path).map_err(|source| InputError::Read {
        path: path.clone(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(InputError::Symlink { path });
    }
    for part in logical.as_path().components() {
        path.push(part);
        metadata = fs::symlink_metadata(&path).map_err(|source| InputError::Read {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(InputError::Symlink { path });
        }
    }
    if !metadata.is_file() {
        return Err(InputError::NotFile { path });
    }
    fs::read_to_string(&path).map_err(|source| InputError::Read { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARENT: &str = r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Parent</title><id>https://example.org/parent</id></info><citation><layout><text variable="title"/></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#;
    const CHILD: &str = r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0"><info><title>Child</title><id>https://example.org/child</id><link rel="independent-parent" href="https://example.org/parent"/></info></style>"#;

    fn settings(text: &str) -> Settings {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn loads_ordered_bibliographies_and_a_checked_local_style() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bib"), "@book{a,title={First}}").unwrap();
        fs::write(dir.path().join("b.bib"), "@book{b,title={Second}}").unwrap();
        fs::write(dir.path().join("parent.csl"), PARENT).unwrap();
        fs::write(dir.path().join("child.csl"), CHILD).unwrap();
        let setting = settings(
            "content = 'main'\nbibliography = ['b.bib', 'a.bib']\nstyle = 'child.csl'\nparent_style = 'parent.csl'\nlocale = 'de-DE'\n",
        );
        let inputs = setting.load(dir.path()).unwrap();
        assert_eq!(
            inputs
                .bibliography
                .sources
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            ["b.bib", "a.bib"]
        );
        assert!(matches!(inputs.style, Style::Dependent(_)));
        assert!(matches!(inputs.parent, Some(Style::Independent(_))));
        assert_eq!(setting.locale.unwrap().into_inner(), "de-DE");
    }

    #[test]
    fn rejects_invalid_input_and_parent_relationships() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bib"), "@book{a,title={First}}").unwrap();
        fs::write(dir.path().join("bad.bib"), "@book{").unwrap();
        fs::write(dir.path().join("parent.csl"), PARENT).unwrap();
        fs::write(dir.path().join("child.csl"), CHILD).unwrap();
        fs::write(
            dir.path().join("other.csl"),
            PARENT.replace("https://example.org/parent", "https://example.org/other"),
        )
        .unwrap();
        let base = "content = 'main'\nbibliography = ['a.bib']\n";
        for (style, parent, expected) in [
            ("child.csl", None, "requires its independent parent"),
            ("parent.csl", Some("parent.csl"), "cannot select a parent"),
            ("child.csl", Some("child.csl"), "must be independent"),
            ("child.csl", Some("other.csl"), "does not match"),
        ] {
            let mut config = format!("{base}style = '{style}'\n");
            if let Some(parent) = parent {
                config.push_str(&format!("parent_style = '{parent}'\n"));
            }
            assert!(
                settings(&config)
                    .load(dir.path())
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
        }
        let config = "content = 'main'\nbibliography = ['bad.bib']\nstyle = 'parent.csl'\n";
        assert!(matches!(
            settings(config).load(dir.path()),
            Err(SettingsError::Bibliography(
                bibliography::Error::Parse { .. }
            ))
        ));
        let config = "content = 'main'\nbibliography = ['missing.bib']\nstyle = 'parent.csl'\n";
        assert!(
            matches!(settings(config).load(dir.path()), Err(SettingsError::Input { span, .. }) if &config[span.clone()] == "'missing.bib'")
        );
        assert!(matches!(
            settings("content = 'main'\nbibliography = []\nstyle = 'parent.csl'\n")
                .load(dir.path()),
            Err(SettingsError::EmptyBibliography)
        ));
        assert!(matches!(
            settings("content = 'bad-name'\nbibliography = ['a.bib']\nstyle = 'parent.csl'\n")
                .load(dir.path()),
            Err(SettingsError::Content { .. })
        ));
    }

    #[test]
    fn page_plans_share_inputs_and_reject_segmented_or_non_markdown_content() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bib"), "@book{a,title={First}}").unwrap();
        fs::write(dir.path().join("parent.csl"), PARENT).unwrap();
        let setting = settings(
            "content='main'\nbibliography=['a.bib']\nstyle='parent.csl'\nlocale='de-DE'\n",
        );
        let mut cache = InputCache::default();
        let names = BTreeSet::from(["main".to_owned()]);
        let content = BTreeMap::from([("main".to_owned(), serde_json::json!("Text [@a]."))]);
        let first = cache
            .plan(
                &setting,
                dir.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        let second = cache
            .plan(
                &setting,
                dir.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        assert!(Arc::ptr_eq(&first.inputs, &second.inputs));
        assert!(Arc::ptr_eq(
            &first.declaration_source,
            &second.declaration_source
        ));
        fs::write(dir.path().join("other.csl"), PARENT).unwrap();
        let other_style = settings("content='main'\nbibliography=['a.bib']\nstyle='other.csl'\n");
        let differently_styled = cache
            .plan(
                &other_style,
                dir.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&first.inputs, &differently_styled.inputs));
        assert!(Arc::ptr_eq(
            &first.inputs.bibliography,
            &differently_styled.inputs.bibliography,
        ));
        assert_eq!(first.content, "main");
        assert_eq!(
            first.locale.as_ref().map(ToString::to_string).as_deref(),
            Some("de-DE")
        );

        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("a.bib"), "@book{a,title={Other}}").unwrap();
        fs::write(other.path().join("parent.csl"), PARENT).unwrap();
        let other_page = cache
            .plan(
                &setting,
                other.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&first.inputs, &other_page.inputs));

        let segmented = BTreeMap::from([("main".to_owned(), serde_json::json!({"intro": "text"}))]);
        assert!(matches!(
            cache.plan(
                &setting,
                dir.path(),
                &segmented,
                &names,
                (Path::new("page"), "")
            ),
            Err(PlanError::Content { .. })
        ));
        assert!(matches!(
            cache.plan(
                &setting,
                dir.path(),
                &content,
                &BTreeSet::new(),
                (Path::new("page"), "")
            ),
            Err(PlanError::Content { .. })
        ));
    }

    #[test]
    fn reads_local_utf8_inputs_and_rejects_invalid_paths() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("styles")).unwrap();
        fs::write(dir.path().join("styles/example.csl"), "style").unwrap();
        assert_eq!(read(dir.path(), "styles/example.csl").unwrap(), "style");
        for value in ["", "../outside", "/absolute", "styles\\example.csl"] {
            assert!(
                matches!(read(dir.path(), value), Err(InputError::InvalidPath { .. })),
                "{value}"
            );
        }
        assert!(matches!(
            read(dir.path(), "styles"),
            Err(InputError::NotFile { .. })
        ));
        assert!(matches!(
            read(dir.path(), "missing.bib"),
            Err(InputError::Read { .. })
        ));
        fs::write(dir.path().join("invalid.bib"), [0xff]).unwrap();
        assert!(
            matches!(read(dir.path(), "invalid.bib"), Err(InputError::Read { source, .. }) if source.kind() == io::ErrorKind::InvalidData)
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_in_root_and_input_path() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("citations");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("source.bib"), "input").unwrap();
        symlink("source.bib", root.join("file.bib")).unwrap();
        symlink("citations", dir.path().join("linked-root")).unwrap();
        symlink(".", root.join("linked-dir")).unwrap();
        for (base, value) in [
            (&root, "file.bib"),
            (&root, "linked-dir/source.bib"),
            (&dir.path().join("linked-root"), "source.bib"),
        ] {
            assert!(matches!(read(base, value), Err(InputError::Symlink { .. })));
        }
    }
    #[test]
    fn maps_conversion_spans_to_the_correct_bibliography_file() {
        let dir = tempfile::tempdir().unwrap();
        let first = "@book{a,title={First}}";
        let second = "@book{b,title={Second}}";
        fs::write(dir.path().join("a.bib"), first).unwrap();
        fs::write(dir.path().join("b.bib"), second).unwrap();
        fs::write(dir.path().join("parent.csl"), PARENT).unwrap();
        let setting =
            settings("content='main'\nbibliography=['a.bib','b.bib']\nstyle='parent.csl'\n");
        let content = BTreeMap::from([("main".to_owned(), serde_json::json!("text"))]);
        let names = BTreeSet::from(["main".to_owned()]);
        let plan = InputCache::default()
            .plan(
                &setting,
                dir.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        let start = first.len() + 1 + second.find("Second").unwrap();
        let (path, text, span) = plan.bibliography_source(start..start + 6).unwrap();
        assert!(path.ends_with("b.bib"));
        assert_eq!(&text[span], "Second");
    }

    #[test]
    fn formats_checked_citations_and_bibliography_from_the_page_plan() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.bib"), "@book{a,title={Hello <script>}}").unwrap();
        fs::write(dir.path().join("parent.csl"), PARENT).unwrap();
        let setting = settings("content='main'\nbibliography=['a.bib']\nstyle='parent.csl'\n");
        let source = "Text [@a].";
        let content = BTreeMap::from([("main".to_owned(), serde_json::json!(source))]);
        let names = BTreeSet::from(["main".to_owned()]);
        let mut plan = InputCache::default()
            .plan(
                &setting,
                dir.path(),
                &content,
                &names,
                (Path::new("page"), ""),
            )
            .unwrap();
        plan.groups.push(crate::markdown::citation::Group {
            span: 5..9,
            keys: std::iter::once(7..8).collect(),
            locators: vec![None],
            footnote: None,
        });
        plan.format(source, "en").unwrap();
        let formatted = plan.formatted.unwrap();
        assert_eq!(formatted.citations.len(), 1);
        assert!(formatted.citations[0].1.contains("&lt;script&gt;"));
        assert!(formatted.bibliography.contains("&lt;script&gt;"));
        assert!(!formatted.bibliography.contains("<script>"));
    }
}
