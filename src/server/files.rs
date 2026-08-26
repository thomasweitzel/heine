//! Safe output-tree serving and HTTP byte-range handling.

use std::{
    io,
    path::{Component, Path, PathBuf},
};

use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use miette::{Diagnostic, Report};
use percent_encoding::percent_decode_str;
use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    time::{Duration, sleep},
};
use tokio_util::io::ReaderStream;

use super::{INTERNAL_DIRECTORY, PublishedSite, ServerState, reload};
use crate::output::is_internal_output_path;

#[derive(Debug, Diagnostic, Error)]
#[error("could not serve generated file {path}")]
#[diagnostic(
    code(heine::server::serve_file),
    help("run another build if generated output changed while this request was in progress")
)]
struct RequestError {
    path: PathBuf,
    #[source]
    source: io::Error,
}

/// The complete development-server routing surface. Keeping it beside the
/// request handlers lets tests exercise the same URI extraction path as the
/// running server without opening a network listener.
pub(crate) fn router(state: std::sync::Arc<ServerState>) -> Router {
    Router::new()
        .route("/", get(get_root).head(head_root))
        .route("/{*path}", get(get_path).head(head_path))
        .with_state(state)
        .layer(middleware::map_response(add_development_headers))
}

async fn add_development_headers(mut response: Response) -> Response {
    development_headers(response.headers_mut());
    response
}

pub(crate) async fn get_root(
    State(state): State<std::sync::Arc<ServerState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    serve(state, uri.path(), uri.query(), &headers, false).await
}

pub(crate) async fn head_root(
    State(state): State<std::sync::Arc<ServerState>>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    serve(state, uri.path(), uri.query(), &HeaderMap::new(), true).await
}

pub(crate) async fn get_path(
    State(state): State<std::sync::Arc<ServerState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    serve(state, uri.path(), uri.query(), &headers, false).await
}

pub(crate) async fn head_path(
    State(state): State<std::sync::Arc<ServerState>>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    serve(state, uri.path(), uri.query(), &HeaderMap::new(), true).await
}

async fn serve(
    state: std::sync::Arc<ServerState>,
    requested: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let published = state.published.borrow().clone();
    let Some(path) = output_path(requested, &published) else {
        return base_redirect_or_not_found(requested, &published);
    };
    if path == Path::new(INTERNAL_DIRECTORY).join(reload::SCRIPT_NAME) {
        return reload_script();
    }
    if path == Path::new(INTERNAL_DIRECTORY).join(reload::EVENTS_NAME) {
        return reload::events(state.published.clone(), reload::client_generation(query))
            .into_response();
    }
    if is_internal_output_path(&path) {
        return not_found();
    }
    match safe_file_after_publication(&published.output_root, &path).await {
        Ok(Some(file)) => file_response(file, &published, headers, head).await,
        Ok(None) => not_found(),
        Err(source) => internal_error(published.output_root.join(&path), source),
    }
}

/// A full build replaces the output directory using two filesystem renames.
/// The server intentionally reads that same directory rather than a second
/// copy, so retry only when the root itself is absent in this tiny publication
/// interval. Ordinary missing paths remain immediate 404 responses.
async fn safe_file_after_publication(root: &Path, relative: &Path) -> io::Result<Option<PathBuf>> {
    let file = safe_file(root, relative).await?;
    if file.is_some() || output_root_exists(root).await? {
        return Ok(file);
    }
    sleep(Duration::from_millis(10)).await;
    safe_file(root, relative).await
}

