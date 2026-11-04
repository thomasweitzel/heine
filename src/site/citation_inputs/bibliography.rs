//! Resolve local BibTeX inputs without allowing recursive library resolution
//! to exhaust the stack on cyclic or excessively deep relationships.

use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
};

use biblatex::{Bibliography, RawBibliography, RawChunk, RawEntry, Spanned};
use thiserror::Error;

use super::Source;

const MAX_LINKS: usize = 50;
const MAX_EXPANDED_BYTES: u64 = 100_000;
const MAX_VISITS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::site) struct Location {
    pub(in crate::site) path: String,
    pub(in crate::site) span: Range<usize>,
}

#[derive(Debug, Error)]
pub(in crate::site) enum Error {
    #[error("invalid bibliography in {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: biblatex::ParseError,
    },
    #[error("invalid bibliography URL: {message}")]
    InvalidUrl { location: Location, message: String },
    #[error("duplicate bibliography key {key:?}")]
    Duplicate {
        key: String,
        first: Location,
        second: Location,
    },
    #[error("bibliography key or alias {key:?} is declared more than once")]
    AliasCollision {
        key: String,
        first: Location,
        second: Location,
    },
    #[error("cyclic bibliography relationship")]
    Cycle { location: Location },
    #[error("bibliography relationship exceeds {MAX_LINKS} links")]
    TooDeep { location: Location },
    #[error("invalid bibliography alias list")]
    InvalidAliases { location: Location },
    #[error("bibliography field expansion exceeds {MAX_EXPANDED_BYTES} source bytes")]
    ExpansionTooLarge { location: Location },
    #[error("{kind} requires more than {MAX_VISITS} recursive visits")]
    TooMuchWork {
        kind: &'static str,
        location: Location,
    },
}

#[derive(Clone)]
struct FileOffset<'a> {
    source: &'a Source,
    start: usize,
}

struct GraphAnalysis {
    order: Vec<usize>,
}

