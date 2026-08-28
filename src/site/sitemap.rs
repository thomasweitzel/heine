//! Structural sitemap generation from resolved render tasks.

use std::path::PathBuf;

use crate::{
    config::{BasePath, SiteOrigin, SitemapConfig},
    output::{GeneratedFile, OutputClaim, OutputPath, OutputPlan},
};

use super::{page::Task, taxonomy};

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
    #[error("sitemap index exceeds Heine's 45 MB conservative protocol limit")]
    IndexTooLarge,
}

impl Sitemap {
    pub(super) fn new(
        config: &SitemapConfig,
        origin: &SiteOrigin,
        base_path: &BasePath,
        tasks: &[Task],
        taxonomy_tasks: &[taxonomy::Task],
    ) -> Result<(Self, Vec<OutputClaim>), Error> {
        let mut entries = tasks
            .iter()
            .filter(|task| task.sitemap_include)
            .map(|task| {
                Ok(Entry {
                    url: canonical(origin, base_path, &task.output_path)?,
                    lastmod: task.sitemap_lastmod.as_ref().map(|time| time.render()),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for task in taxonomy_tasks {
            entries.push(Entry {
                url: canonical(origin, base_path, &task.output_path)?,
                lastmod: None,
            });
        }
        entries.sort_by(|left, right| left.url.cmp(&right.url));

        let groups = groups(entries);
        let mut files = Vec::new();
        if groups.len() == 1 {
            files.push(File {
                path: sitemap_path(),
                xml: urlset(&groups[0]),
            });
        } else {
            let child_paths = (1..=groups.len()).map(child_path).collect::<Vec<_>>();
            let locations = child_paths
                .iter()
                .map(|path| canonical(origin, base_path, path))
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
                xml: urlset(entries),
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
}

fn canonical(
    origin: &SiteOrigin,
    base_path: &BasePath,
    path: &OutputPath,
) -> Result<String, Error> {
    let url = origin.canonical_url(&base_path.join(&path.url_path()));
    if url.len() >= URL_LIMIT {
        return Err(Error::UrlTooLong { url });
    }
    Ok(url)
}

fn groups(entries: Vec<Entry>) -> Vec<Vec<Entry>> {
    groups_with_limits(entries, ENTRY_LIMIT, BYTE_LIMIT)
}

fn groups_with_limits(
    entries: Vec<Entry>,
    entry_limit: usize,
    byte_limit: usize,
) -> Vec<Vec<Entry>> {
    let mut groups = vec![Vec::new()];
    let mut bytes = urlset_prefix().len() + urlset_suffix().len();
    for entry in entries {
        let entry_bytes = url_entry_len(&entry);
        if !groups
            .last()
            .expect("at least one sitemap group")
            .is_empty()
            && (groups.last().expect("at least one sitemap group").len() == entry_limit
                || bytes + entry_bytes > byte_limit)
        {
            groups.push(Vec::new());
            bytes = urlset_prefix().len() + urlset_suffix().len();
        }
        bytes += entry_bytes;
        groups
            .last_mut()
            .expect("at least one sitemap group")
            .push(entry);
    }
    groups
}

fn sitemap_path() -> OutputPath {
    OutputPath::try_new(PathBuf::from("sitemap.xml")).expect("fixed sitemap path is valid")
}
fn child_path(index: usize) -> OutputPath {
    OutputPath::try_new(PathBuf::from("sitemaps").join(format!("{index}.xml")))
        .expect("fixed sitemap child path is valid")
}
fn urlset(entries: &[Entry]) -> String {
    let mut xml = String::with_capacity(
        urlset_prefix().len()
            + entries.iter().map(url_entry_len).sum::<usize>()
            + urlset_suffix().len(),
    );
    xml.push_str(urlset_prefix());
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
fn urlset_prefix() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n"
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
    xml.push_str("</url>\n");
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
    fn groups_entries_by_count_and_serialized_size_without_empty_children() {
        let entry = |url: &str| Entry {
            url: url.into(),
            lastmod: None,
        };
        let groups = groups_with_limits(
            vec![
                entry("https://example.org/a"),
                entry("https://example.org/b"),
            ],
            1,
            usize::MAX,
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0][0].url, "https://example.org/a");
        assert_eq!(groups[1][0].url, "https://example.org/b");

        let single_entry_bytes = urlset_prefix().len()
            + urlset_suffix().len()
            + url_entry_len(&entry("https://example.org/a"));
        let groups = groups_with_limits(
            vec![
                entry("https://example.org/a"),
                entry("https://example.org/b"),
            ],
            usize::MAX,
            single_entry_bytes,
        );
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.len() == 1));
    }

    #[test]
    fn entry_length_matches_its_escaped_xml_representation() {
        let entry = Entry {
            url: "https://example.org/a?subject=fish&chips".into(),
            lastmod: Some("2026-08-28T10:30:00Z".into()),
        };
        let mut rendered = String::new();
        push_url_entry(&mut rendered, &entry);
        assert_eq!(url_entry_len(&entry), rendered.len());
        assert_eq!(
            rendered,
            "  <url><loc>https://example.org/a?subject=fish&amp;chips</loc><lastmod>2026-08-28T10:30:00Z</lastmod></url>\n"
        );
    }
}
