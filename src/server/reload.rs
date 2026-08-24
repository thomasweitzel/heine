//! Development-only live-reload endpoints and HTML instrumentation.

use std::{borrow::Cow, convert::Infallible};

use axum::response::{Sse, sse::Event};
use tokio_stream::{StreamExt, wrappers::WatchStream};

use super::{INTERNAL_DIRECTORY, PublishedSite};

pub(crate) const SCRIPT_NAME: &str = "reload.js";
pub(crate) const EVENTS_NAME: &str = "events";

pub(crate) fn script() -> &'static str {
    "(() => {\n  const script = new URL(document.currentScript.src);\n  const currentBasePath = script.pathname.slice(0, -'__heine/reload.js'.length);\n  const eventsUrl = new URL('events', script);\n  const generation = script.searchParams.get('generation');\n  if (generation !== null) eventsUrl.searchParams.set('generation', generation);\n  const events = new EventSource(eventsUrl);\n  events.addEventListener('reload', (event) => {\n    const { base_path: nextBasePath } = JSON.parse(event.data);\n    if (nextBasePath === currentBasePath) {\n      window.location.reload();\n      return;\n    }\n    const relativePath = window.location.pathname.startsWith(currentBasePath)\n      ? window.location.pathname.slice(currentBasePath.length)\n      : '';\n    window.location.assign(`${nextBasePath}${relativePath}${window.location.search}${window.location.hash}`);\n  });\n})();\n"
}

pub(crate) fn events(
    published: tokio::sync::watch::Receiver<PublishedSite>,
    client_generation: Option<u64>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // The page declares the generation that produced its HTML. A connection
    // emits the current state when it differs, closing the gap between serving
    // that page and opening its EventSource connection.
    let stream = pending_publications(published, client_generation)
        .map(|site| Ok(Event::default().event("reload").data(event_data(&site))));
    Sse::new(stream)
}

fn pending_publications(
    published: tokio::sync::watch::Receiver<PublishedSite>,
    client_generation: Option<u64>,
) -> impl tokio_stream::Stream<Item = PublishedSite> {
    WatchStream::new(published)
        .filter(move |site| should_reload(client_generation, site.generation))
}

pub(crate) fn client_generation(query: Option<&str>) -> Option<u64> {
    query?
        .split('&')
        .filter_map(|parameter| parameter.split_once('='))
        .find_map(|(key, value)| (key == "generation").then_some(value))
        .and_then(|value| value.parse().ok())
}

fn should_reload(client_generation: Option<u64>, published_generation: u64) -> bool {
    client_generation != Some(published_generation)
}

fn event_data(site: &PublishedSite) -> String {
    serde_json::json!({
        "generation": site.generation,
        "base_path": site.base_path,
    })
    .to_string()
}

pub(crate) fn inject(html: String, published: &PublishedSite) -> String {
    let script_url = format!(
        "{}{}/{SCRIPT_NAME}?generation={}",
        published.base_path, INTERNAL_DIRECTORY, published.generation
    );
    let script = format!(
        r#"<script defer src="{}"></script>"#,
        escape_html_attribute(&script_url)
    );
    if let Some(position) = html.rfind("</head>") {
        let mut output = String::with_capacity(html.len() + script.len());
        output.push_str(&html[..position]);
        output.push_str(&script);
        output.push_str(&html[position..]);
        output
    } else {
        format!("{html}{script}")
    }
}

/// Escapes a value embedded in the double-quoted `src` attribute that Heine
/// injects outside Tera's normal HTML-escaping boundary.
fn escape_html_attribute(value: &str) -> Cow<'_, str> {
    if !value.contains(['&', '"', '\'', '<', '>']) {
        return Cow::Borrowed(value);
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        escaped.push_str(match character {
            '&' => "&amp;",
            '"' => "&quot;",
            '\'' => "&#39;",
            '<' => "&lt;",
            '>' => "&gt;",
            character => {
                escaped.push(character);
                continue;
            }
        });
    }
    Cow::Owned(escaped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[test]
    fn injects_before_the_final_head_close() {
        assert_eq!(
            inject(
                "<head><title>x</title></head><body></body>".into(),
                &PublishedSite {
                    base_path: "/guide/".into(),
                    output_root: "public".into(),
                    generation: 4,
                }
            ),
            "<head><title>x</title><script defer src=\"/guide/__heine/reload.js?generation=4\"></script></head><body></body>"
        );
    }

    #[test]
    fn appends_when_document_has_no_head() {
        assert_eq!(
            inject(
                "hello".into(),
                &PublishedSite {
                    base_path: "/".into(),
                    output_root: "public".into(),
                    generation: 1
                }
            ),
            "hello<script defer src=\"/__heine/reload.js?generation=1\"></script>"
        );
    }

    #[test]
    fn escapes_the_reload_url_for_its_html_attribute_context() {
        assert_eq!(
            inject(
                "<head></head>".into(),
                &PublishedSite {
                    base_path: "/guide&copy;/".into(),
                    output_root: "public".into(),
                    generation: 1,
                }
            ),
            "<head><script defer src=\"/guide&amp;copy;/__heine/reload.js?generation=1\"></script></head>"
        );
    }

    #[test]
    fn escapes_html_attribute_metacharacters() {
        assert_eq!(
            escape_html_attribute("&\"'<>").as_ref(),
            "&amp;&quot;&#39;&lt;&gt;"
        );
    }

    #[test]
    fn reload_events_describe_the_published_base_path() {
        let site = PublishedSite {
            base_path: "/guide/".into(),
            output_root: "public/guide".into(),
            generation: 3,
        };
        assert_eq!(
            event_data(&site),
            r#"{"base_path":"/guide/","generation":3}"#
        );
    }

    #[test]
    fn reload_client_preserves_the_current_route_for_a_new_base_path() {
        assert!(script().contains("window.location.assign"));
        assert!(script().contains("window.location.search"));
        assert!(script().contains("window.location.hash"));
        assert!(script().contains("eventsUrl.searchParams.set"));
    }

    #[test]
    fn generation_protocol_reloads_missed_or_future_publications() {
        assert!(!should_reload(Some(3), 3));
        assert!(should_reload(Some(3), 4));
        assert!(should_reload(None, 3));
        assert!(should_reload(Some(4), 3));
        assert_eq!(client_generation(Some("generation=42")), Some(42));
        assert_eq!(client_generation(Some("x=1&generation=42")), Some(42));
        assert_eq!(client_generation(Some("generation=not-a-number")), None);
    }

    #[test]
    fn publication_stream_closes_the_connection_race_without_reloading_current_pages() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let current = PublishedSite {
                base_path: "/".into(),
                output_root: "public".into(),
                generation: 2,
            };
            let (_sender, receiver) = tokio::sync::watch::channel(current.clone());
            let mut missed = Box::pin(pending_publications(receiver, Some(1)));
            assert_eq!(
                missed.next().await.expect("missed publication").generation,
                2
            );

            let (sender, receiver) = tokio::sync::watch::channel(current);
            let mut current_page = Box::pin(pending_publications(receiver, Some(2)));
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(1), current_page.next())
                    .await
                    .is_err()
            );
            sender
                .send(PublishedSite {
                    base_path: "/".into(),
                    output_root: "public".into(),
                    generation: 3,
                })
                .expect("watcher remains connected");
            assert_eq!(
                current_page
                    .next()
                    .await
                    .expect("future publication")
                    .generation,
                3
            );
        });
    }
}