fn location(files: &[FileOffset<'_>], span: Range<usize>) -> Location {
    let file = files
        .iter()
        .rev()
        .find(|file| file.start <= span.start)
        .unwrap_or(&files[0]);
    Location {
        path: file.source.path.clone(),
        span: span.start.saturating_sub(file.start)..span.end.saturating_sub(file.start),
    }
}

fn shift(span: &mut Range<usize>, offset: usize) {
    span.start += offset;
    span.end += offset;
}

fn shift_raw(raw: &mut RawBibliography<'_>, offset: usize) {
    for entry in &mut raw.entries {
        shift(&mut entry.span, offset);
        shift(&mut entry.v.key.span, offset);
        shift(&mut entry.v.kind.span, offset);
        for field in &mut entry.v.fields {
            shift_pair(field, offset);
        }
    }
    for abbreviation in &mut raw.abbreviations {
        shift_pair(abbreviation, offset);
    }
}

fn shift_pair(pair: &mut biblatex::Pair<'_>, offset: usize) {
    shift(&mut pair.key.span, offset);
    shift(&mut pair.value.span, offset);
    for chunk in &mut pair.value.v {
        shift(&mut chunk.span, offset);
    }
}

type Edge = (usize, Range<usize>);

/// Analyze only reachable nodes. Kahn's algorithm bounds traversal and keeps
/// diagnostic selection stable without recursive graph walks.
fn check_graph(
    graph: &[Vec<Edge>],
    roots: &[usize],
    files: &[FileOffset<'_>],
) -> Result<GraphAnalysis, Error> {
    let mut reachable = vec![false; graph.len()];
    let mut pending = roots.to_vec();
    while let Some(node) = pending.pop() {
        if !std::mem::replace(&mut reachable[node], true) {
            pending.extend(graph[node].iter().map(|(target, _)| *target));
        }
    }
    let mut incoming = vec![0usize; graph.len()];
    for (node, edges) in graph.iter().enumerate() {
        if reachable[node] {
            for (target, _) in edges {
                incoming[*target] += 1;
            }
        }
    }
    let mut ready: VecDeque<_> = (0..graph.len())
        .filter(|&i| reachable[i] && incoming[i] == 0)
        .collect();
    let mut depth = vec![0usize; graph.len()];
    let mut visited = 0;
    let mut order = Vec::with_capacity(graph.len());
    while let Some(node) = ready.pop_front() {
        visited += 1;
        order.push(node);
        for (target, span) in &graph[node] {
            let next = depth[node] + 1;
            if next > MAX_LINKS {
                return Err(Error::TooDeep {
                    location: location(files, span.clone()),
                });
            }
            depth[*target] = depth[*target].max(next);
            incoming[*target] -= 1;
            if incoming[*target] == 0 {
                ready.push_back(*target);
            }
        }
    }
    if visited != reachable.iter().filter(|&&item| item).count() {
        // Find a back edge, so the label belongs to the cycle itself rather
        // than to a downstream node that Kahn's algorithm also leaves behind.
        let mut state = vec![0u8; graph.len()];
        for root in 0..graph.len() {
            if !reachable[root] || state[root] != 0 {
                continue;
            }
            state[root] = 1;
            let mut stack = vec![(root, 0usize)];
            while let Some((node, index)) = stack.last_mut() {
                if *index == graph[*node].len() {
                    state[*node] = 2;
                    stack.pop();
                    continue;
                }
                let (target, span) = &graph[*node][*index];
                *index += 1;
                if state[*target] == 1 {
                    return Err(Error::Cycle {
                        location: location(files, span.clone()),
                    });
                }
                if state[*target] == 0 {
                    state[*target] = 1;
                    stack.push((*target, 0));
                }
            }
        }
    }
    Ok(GraphAnalysis { order })
}

fn check_abbreviations(raw: &RawBibliography<'_>, files: &[FileOffset<'_>]) -> Result<(), Error> {
    // Collect in source order, matching biblatex's last-definition-wins rule.
    let definitions: BTreeMap<_, _> = raw
        .abbreviations
        .iter()
        .map(|item| (item.key.v, &item.value.v))
        .collect();
    let indices: BTreeMap<_, _> = definitions
        .keys()
        .enumerate()
        .map(|(index, key)| (*key, index))
        .collect();
    let graph: Vec<Vec<Edge>> = definitions
        .values()
        .map(|value| {
            value
                .iter()
                .filter_map(|chunk| {
                    if let RawChunk::Abbreviation(name) = chunk.v {
                        indices.get(name).map(|&index| (index, chunk.span.clone()))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    let roots: Vec<_> = raw
        .entries
        .iter()
        .flat_map(|entry| &entry.v.fields)
        .flat_map(|field| &field.value.v)
        .filter_map(|chunk| {
            if let RawChunk::Abbreviation(name) = chunk.v {
                indices.get(name).copied()
            } else {
                None
            }
        })
        .collect();
    let analysis = check_graph(&graph, &roots, files)?;
    let mut expanded_bytes = vec![0u64; graph.len()];
    let mut expanded_visits = vec![0u64; graph.len()];
    let values: Vec<_> = definitions.values().copied().collect();
    for &node in analysis.order.iter().rev() {
        let mut bytes = 0u64;
        let mut visits = 1u64;
        for chunk in values[node] {
            match chunk.v {
                RawChunk::Normal(text) => bytes = bytes.saturating_add(text.len() as u64),
                RawChunk::Abbreviation(name) => {
                    if let Some(&target) = indices.get(name) {
                        bytes = bytes.saturating_add(expanded_bytes[target]);
                        visits = visits.saturating_add(expanded_visits[target]);
                    } else {
                        // Built-in month names can expand beyond their short names.
                        bytes = bytes.saturating_add(9.max(name.len()) as u64);
                        visits = visits.saturating_add(1);
                    }
                }
            }
        }
        expanded_bytes[node] = bytes;
        expanded_visits[node] = visits;
    }
    for entry in &raw.entries {
        for field in &entry.v.fields {
            let mut bytes = 0u64;
            let mut visits = 0u64;
            let mut has_abbreviation = false;
            for chunk in &field.value.v {
                match chunk.v {
                    RawChunk::Normal(text) => bytes = bytes.saturating_add(text.len() as u64),
                    RawChunk::Abbreviation(name) => {
                        has_abbreviation = true;
                        if let Some(&target) = indices.get(name) {
                            bytes = bytes.saturating_add(expanded_bytes[target]);
                            visits = visits.saturating_add(expanded_visits[target]);
                        } else {
                            bytes = bytes.saturating_add(9.max(name.len()) as u64);
                            visits = visits.saturating_add(1);
                        }
                    }
                }
            }
            if has_abbreviation && visits > MAX_VISITS {
                return Err(Error::TooMuchWork {
                    kind: "abbreviation expansion",
                    location: location(files, field.value.span.clone()),
                });
            }
            if has_abbreviation && bytes > MAX_EXPANDED_BYTES {
                return Err(Error::ExpansionTooLarge {
                    location: location(files, field.value.span.clone()),
                });
            }
        }
    }
    Ok(())
}

fn check_inheritance(raw: &RawBibliography<'_>, files: &[FileOffset<'_>]) -> Result<(), Error> {
    // Let biblatex interpret abbreviated values, while renaming relationships
    // prevents its recursive inheritance resolver from following them yet.
    let probe = RawBibliography {
        preamble: raw.preamble.clone(),
        abbreviations: raw.abbreviations.clone(),
        entries: raw
            .entries
            .iter()
            .map(|entry| {
                let fields = entry
                    .v
                    .fields
                    .iter()
                    .filter_map(|field| {
                        let name = if field.key.v.eq_ignore_ascii_case("crossref") {
                            "heine_crossref"
                        } else if field.key.v.eq_ignore_ascii_case("ids") {
                            "heine_ids"
                        } else if field.key.v.eq_ignore_ascii_case("xdata") {
                            "heine_xdata"
                        } else {
                            return None;
                        };
                        let mut field = field.clone();
                        field.key.v = name;
                        Some(field)
                    })
                    .collect();
                Spanned::new(
                    RawEntry {
                        key: entry.v.key.clone(),
                        kind: entry.v.kind.clone(),
                        fields,
                    },
                    entry.span.clone(),
                )
            })
            .collect(),
    };
    let parsed = Bibliography::from_raw(probe).map_err(|source| parse_error(files, source))?;
    let mut indices: BTreeMap<String, (usize, &Range<usize>)> = raw
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.v.key.v.to_owned(), (index, &entry.v.key.span)))
        .collect();
    for (index, entry) in raw.entries.iter().enumerate() {
        if let Some(field) = entry
            .v
            .fields
            .iter()
            .rev()
            .find(|field| field.key.v.eq_ignore_ascii_case("ids"))
        {
            let Some(item) = parsed.get(entry.v.key.v) else {
                continue;
            };
            let aliases =
                item.get_as::<Vec<String>>("heine_ids")
                    .map_err(|_| Error::InvalidAliases {
                        location: location(files, field.value.span.clone()),
                    })?;
            for alias in aliases {
                let span = &field.value.span;
                if let Some((_, existing)) = indices.get(&alias) {
                    let (first, second) = if existing.start <= span.start {
                        ((*existing).clone(), span.clone())
                    } else {
                        (span.clone(), (*existing).clone())
                    };
                    return Err(Error::AliasCollision {
                        key: alias,
                        first: location(files, first),
                        second: location(files, second),
                    });
                }
                indices.insert(alias, (index, span));
            }
        }
    }
    let mut graph = vec![Vec::new(); raw.entries.len()];
    for (index, entry) in raw.entries.iter().enumerate() {
        let Some(item) = parsed.get(entry.v.key.v) else {
            continue;
        };
        for (field_name, probe_name) in [("crossref", "heine_crossref"), ("xdata", "heine_xdata")] {
            let Some(field) = entry
                .v
                .fields
                .iter()
                .rev()
                .find(|field| field.key.v.eq_ignore_ascii_case(field_name))
            else {
                continue;
            };
            if field_name == "crossref" {
                if let Ok(key) = item.get_as::<String>(probe_name)
                    && let Some((target, _)) = indices.get(key.as_str())
                {
                    graph[index].push((*target, field.value.span.clone()));
                }
            } else if let Ok(keys) = item.get_as::<Vec<String>>(probe_name) {
                graph[index].extend(keys.iter().filter_map(|key| {
                    indices
                        .get(key.as_str())
                        .map(|(target, _)| (*target, field.value.span.clone()))
                }));
            }
        }
    }
    let analysis = check_graph(&graph, &(0..graph.len()).collect::<Vec<_>>(), files)?;
    let mut visits = vec![1u64; graph.len()];
    for &node in analysis.order.iter().rev() {
        for (target, span) in &graph[node] {
            visits[node] = visits[node].saturating_add(visits[*target]);
            if visits[node] > MAX_VISITS {
                return Err(Error::TooMuchWork {
                    kind: "inheritance resolution",
                    location: location(files, span.clone()),
                });
            }
        }
    }
    Ok(())
}

fn parse_error(files: &[FileOffset<'_>], mut source: biblatex::ParseError) -> Error {
    let place = location(files, source.span.clone());
    source.span = place.span;
    Error::Parse {
        path: place.path,
        source,
    }
}

struct Prepared<'a> {
    raw: RawBibliography<'a>,
    files: Vec<FileOffset<'a>>,
}

fn first_url_hyphen_error(
    raw: &RawBibliography<'_>,
    files: &[FileOffset<'_>],
) -> Option<(usize, Error)> {
    if !raw.entries.iter().any(|entry| {
        entry
            .v
            .fields
            .iter()
            .any(|field| field.key.v.eq_ignore_ascii_case("url"))
    }) {
        return None;
    }
    let definitions: BTreeMap<_, _> = raw
        .abbreviations
        .iter()
        .map(|item| (item.key.v, &item.value.v))
        .collect();
    for (index, entry) in raw.entries.iter().enumerate() {
        // biblatex retains the last field with a given case-insensitive name.
        let Some(field) = entry
            .v
            .fields
            .iter()
            .rev()
            .find(|field| field.key.v.eq_ignore_ascii_case("url"))
        else {
            continue;
        };
        let mut pending: Vec<_> = field.value.v.iter().rev().collect();
        while let Some(chunk) = pending.pop() {
            match chunk.v {
                RawChunk::Normal(text) => {
                    if let Some(offset) = text.find("--") {
                        let start = chunk.span.start + offset;
                        return Some((index, Error::InvalidUrl {
                            location: location(files, start..start + 2),
                            message: "consecutive hyphens in a URL cannot be preserved by the bibliography parser; use a verified percent-encoded URL".into(),
                        }));
                    }
                }
                RawChunk::Abbreviation(name) => {
                    if let Some(value) = definitions.get(name) {
                        pending.extend(value.iter().rev());
                    }
                }
            }
        }
    }
    None
}

impl Prepared<'_> {
    fn finish(self) -> Result<Bibliography, Error> {
        if self.files.is_empty() {
            return Ok(Bibliography::new());
        }
        // Attribute a hyphen error to the entry that declares the URL. Other
        // URL errors are reported in resolved entry order after parsing.
        let mut hyphen_error = first_url_hyphen_error(&self.raw, &self.files);
        let bibliography =
            Bibliography::from_raw(self.raw).map_err(|source| parse_error(&self.files, source))?;
        for (index, entry) in bibliography.iter().enumerate() {
            if hyphen_error
                .as_ref()
                .is_some_and(|(entry, _)| *entry == index)
                && let Some((_, error)) = hyphen_error.take()
            {
                return Err(error);
            }
            let Some(chunks) = entry.get("url") else {
                continue;
            };
            let span = chunks.first().map_or(0..0, |chunk| chunk.span.clone());
            let value = entry.url().map_err(|error| Error::InvalidUrl {
                location: location(&self.files, span.clone()),
                message: error.to_string(),
            })?;
            if value
                .as_bytes()
                .windows(2)
                .any(|pair| pair[0] == b'\\' && matches!(pair[1], b'&' | b'%' | b'_' | b'#'))
            {
                return Err(Error::InvalidUrl {
                    location: location(&self.files, span),
                    message: "write URL characters literally, for example & instead of \\&".into(),
                });
            }
            let parsed = url::Url::parse(&value);
            if value.chars().any(|character| {
                character.is_whitespace() || character.is_control() || character == '\\'
            }) || !parsed.is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            }) {
                return Err(Error::InvalidUrl {
                    location: location(&self.files, span),
                    message: "use an absolute http or https URL without credentials".into(),
                });
            }
        }
        Ok(bibliography)
    }
}

fn prepare(sources: &[Source]) -> Result<Prepared<'_>, Error> {
    let mut raw = RawBibliography {
        preamble: String::new(),
        entries: Vec::new(),
        abbreviations: Vec::new(),
    };
    let mut files = Vec::with_capacity(sources.len());
    let mut offset = 0;
    let mut keys = BTreeMap::<&str, (&str, Range<usize>)>::new();
    for source in sources {
        let mut parsed =
            RawBibliography::parse(&source.text).map_err(|source_error| Error::Parse {
                path: source.path.clone(),
                source: source_error,
            })?;
        for entry in &parsed.entries {
            let at = (source.path.as_str(), entry.v.key.span.clone());
            if let Some((first_path, first_span)) = keys.insert(entry.v.key.v, at) {
                return Err(Error::Duplicate {
                    key: entry.v.key.v.to_owned(),
                    first: Location {
                        path: first_path.to_owned(),
                        span: first_span,
                    },
                    second: Location {
                        path: source.path.clone(),
                        span: entry.v.key.span.clone(),
                    },
                });
            }
        }
        shift_raw(&mut parsed, offset);
        raw.entries.extend(parsed.entries);
        raw.abbreviations.extend(parsed.abbreviations);
        raw.preamble.push_str(&parsed.preamble);
        files.push(FileOffset {
            source,
            start: offset,
        });
        offset += source.text.len() + 1;
    }
    if files.is_empty() {
        return Ok(Prepared { raw, files });
    }
    check_abbreviations(&raw, &files)?;
    check_inheritance(&raw, &files)?;
    Ok(Prepared { raw, files })
}