async fn output_root_exists(root: &Path) -> io::Result<bool> {
    match tokio::fs::symlink_metadata(root).await {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn base_redirect_or_not_found(requested: &str, published: &PublishedSite) -> Response {
    if published.base_path.as_str() != "/"
        && requested == published.base_path.as_str().trim_end_matches('/')
    {
        return development_redirect(published.base_path.as_str());
    }
    not_found()
}

fn output_path(requested: &str, published: &PublishedSite) -> Option<PathBuf> {
    let relative = if published.base_path.as_str() == "/" {
        requested
    } else {
        requested.strip_prefix(published.base_path.as_str())?
    };
    decoded_relative_path(relative)
}

fn decoded_relative_path(path: &str) -> Option<PathBuf> {
    let mut output = PathBuf::new();
    for part in path.split('/') {
        if part.is_empty() {
            continue;
        }
        let decoded = percent_decode_str(part).decode_utf8().ok()?;
        if decoded.is_empty()
            || decoded.contains(['\\', '\0', '/'])
            || matches!(decoded.as_ref(), "." | "..")
        {
            return None;
        }
        output.push(decoded.as_ref());
    }
    if path.is_empty() || path.ends_with('/') {
        output.push("index.html");
    }
    Some(output)
}

async fn safe_file(root: &Path, relative: &Path) -> io::Result<Option<PathBuf>> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Ok(None);
        };
        current.push(component);
        let metadata = match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            return Ok(None);
        }
    }
    match tokio::fs::metadata(&current).await {
        Ok(metadata) if metadata.is_file() => Ok(Some(current)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

async fn file_response(
    path: PathBuf,
    published: &PublishedSite,
    headers: &HeaderMap,
    head: bool,
) -> Response {
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(source) => return internal_error(&path, source),
    };
    let length = metadata.len();
    let content_type = content_type(&path);
    if head {
        return head_response(path, published, &content_type, length).await;
    }
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    match parse_range(range, length) {
        RangeRequest::Unsatisfiable => range_not_satisfiable(length),
        RangeRequest::Single { start, end } => {
            ranged_response(path, &content_type, start, end, length).await
        }
        RangeRequest::Full => {
            if is_html(&content_type) {
                match tokio::fs::read(&path).await.and_then(|bytes| {
                    String::from_utf8(bytes)
                        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))
                }) {
                    Ok(html) => {
                        let html = reload::inject(html, published);
                        response(
                            StatusCode::OK,
                            &content_type,
                            html.len() as u64,
                            None,
                            Body::from(html),
                        )
                    }
                    Err(_) => full_file_response(path, &content_type, length).await,
                }
            } else {
                full_file_response(path, &content_type, length).await
            }
        }
    }
}

async fn head_response(
    path: PathBuf,
    published: &PublishedSite,
    content_type: &str,
    length: u64,
) -> Response {
    if !is_html(content_type) {
        return response(StatusCode::OK, content_type, length, None, Body::empty());
    }
    let length = tokio::fs::read(path)
        .await
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|html| reload::inject(html, published).len() as u64)
        .unwrap_or(length);
    response(StatusCode::OK, content_type, length, None, Body::empty())
}

async fn full_file_response(path: PathBuf, content_type: &str, length: u64) -> Response {
    let file = match File::open(&path).await {
        Ok(file) => file,
        Err(source) => return internal_error(path, source),
    };
    let stream = ReaderStream::new(file);
    response(
        StatusCode::OK,
        content_type,
        length,
        None,
        Body::from_stream(stream),
    )
}

async fn ranged_response(
    path: PathBuf,
    content_type: &str,
    start: u64,
    end: u64,
    total_length: u64,
) -> Response {
    let mut file = match File::open(&path).await {
        Ok(file) => file,
        Err(source) => return internal_error(path, source),
    };
    if let Err(source) = file.seek(SeekFrom::Start(start)).await {
        return internal_error(path, source);
    }
    let length = end - start + 1;
    let stream = ReaderStream::new(file.take(length));
    response(
        StatusCode::PARTIAL_CONTENT,
        content_type,
        length,
        Some(format!("bytes {start}-{end}/{total_length}")),
        Body::from_stream(stream),
    )
}

fn response(
    status: StatusCode,
    content_type: &str,
    length: u64,
    content_range: Option<String>,
    body: Body,
) -> Response {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length)
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some(content_range) = content_range {
        builder = builder.header(header::CONTENT_RANGE, content_range);
    }
    builder
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn range_not_satisfiable(length: u64) -> Response {
    response(
        StatusCode::RANGE_NOT_SATISFIABLE,
        "application/octet-stream",
        0,
        Some(format!("bytes */{length}")),
        Body::empty(),
    )
}

fn not_found() -> Response {
    empty_response(StatusCode::NOT_FOUND)
}

