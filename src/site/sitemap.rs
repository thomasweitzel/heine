//! Structural sitemap generation from resolved render tasks.

use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    config::{CanonicalUrls, SitemapConfig},
    locale::Locale,
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
};

use super::{
    page::{Page, PageId, Pager, Task, TranslationId},
    series, taxonomy,
};

const ENTRY_LIMIT: usize = 45_000;
const BYTE_LIMIT: usize = 45_000_000;
const URL_LIMIT: usize = 2_048;

#[derive(Debug)]
pub(super) struct Sitemap {
    files: Vec<File>,
}

#[derive(Debug)]
struct File {
    path: OutputPath,
    xml: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("canonical sitemap URL must be shorter than the 2,048-character protocol limit: {url}")]
    UrlTooLong { url: String },
    #[error("sitemap entry exceeds Heine's 45 MB conservative protocol limit: {url}")]
    EntryTooLarge { url: String },
    #[error("sitemap index exceeds Heine's 45 MB conservative protocol limit")]
    IndexTooLarge,
}

impl Sitemap {
    pub(super) fn output_count(&self) -> usize {
        self.files.len()
    }

    pub(super) fn new(
        config: &SitemapConfig,
        urls: &CanonicalUrls,
        pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
        tasks: &[Task],
        taxonomy_tasks: &[taxonomy::Task],
        series_tasks: &[series::Task],
    ) -> Result<(Self, Vec<OutputClaim>), Error> {
        let alternates = alternate_catalog(config, urls, pages, tasks)?;
        let mut entries = tasks
            .iter()
            .filter(|task| {
                sitemap_task_included(config, task.sitemap_include, task.pagination.as_deref())
            })
            .map(|task| {
                Ok(Entry {
                    url: canonical(urls, &task.output_path)?,
                    lastmod: task
                        .sitemap_lastmod
                        .as_ref()
                        .map(super::page::PublicationTime::render),
                    alternates: alternates
                        .get(&(task.locale.clone(), task.output_path.clone()))
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for task in taxonomy_tasks.iter().filter(|task| {
            sitemap_task_included(config, task.sitemap_include, task.pagination.as_deref())
        }) {
            entries.push(Entry {
                url: canonical(urls, &task.output_path)?,
                lastmod: None,
                alternates: Vec::new(),
            });
        }
        for task in series_tasks.iter().filter(|task| {
            sitemap_task_included(config, task.sitemap_include, task.pagination.as_deref())
        }) {
            entries.push(Entry {
                url: canonical(urls, &task.output_path)?,
                lastmod: None,
                alternates: Vec::new(),
            });
        }
        entries.sort_by(|left, right| left.url.cmp(&right.url));

        let groups = groups(entries, config.hreflang())?;
        let mut files = Vec::new();
        if groups.len() == 1 {
            files.push(File {
                path: sitemap_path(),
                xml: urlset(&groups[0], config.hreflang()),
            });
        } else {
            let child_paths = (1..=groups.len()).map(child_path).collect::<Vec<_>>();
            let locations = child_paths
                .iter()
                .map(|path| canonical(urls, path))
                .collect::<Result<Vec<_>, _>>()?;
            let index = sitemap_index(&locations);
            if index.len() > BYTE_LIMIT {
                return Err(Error::IndexTooLarge);
            }
            files.push(File {
                path: sitemap_path(),
                xml: index,
            });
            files.extend(groups.iter().zip(child_paths).map(|(entries, path)| File {
                path,
                xml: urlset(entries, config.hreflang()),
            }));
        }
        let claims = files
            .iter()
            .filter_map(|file| config.output_claim(file.path.clone()))
            .collect();
        Ok((Self { files }, claims))
    }

    pub(super) fn generated(&self, plan: &OutputPlan) -> Vec<GeneratedFile> {
        self.files
            .iter()
            .map(|file| {
                let path = plan
                    .planned_path(&file.path)
                    .expect("sitemap output paths were validated before rendering");
                GeneratedFile::text(path, file.xml.clone())
            })
            .collect()
    }
}

#[derive(Debug)]
struct Entry {
    url: String,
    lastmod: Option<String>,
    alternates: Vec<Alternate>,
}

#[derive(Debug, Clone)]
struct Alternate {
    locale: String,
    url: String,
}

#[derive(Debug)]
struct AlternateMember {
    output_path: OutputPath,
    url: String,
}

fn canonical(urls: &CanonicalUrls, path: &OutputPath) -> Result<String, Error> {
    let url = urls.output_url(path);
    if url.len() >= URL_LIMIT {
        return Err(Error::UrlTooLong { url });
    }
    Ok(url)
}

fn groups(entries: Vec<Entry>, hreflang: bool) -> Result<Vec<Vec<Entry>>, Error> {
    groups_with_limits(entries, ENTRY_LIMIT, BYTE_LIMIT, hreflang)
}

fn groups_with_limits(
    entries: Vec<Entry>,
    entry_limit: usize,
    byte_limit: usize,
    hreflang: bool,
) -> Result<Vec<Vec<Entry>>, Error> {
    let mut groups = vec![Vec::new()];
    let overhead = urlset_prefix(hreflang).len() + urlset_suffix().len();
    let mut bytes = overhead;
    for entry in entries {
        let entry_bytes = url_entry_len(&entry);
        if overhead + entry_bytes > byte_limit {
            return Err(Error::EntryTooLarge { url: entry.url });
        }
        if !groups
            .last()
            .expect("at least one sitemap group")
            .is_empty()
            && (groups.last().expect("at least one sitemap group").len() == entry_limit
                || bytes + entry_bytes > byte_limit)
        {
            groups.push(Vec::new());
            bytes = overhead;
        }
        bytes += entry_bytes;
        groups
            .last_mut()
            .expect("at least one sitemap group")
            .push(entry);
    }
    Ok(groups)
}

fn sitemap_path() -> OutputPath {
    OutputPath::try_new(PathBuf::from("sitemap.xml")).expect("fixed sitemap path is valid")
}
fn child_path(index: usize) -> OutputPath {
    OutputPath::try_new(PathBuf::from("sitemaps").join(format!("{index}.xml")))
        .expect("fixed sitemap child path is valid")
}
fn urlset(entries: &[Entry], hreflang: bool) -> String {
    let mut xml = String::with_capacity(
        urlset_prefix(hreflang).len()
            + entries.iter().map(url_entry_len).sum::<usize>()
            + urlset_suffix().len(),
    );
    xml.push_str(urlset_prefix(hreflang));
    for entry in entries {
        push_url_entry(&mut xml, entry);
    }
    xml.push_str(urlset_suffix());
    xml
}
fn sitemap_index(locations: &[String]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for location in locations {
        xml.push_str("  <sitemap><loc>");
        xml.push_str(&escape(location));
        xml.push_str("</loc></sitemap>\n");
    }
    xml.push_str("</sitemapindex>\n");
    xml
}
fn urlset_prefix(hreflang: bool) -> &'static str {
    if hreflang {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n"
    } else {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n"
    }
}
fn urlset_suffix() -> &'static str {
    "</urlset>\n"
}
fn url_entry_len(entry: &Entry) -> usize {
    "  <url><loc>".len()
        + escaped_len(&entry.url)
        + "</loc>".len()
        + entry.lastmod.as_deref().map_or(0, |lastmod| {
            "<lastmod>".len() + escaped_len(lastmod) + "</lastmod>".len()
        })
        + entry.alternates.iter().map(alternate_len).sum::<usize>()
        + "</url>\n".len()
}

fn push_url_entry(xml: &mut String, entry: &Entry) {
    xml.push_str("  <url><loc>");
    push_escaped(xml, &entry.url);
    xml.push_str("</loc>");
    if let Some(lastmod) = &entry.lastmod {
        xml.push_str("<lastmod>");
        push_escaped(xml, lastmod);
        xml.push_str("</lastmod>");
    }
    for alternate in &entry.alternates {
        push_alternate(xml, alternate);
    }
    xml.push_str("</url>\n");
}

fn alternate_len(alternate: &Alternate) -> usize {
    "<xhtml:link rel=\"alternate\" hreflang=\"".len()
        + escaped_len(&alternate.locale)
        + "\" href=\"".len()
        + escaped_len(&alternate.url)
        + "\"/>".len()
}

fn push_alternate(xml: &mut String, alternate: &Alternate) {
    xml.push_str("<xhtml:link rel=\"alternate\" hreflang=\"");
    push_escaped(xml, &alternate.locale);
    xml.push_str("\" href=\"");
    push_escaped(xml, &alternate.url);
    xml.push_str("\"/>");
}

fn alternate_catalog(
    config: &SitemapConfig,
    urls: &CanonicalUrls,
    pages: &BTreeMap<Locale, BTreeMap<PageId, Page>>,
    tasks: &[Task],
) -> Result<BTreeMap<(Locale, OutputPath), Vec<Alternate>>, Error> {
    if !config.hreflang() {
        return Ok(BTreeMap::new());
    }

    let mut groups = BTreeMap::<TranslationId, BTreeMap<Locale, AlternateMember>>::new();
    for task in tasks.iter().filter(|task| is_authored_sitemap_task(task)) {
        let page = &pages[&task.locale][&task.page_id];
        let Some(translationid) = &page.translationid else {
            continue;
        };
        groups.entry(translationid.clone()).or_default().insert(
            task.locale.clone(),
            AlternateMember {
                output_path: task.output_path.clone(),
                url: canonical(urls, &task.output_path)?,
            },
        );
    }

    let mut catalog = BTreeMap::new();
    for members in groups.into_values().filter(|members| members.len() >= 2) {
        let alternates = members
            .iter()
            .map(|(locale, member)| Alternate {
                locale: locale.to_string(),
                url: member.url.clone(),
            })
            .collect::<Vec<_>>();
        for (locale, member) in members {
            catalog.insert((locale, member.output_path), alternates.clone());
        }
    }
    Ok(catalog)
}

/// Later pagination tasks are generated views and do not name independently
/// translatable authored output.
fn is_authored_sitemap_task(task: &Task) -> bool {
    task.sitemap_include && task.pagination.as_deref().is_none_or(Pager::is_first)
}

fn sitemap_task_included(
    config: &SitemapConfig,
    sitemap_include: bool,
    pagination: Option<&Pager>,
) -> bool {
    sitemap_include && (config.include_pagers() || pagination.is_none_or(Pager::is_first))
}

fn escaped_len(value: &str) -> usize {
    value
        .bytes()
        .map(|byte| match byte {
            b'&' => "&amp;".len(),
            b'<' | b'>' => "&lt;".len(),
            b'\"' => "&quot;".len(),
            b'\'' => "&apos;".len(),
            _ => 1,
        })
        .sum()
}

fn push_escaped(xml: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '\"' => xml.push_str("&quot;"),
            '\'' => xml.push_str("&apos;"),
            _ => xml.push(character),
        }
    }
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(escaped_len(value));
    push_escaped(&mut escaped, value);
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_text() {
        assert_eq!(escape("a&<'\""), "a&amp;&lt;&apos;&quot;");
    }