pub(super) fn resolve(sources: &[Source]) -> Result<Bibliography, Error> {
    prepare(sources)?.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: &str, text: &str) -> Source {
        Source {
            path: path.into(),
            text: text.into(),
        }
    }

    #[test]
    fn resolved_url_spans_do_not_always_identify_only_the_url() {
        let sources = [input(
            "urls.bib",
            "@string{host={https://example.org/a--b}}\n\
             @book{literal,url={https://example.org/a--b?x=1\\&y=2}}\n\
             @book{macro,url=host # {?x=1\\&y=2}}\n\
             @xdata{parent,url=host}\n\
             @book{inherited,xdata={parent}}\n",
        )];
        let raw = RawBibliography::parse(&sources[0].text).unwrap();
        let bibliography = Bibliography::from_raw(raw).unwrap();
        let literal = bibliography.get("literal").unwrap();
        assert_eq!(literal.url().unwrap(), "https://example.org/a–b?x=1\\&y=2");
        let literal_span = literal.get("url").unwrap()[0].span.clone();
        assert_eq!(
            &sources[0].text[literal_span],
            "https://example.org/a--b?x=1\\&y=2"
        );
        let assembled = bibliography.get("macro").unwrap();
        assert_eq!(assembled.url().unwrap(), literal.url().unwrap());
        let assembled_span = assembled.get("url").unwrap()[0].span.clone();
        assert!(sources[0].text[assembled_span].contains("@book{literal"));
        let inherited = bibliography.get("inherited").unwrap();
        assert_eq!(inherited.url().unwrap(), "https://example.org/a–b");
        let inherited_span = inherited.get("url").unwrap()[0].span.clone();
        assert_eq!(&sources[0].text[inherited_span], "https://example.org/a--b");
    }

    #[test]
    fn url_span_can_include_unrelated_hyphens() {
        let sources = [input(
            "urls.bib",
            "@string{host={https://example.org}}\n\
             @book{unrelated,title={A--B}}\n\
             @book{work,url=host # {/path}}\n",
        )];
        let prepared = prepare(&sources).unwrap();
        let bibliography = Bibliography::from_raw(prepared.raw).unwrap();
        let entry = bibliography.get("work").unwrap();
        assert_eq!(entry.url().unwrap(), "https://example.org/path");
        let span = entry.get("url").unwrap()[0].span.clone();
        assert!(sources[0].text[span].contains("A--B"));
        assert!(resolve(&sources).is_ok());
    }

    #[test]
    fn url_hyphen_error_points_to_the_abbreviation_definition() {
        let sources = [
            input("shared.bib", "@string{host={https://example.org/a--b}}\n"),
            input("works.bib", "@book{work,url=host # {/path}}\n"),
        ];
        let Err(Error::InvalidUrl { location, .. }) = resolve(&sources) else {
            panic!("URL hyphens must be rejected");
        };
        assert_eq!(location.path, "shared.bib");
        assert_eq!(&sources[0].text[location.span], "--");
    }

    #[test]
    fn url_checks_follow_field_and_abbreviation_overrides() {
        let sources = [input(
            "works.bib",
            "@string{host={https://example.org/a--b}}\n\
             @string{host={https://example.org/good}}\n\
             @book{parent,url=host}\n\
             @book{child,crossref={parent},url={https://example.org/a--b},URL={https://example.org/child}}\n",
        )];
        let bibliography = resolve(&sources).unwrap();
        assert_eq!(
            bibliography.get("parent").unwrap().url().unwrap(),
            "https://example.org/good"
        );
        assert_eq!(
            bibliography.get("child").unwrap().url().unwrap(),
            "https://example.org/child"
        );
    }

    #[test]
    fn url_checks_use_the_last_abbreviation_definition_across_files() {
        let sources = [
            input(
                "first.bib",
                "@string{host={https://example.org/good}}\n@book{work,url=host}\n",
            ),
            input("last.bib", "@string{host={https://example.org/a--b}}\n"),
        ];
        let Err(Error::InvalidUrl { location, .. }) = resolve(&sources) else {
            panic!("the effective URL must be rejected");
        };
        assert_eq!(location.path, "last.bib");
        assert_eq!(&sources[1].text[location.span], "--");
    }

    #[test]
    fn rejects_collisions_between_keys_and_aliases_in_either_order() {
        for (first, second) in [
            ("@book{a,title={A}}", "@book{b,ids={a},title={B}}"),
            ("@book{b,ids={a},title={B}}", "@book{a,title={A}}"),
            (
                "@book{a,ids={shared},title={A}}",
                "@book{b,ids={shared},title={B}}",
            ),
        ] {
            let sources = [input("first.bib", first), input("second.bib", second)];
            let Err(Error::AliasCollision {
                key,
                first: first_location,
                second: second_location,
            }) = resolve(&sources)
            else {
                panic!("colliding key or alias must fail");
            };
            assert!(key == "a" || key == "shared");
            assert_eq!(first_location.path, "first.bib");
            assert_eq!(second_location.path, "second.bib");
            assert!(first[first_location.span].contains(&key));
            assert!(second[second_location.span].contains(&key));
        }
    }

    #[test]
    fn rejects_aliases_repeated_within_one_entry() {
        for value in ["a", "other, other"] {
            let source = input("works.bib", &format!("@book{{a,ids={{{value}}}}}"));
            assert!(matches!(
                resolve(&[source]),
                Err(Error::AliasCollision { .. })
            ));
        }
    }

    #[test]
    fn url_hyphens_from_separate_fragments_are_preserved() {
        let sources = [input(
            "works.bib",
            "@string{prefix={https://example.org/a-}}\n@book{work,url=prefix # {-b}}\n",
        )];
        let bibliography = resolve(&sources).unwrap();
        assert_eq!(
            bibliography.get("work").unwrap().url().unwrap(),
            "https://example.org/a--b"
        );
    }

    #[test]
    fn inherited_hyphen_error_is_ordered_at_the_parent_declaration() {
        let sources = [input(
            "works.bib",
            "@inbook{child,crossref={parent}}\n\
             @book{other,url={not a url}}\n\
             @book{parent,url={https://example.org/a--b}}\n",
        )];
        let Err(Error::InvalidUrl { location, .. }) = resolve(&sources) else {
            panic!("invalid URL must fail");
        };
        assert_eq!(&sources[0].text[location.span], "not a url");
    }

    #[test]
    fn url_diagnostics_follow_parse_then_entry_order() {
        let sources = [input(
            "works.bib",
            "@book{first,url={not a url}}\n@book{second,url={https://example.org/a--b}}\n",
        )];
        let Err(Error::InvalidUrl { location, .. }) = resolve(&sources) else {
            panic!("invalid URL must fail");
        };
        assert_eq!(&sources[0].text[location.span], "not a url");
        let sources = [input(
            "works.bib",
            "@book{first,url={https://example.org/a--b}}\n@book{second,title=undefined}\n",
        )];
        assert!(matches!(resolve(&sources), Err(Error::Parse { .. })));
    }

    #[test]
    fn resolves_files_together_and_keeps_authored_order_for_abbreviations() {
        let sources = [
            input(
                "shared.bib",
                "@string{press={First}}\n@book{parent,title={Parent},publisher=press}",
            ),
            input(
                "paper.bib",
                "@string{press={Second}}\n@inbook{child,title={Chapter},crossref={parent}}",
            ),
        ];
        let bibliography = resolve(&sources).unwrap();
        assert_eq!(
            bibliography
                .get("child")
                .unwrap()
                .get_as::<String>("publisher")
                .unwrap(),
            "Second"
        );
    }

    #[test]
    fn reports_both_locations_for_duplicate_keys() {
        let sources = [
            input("one.bib", "@book{same,title={One}}"),
            input("two.bib", "@book{same,title={Two}}"),
        ];
        let Err(Error::Duplicate { first, second, .. }) = resolve(&sources) else {
            panic!("expected duplicate")
        };
        assert_eq!(
            (&first.path, &sources[0].text[first.span]),
            (&"one.bib".to_owned(), "same")
        );
        assert_eq!(
            (&second.path, &sources[1].text[second.span]),
            (&"two.bib".to_owned(), "same")
        );
    }

    #[test]
    fn reports_cross_file_cycles_at_the_responsible_source() {
        let sources = [
            input("one.bib", "@string{a=b}\n@book{x,title=a}"),
            input("two.bib", "@string{b=a}"),
        ];
        let Err(Error::Cycle { location }) = resolve(&sources) else {
            panic!("expected cycle")
        };
        assert_eq!(location.path, "two.bib");
        assert_eq!(&sources[1].text[location.span], "a");

        let sources = [
            input("one.bib", "@book{a,crossref={b}}"),
            input("two.bib", "@book{b,crossref={a}}"),
        ];
        let Err(Error::Cycle { location }) = resolve(&sources) else {
            panic!("expected cycle")
        };
        assert_eq!(location.path, "two.bib");
        assert!(sources[1].text[location.span].contains('a'));

        let sources = [input(
            "aliases.bib",
            "@book{a,ids={alias},crossref={alias}}",
        )];
        assert!(matches!(resolve(&sources), Err(Error::Cycle { .. })));
    }

    #[test]
    fn enforces_fifty_link_limit_for_both_relationships() {
        for links in [50, 51] {
            let mut abbreviations = String::new();
            let mut inheritance = String::new();
            for i in 0..links {
                abbreviations.push_str(&format!("@string{{s{i}=s{}}}\n", i + 1));
                inheritance.push_str(&format!("@book{{s{i},crossref={{s{}}}}}\n", i + 1));
            }
            abbreviations.push_str(&format!("@string{{s{links}={{ok}}}}\n@book{{a,title=s0}}"));
            inheritance.push_str(&format!("@book{{s{links},title={{ok}}}}"));
            for text in [abbreviations, inheritance] {
                let result = resolve(&[input("input.bib", &text)]);
                if links == 50 {
                    assert!(result.is_ok(), "{result:?}");
                } else {
                    assert!(matches!(result, Err(Error::TooDeep { .. })), "{result:?}");
                }
            }
        }
    }

    #[test]
    fn branching_abbreviations_and_inheritance_preserve_library_behavior() {
        // Keep these examples small: biblatex recursively expands each edge,
        // so repeating a reference doubles the work at each level.
        let mut abbreviations = String::from("@string{s0={x}}\n");
        for level in 1..=8 {
            abbreviations.push_str(&format!(
                "@string{{s{level}=s{} # s{}}}\n",
                level - 1,
                level - 1
            ));
        }
        abbreviations.push_str("@book{work,title=s8}");
        let bibliography = resolve(&[input("strings.bib", &abbreviations)]).unwrap();
        assert_eq!(
            bibliography
                .get("work")
                .unwrap()
                .get_as::<String>("title")
                .unwrap()
                .len(),
            1 << 8
        );

        let mut inheritance = String::from("@xdata{s0,publisher={Press}}\n");
        for level in 1..=8 {
            inheritance.push_str(&format!(
                "@xdata{{s{level},xdata={{s{},s{}}}}}\n",
                level - 1,
                level - 1
            ));
        }
        inheritance.push_str("@book{work,title={Example},xdata={s8}}");
        let bibliography = resolve(&[input("inheritance.bib", &inheritance)]).unwrap();
        assert_eq!(
            bibliography
                .get("work")
                .unwrap()
                .get_as::<String>("publisher")
                .unwrap(),
            "Press"
        );
    }

    #[test]
    fn enforces_expansion_size_before_library_resolution() {
        for length in [100_000, 100_001] {
            let text = format!(
                "@string{{value={{x}}}}\n@book{{work,title=value # {{{}}}}}",
                "a".repeat(length - 1)
            );
            let result = resolve(&[input("input.bib", &text)]);
            if length == 100_000 {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(result, Err(Error::ExpansionTooLarge { .. })),
                    "{result:?}"
                );
            }
        }
        let shared = input("shared.bib", "@string{s={x}}\n");
        let second = input(
            "paper.bib",
            &format!("@book{{a,xdata=s # {{{}}}}}", "y".repeat(100_000)),
        );
        let result = resolve(&[shared, second]);
        let Err(Error::ExpansionTooLarge { location }) = result else {
            panic!("expected oversized expansion")
        };
        assert_eq!(location.path, "paper.bib");
    }

    #[test]
    fn enforces_per_field_and_per_entry_visit_limits() {
        for count in [1_000, 1_001] {
            let abbreviations = format!(
                "@string{{s={{x}}}}\n@book{{work,title={}}}",
                vec!["s"; count].join(" # ")
            );
            let result = resolve(&[input("strings.bib", &abbreviations)]);
            if count == 1_000 {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err(Error::TooMuchWork {
                            kind: "abbreviation expansion",
                            ..
                        })
                    ),
                    "{result:?}"
                );
            }

            let inheritance = format!(
                "@xdata{{parent,publisher={{Press}}}}\n@book{{work,title={{Example}},xdata={{{}}}}}",
                vec!["parent"; count - 1].join(",")
            );
            let result = resolve(&[input("inheritance.bib", &inheritance)]);
            if count == 1_000 {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err(Error::TooMuchWork {
                            kind: "inheritance resolution",
                            ..
                        })
                    ),
                    "{result:?}"
                );
            }
        }
    }
}