fn internal_error(path: impl Into<PathBuf>, source: io::Error) -> Response {
    eprintln!(
        "{:?}",
        Report::new(RequestError {
            path: path.into(),
            source,
        })
    );
    empty_response(StatusCode::INTERNAL_SERVER_ERROR)
}

fn development_redirect(location: &str) -> Response {
    Redirect::temporary(location).into_response()
}

fn empty_response(status: StatusCode) -> Response {
    status.into_response()
}

fn development_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
}

fn content_type(path: &Path) -> String {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
    {
        "text/html; charset=utf-8".into()
    } else {
        mime.essence_str().into()
    }
}

fn is_html(content_type: &str) -> bool {
    content_type.starts_with("text/html")
}

fn reload_script() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/javascript; charset=utf-8")
        .body(Body::from(reload::script()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

enum RangeRequest {
    Full,
    Single { start: u64, end: u64 },
    Unsatisfiable,
}

fn parse_range(value: Option<&str>, length: u64) -> RangeRequest {
    let Some(value) = value else {
        return RangeRequest::Full;
    };
    let Some(specifications) = value.strip_prefix("bytes=") else {
        return RangeRequest::Full;
    };
    if specifications.contains(',') {
        return RangeRequest::Full;
    }
    let Some((start, end)) = specifications.split_once('-') else {
        return RangeRequest::Full;
    };
    let parsed = match (start.trim(), end.trim()) {
        ("", "") => return RangeRequest::Full,
        ("", suffix) => suffix.parse::<u64>().ok().map(|suffix| {
            if suffix == 0 || length == 0 {
                None
            } else {
                let count = suffix.min(length);
                Some((length - count, length - 1))
            }
        }),
        (start, "") => start
            .parse::<u64>()
            .ok()
            .map(|start| (start < length).then_some((start, length.saturating_sub(1)))),
        (start, end) => match (start.parse::<u64>(), end.parse::<u64>()) {
            (Ok(start), Ok(end)) if start <= end && start < length => {
                Some(Some((start, end.min(length - 1))))
            }
            (Ok(_), Ok(_)) => Some(None),
            _ => return RangeRequest::Full,
        },
    };
    match parsed.flatten() {
        Some((start, end)) => RangeRequest::Single { start, end },
        None => RangeRequest::Unsatisfiable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use axum::{
        body::to_bytes,
        http::{HeaderValue, Request, Uri},
    };
    use tower::ServiceExt;

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn assert_development_headers(headers: &HeaderMap) {
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    }

    #[test]
    fn resolves_only_safe_url_components() {
        assert_eq!(decoded_relative_path("post.html"), Some("post.html".into()));
        assert_eq!(
            decoded_relative_path("blog/"),
            Some("blog/index.html".into())
        );
        assert_eq!(decoded_relative_path("%2e%2e/secret"), None);
        assert_eq!(decoded_relative_path("a%2fb"), None);
        assert_eq!(decoded_relative_path("a%5cb"), None);
    }

    #[test]
    fn matches_percent_encoded_base_paths_without_decoding_the_prefix() {
        let published = PublishedSite {
            base_path: "/guide%20notes/".try_into().expect("valid base path"),
            output_root: PathBuf::from("public"),
            generation: 1,
        };

        assert_eq!(
            output_path("/guide%20notes/index.html", &published),
            Some(PathBuf::from("index.html"))
        );
    }

    #[test]
    fn supports_single_byte_ranges() {
        assert!(matches!(
            parse_range(Some("bytes=2-4"), 10),
            RangeRequest::Single { start: 2, end: 4 }
        ));
        assert!(matches!(
            parse_range(Some("bytes=-3"), 10),
            RangeRequest::Single { start: 7, end: 9 }
        ));
        assert!(matches!(
            parse_range(Some("bytes=8-"), 10),
            RangeRequest::Single { start: 8, end: 9 }
        ));
        assert!(matches!(
            parse_range(Some("bytes=99-"), 10),
            RangeRequest::Unsatisfiable
        ));
    }

    #[test]
    fn ignores_unsupported_or_multi_ranges() {
        assert!(matches!(
            parse_range(Some("items=1-2"), 10),
            RangeRequest::Full
        ));
        assert!(matches!(
            parse_range(Some("bytes=1-2,4-5"), 10),
            RangeRequest::Full
        ));
    }

    #[test]
    fn serves_base_path_html_with_reload_and_byte_ranges() {
        let root = std::env::temp_dir().join(format!(
            "heine-server-test-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create output fixture");
        fs::write(root.join("index.html"), "<head></head><body>home</body>")
            .expect("write html fixture");
        fs::write(root.join("audio.ogg"), b"0123456789").expect("write range fixture");
        let (sender, receiver) = tokio::sync::watch::channel(PublishedSite {
            base_path: "/guide/".try_into().expect("valid base path"),
            output_root: root.clone(),
            generation: 1,
        });
        let _sender = sender;
        let state = std::sync::Arc::new(ServerState {
            published: receiver,
        });
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let response = get_path(
                State(state.clone()),
                OriginalUri(Uri::from_static("/guide/")),
                HeaderMap::new(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            assert!(
                std::str::from_utf8(&body)
                    .expect("html")
                    .contains("/guide/__heine/reload.js?generation=1")
            );

            let response = head_path(
                State(state.clone()),
                OriginalUri(Uri::from_static("/guide/")),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_LENGTH],
                body.len().to_string()
            );
            let head_body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("head body");
            assert!(head_body.is_empty());

            let mut headers = HeaderMap::new();
            headers.insert(header::RANGE, HeaderValue::from_static("bytes=2-4"));
            let response = get_path(
                State(state.clone()),
                OriginalUri(Uri::from_static("/guide/audio.ogg")),
                headers,
            )
            .await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-4/10");
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            assert_eq!(&body[..], b"234");

            let mut headers = HeaderMap::new();
            headers.insert(header::RANGE, HeaderValue::from_static("bytes=99-"));
            let response = get_path(
                State(state.clone()),
                OriginalUri(Uri::from_static("/guide/audio.ogg")),
                headers,
            )
            .await;
            assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);

            let response = get_path(
                State(state.clone()),
                OriginalUri(Uri::from_static("/outside")),
                HeaderMap::new(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);

            let response = get_path(
                State(state),
                OriginalUri(Uri::from_static("/guide")),
                HeaderMap::new(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
            assert_eq!(response.headers()[header::LOCATION], "/guide/");
        });
        fs::remove_dir_all(root).expect("remove output fixture");
    }

    #[test]
    fn router_preserves_encoded_paths_and_development_response_headers() {
        let root = std::env::temp_dir().join(format!(
            "heine-server-router-test-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create output fixture");
        fs::write(root.join("index.html"), "<head></head>").expect("write html fixture");
        fs::write(root.join("audio.ogg"), b"0123456789").expect("write range fixture");
        fs::write(root.join(".heine-output"), "heine-output-v1\n").expect("write marker");
        let (_, receiver) = tokio::sync::watch::channel(PublishedSite {
            base_path: "/guide%20notes/".try_into().expect("valid base path"),
            output_root: root.clone(),
            generation: 1,
        });
        let state = std::sync::Arc::new(ServerState {
            published: receiver,
        });
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes/")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::OK);
            assert_development_headers(response.headers());

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .method("HEAD")
                        .uri("/guide%20notes/")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::OK);
            assert_development_headers(response.headers());
            assert!(
                to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("head body")
                    .is_empty()
            );

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
            assert_eq!(response.headers()[header::LOCATION], "/guide%20notes/");
            assert_development_headers(response.headers());

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes/__heine/events?generation=1")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::OK);
            assert_development_headers(response.headers());

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes/__heine/reload.js")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::OK);
            assert_development_headers(response.headers());

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes/audio.ogg")
                        .header(header::RANGE, "bytes=99-")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
            assert_development_headers(response.headers());

            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri("/guide%20notes/missing")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_development_headers(response.headers());

            for path in [
                "/guide%20notes/%2E%2E/secret",
                "/guide%20notes/a%2Fb",
                "/guide%20notes/.heine-output",
            ] {
                let response = router(state.clone())
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .body(Body::empty())
                            .expect("request"),
                    )
                    .await
                    .expect("router response");
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
                assert_development_headers(response.headers());
            }

            let response = router(state)
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/guide%20notes/")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_development_headers(response.headers());
        });
        fs::remove_dir_all(root).expect("remove output fixture");
    }
}