    #[test]
    fn ordinary_urlsets_keep_the_existing_serialization() {
        let entries = [Entry {
            url: "https://example.org/index.html".into(),
            lastmod: None,
            alternates: Vec::new(),
        }];

        assert_eq!(
            urlset(&entries, false),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url><loc>https://example.org/index.html</loc></url>\n</urlset>\n"
        );
    }

    #[test]
    fn groups_entries_by_count_and_serialized_size_without_empty_children() {
        let entry = |url: &str| Entry {
            url: url.into(),
            lastmod: None,
            alternates: Vec::new(),
        };
        let groups = groups_with_limits(
            vec![
                entry("https://example.org/a"),
                entry("https://example.org/b"),
            ],
            1,
            usize::MAX,
            false,
        )
        .expect("entries fit");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0][0].url, "https://example.org/a");
        assert_eq!(groups[1][0].url, "https://example.org/b");

        let single_entry_bytes = urlset_prefix(false).len()
            + urlset_suffix().len()
            + url_entry_len(&entry("https://example.org/a"));
        let groups = groups_with_limits(
            vec![
                entry("https://example.org/a"),
                entry("https://example.org/b"),
            ],
            usize::MAX,
            single_entry_bytes,
            false,
        )
        .expect("each entry fits");
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.len() == 1));
    }

    #[test]
    fn groups_entries_by_serialized_size_with_hreflang_annotations() {
        let entry = |url: &str| Entry {
            url: url.into(),
            lastmod: None,
            alternates: vec![Alternate {
                locale: "de".into(),
                url: "https://example.org/de/a".into(),
            }],
        };
        let first = entry("https://example.org/a");
        let byte_limit = urlset_prefix(true).len() + urlset_suffix().len() + url_entry_len(&first);
        let groups = groups_with_limits(
            vec![first, entry("https://example.org/b")],
            usize::MAX,
            byte_limit,
            true,
        )
        .expect("each entry fits");
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.len() == 1));
    }

    #[test]
    fn rejects_an_entry_that_cannot_fit_in_one_sitemap() {
        let entry = Entry {
            url: "https://example.org/a".into(),
            lastmod: None,
            alternates: Vec::new(),
        };
        let error = groups_with_limits(vec![entry], usize::MAX, 1, false)
            .expect_err("the XML wrapper alone exceeds this limit");
        assert!(matches!(error, Error::EntryTooLarge { .. }));
    }

    #[test]
    fn entry_length_matches_its_escaped_xml_representation() {
        let entry = Entry {
            url: "https://example.org/a?subject=fish&chips".into(),
            lastmod: Some("2026-08-28T10:30:00Z".into()),
            alternates: vec![Alternate {
                locale: "de".into(),
                url: "https://example.org/de/a?subject=fish&chips".into(),
            }],
        };
        let mut rendered = String::new();
        push_url_entry(&mut rendered, &entry);
        assert_eq!(url_entry_len(&entry), rendered.len());
        assert_eq!(
            rendered,
            "  <url><loc>https://example.org/a?subject=fish&amp;chips</loc><lastmod>2026-08-28T10:30:00Z</lastmod><xhtml:link rel=\"alternate\" hreflang=\"de\" href=\"https://example.org/de/a?subject=fish&amp;chips\"/></url>\n"
        );
    }
}
