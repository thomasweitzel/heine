use super::*;

fn stream_copy_profile(name: &str, inputs: &str, result: &str) -> String {
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    format!(
        r#"
[renderers.{name}]
protocol = "stream"
program = "{program}"
timeout_seconds = 10
cache_identity = "copy-1"
inputs = {inputs}
result = "{result}"
[renderers.{name}.block]
arguments = {arguments}
"#,
    )
}

fn stream_picture_copy_profile(name: &str) -> String {
    let (program, arguments) = if cfg!(windows) {
        ("cmd", "[\"/C\", \"more\"]")
    } else {
        ("cat", "[]")
    };
    format!(
        r#"
[renderers.{name}]
protocol = "stream"
program = "{program}"
timeout_seconds = 10
cache_identity = "copy-picture-1"
inputs = ["body"]
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.{name}.block.light]
arguments = {arguments}
[renderers.{name}.block.dark]
arguments = {arguments}
"#,
    )
}

#[test]
fn renderer_requests_resolve_profiles_before_markdown_renders() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[site]
time_zone = "Europe/Berlin"

[i18n]
default_locale = "en"
[i18n.locales.en]
direction = "ltr"

[renderers.katex]
protocol = "stream"
program = "katex"
timeout_seconds = 10
cache_identity = "katex-0.16.27"
inputs = ["body"]
result = "html"
[renderers.katex.inline]
arguments = ["--format", "htmlAndMathml"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", ":render[unknown]`x^2`\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown profile fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("renderer profile \"unknown\" is not configured"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains(fixture.path().to_str().expect("UTF-8 fixture path")));

    fixture.write(
        "content/en/index.md",
        ":render[katex output=\"resource\"]`x^2`\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid output fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("cannot produce the requested output form"),
        "{diagnostic}"
    );
}

#[test]
fn permitted_stream_renderers_embed_checked_html_and_reuse_cached_results() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &stream_copy_profile("copy", "[\"body\"]", "html"),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "before\n\n:::render[copy]\n<span>cached result</span>\n:::\n\nafter\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let options = heine::BuildOptions::new(BuildMode::Full).allow_external_renderers();
    heine::build_at_with_options(fixture.path(), options).expect("permitted renderer builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<span>cached result</span>"), "{output}");

    let cache = fixture.path().join(".heine/cache/renderers/v1");
    let entries = fs::read_dir(&cache).expect("renderer cache").count();
    assert_eq!(entries, 2, "one cached result and its integrity record");
    let entry = fs::read_dir(&cache)
        .expect("renderer cache")
        .find_map(|entry| {
            let entry = entry.expect("cache entry");
            (entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "sha256"))
            .then_some(entry.path())
        })
        .expect("typed cache record");
    assert!(
        fs::read_to_string(entry)
            .expect("typed cache record")
            .starts_with("v1 html "),
        "cache records its result type"
    );

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full)
            .allow_external_renderers()
            .refresh_renderers(),
    )
    .expect("refresh bypasses and replaces cached renderer result");
    assert_eq!(
        fs::read_dir(&cache).expect("renderer cache").count(),
        2,
        "refresh replaces the same cache entry"
    );
}

#[test]
fn embedded_renderer_html_is_not_reparsed_as_markdown() {
    let fixture = Fixture::new();
    let inline_arguments = if cfg!(windows) {
        "[\"/C\", \"more\"]"
    } else {
        "[]"
    };
    fixture.write(
        "heine.toml",
        &format!(
            "{}\n[renderers.copy.inline]\narguments = {inline_arguments}\n",
            stream_copy_profile("copy", "[\"body\"]", "html")
        ),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "Before :render[copy]`<annotation>\\mathcal{F}[e](\\xi)</annotation>` after.\n\n:::render[copy]\n<annotation>\\mathcal{F}[e](\\xi)</annotation>\n:::\n\n## Following heading\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let options = heine::BuildOptions::new(BuildMode::Full).allow_external_renderers();
    heine::build_at_with_options(fixture.path(), options).expect("renderer builds");

    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<annotation>\\mathcal{F}[e](\\xi)</annotation>"),
        "{output}"
    );
    assert!(
        output.contains("<p>Before <annotation>\\mathcal{F}[e](\\xi)</annotation> after.</p>"),
        "{output}"
    );
    assert!(output.contains("<h2>Following heading</h2>"), "{output}");
    assert!(!output.contains("href=\"%5Cxi\""), "{output}");
}

#[test]
fn embedded_svg_renderers_attach_request_classes_without_changing_cached_output() {
    let fixture = Fixture::new();
    let inline_arguments = if cfg!(windows) {
        "[\"/C\", \"more\"]"
    } else {
        "[]"
    };
    let profile = stream_copy_profile("diagram", "[\"body\"]", "svg").replacen(
        "result = \"svg\"\n",
        "result = \"svg\"\nmaterializations = [\"embedded\"]\n",
        1,
    );
    fixture.write(
        "heine.toml",
        &format!("{profile}\n[renderers.diagram.inline]\narguments = {inline_arguments}\n"),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "First :render[diagram class=\"typst-inline-math\"]`<svg class=\"renderer\" xmlns=\"http://www.w3.org/2000/svg\"><use fill=\"#000000\"/></svg>` and :render[diagram class=\"second-math\"]`<svg class=\"renderer\" xmlns=\"http://www.w3.org/2000/svg\"><use fill=\"#000000\"/></svg>`.\n\n:::render[diagram class=\"typst-block-math\"]\n<svg class=\"renderer\" xmlns=\"http://www.w3.org/2000/svg\"><use fill=\"#000000\"/></svg>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("embedded SVG renderers build");

    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<p>First <svg class=\"renderer typst-inline-math\"")
            && output.contains("<svg class=\"renderer second-math\"")
            && output.contains("<svg class=\"renderer typst-block-math\""),
        "{output}"
    );
    assert!(output.contains("<use fill=\"#000000\""), "{output}");
    assert!(
        !output.contains("<p><svg"),
        "a block SVG replacement must not become paragraph content: {output}"
    );

    let cache = fixture.path().join(".heine/cache/renderers/v1");
    assert_eq!(
        fs::read_dir(&cache).expect("renderer cache").count(),
        4,
        "one inline and one block renderer result each have bytes and integrity records"
    );
    let cached_svg = fs::read_dir(&cache)
        .expect("renderer cache")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "bin"))
        .and_then(|path| fs::read_to_string(path).ok())
        .expect("cached SVG");
    assert!(cached_svg.contains("class=\"renderer\""), "{cached_svg}");
    assert!(!cached_svg.contains("typst-inline-math"), "{cached_svg}");
}

#[test]
fn renderer_classes_require_an_embedded_svg_result() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &stream_copy_profile("copy", "[\"body\"]", "html"),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[copy class=\"not-supported\"]\n<div>content</div>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("HTML class is rejected");
    assert!(format!("{error:?}").contains(
        "class is valid only for a picture renderer resource or an embedded SVG renderer result"
    ));

    fixture.write(
        "heine.toml",
        &stream_copy_profile("copy", "[\"body\"]", "svg").replacen(
            "result = \"svg\"\n",
            "result = \"svg\"\nmaterializations = [\"embedded\", \"resource\"]\ndefault_output = \"resource\"\n",
            1,
        ),
    );
    fixture.write(
        "content/en/index.md",
        ":::render[copy class=\"not-supported\" alt=\"diagram\"]\n<svg/>\n:::\n",
    );
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("SVG resource class is rejected");
    assert!(format!("{error:?}").contains(
        "class is valid only for a picture renderer resource or an embedded SVG renderer result"
    ));

    fixture.write(
        "heine.toml",
        &stream_copy_profile("copy", "[\"body\"]", "svg").replacen(
            "result = \"svg\"\n",
            "result = \"svg\"\nmaterializations = [\"embedded\"]\n",
            1,
        ),
    );
    fixture.write(
        "content/en/index.md",
        ":::render[copy class=\"repeated repeated\"]\n<svg/>\n:::\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("repeated class is rejected");
    assert!(format!("{error:?}").contains("renderer request class must not repeat a token"));
}

#[cfg(unix)]
#[test]
fn stream_picture_renderers_publish_two_appearance_resources_and_native_markup() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.diagram]
protocol = "stream"
program = "sh"
timeout_seconds = 10
cache_identity = "diagram-1"
inputs = ["body"]
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.diagram.block.light]
arguments = ["-c", "cat >/dev/null; printf '<svg xmlns=\"http://www.w3.org/2000/svg\"/>'"]
[renderers.diagram.block.dark]
arguments = ["-c", "cat >/dev/null; printf '<svg xmlns=\"http://www.w3.org/2000/svg\"><rect fill=\"white\"/></svg>'"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture\" caption=\"Request path\" class=\"diagram  align-center\"]\ninput\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("picture renderer builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<figure class=\"diagram align-center\">"),
        "{output}"
    );
    assert!(
        output.contains("<picture class=\"diagram align-center\">"),
        "{output}"
    );
    assert!(output.contains("prefers-color-scheme: dark"), "{output}");
    assert_eq!(
        fs::read_dir(fixture.path().join("public/heine-renderers"))
            .expect("resources")
            .count(),
        2
    );
    assert_eq!(
        fs::read_dir(fixture.path().join(".heine/cache/renderers/v1"))
            .expect("cache")
            .count(),
        4
    );
}

#[test]
fn picture_renderer_appearance_roles_use_distinct_cache_entries() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", &stream_picture_copy_profile("diagram"));
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture\"]\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let options = heine::BuildOptions::new(BuildMode::Full).allow_external_renderers();
    heine::build_at_with_options(fixture.path(), options).expect("picture renderer builds");

    assert_eq!(
        fs::read_dir(fixture.path().join("public/heine-renderers"))
            .expect("resources")
            .count(),
        1,
        "identical appearance bytes share one published resource"
    );
    let cache = fixture.path().join(".heine/cache/renderers/v1");
    assert_eq!(
        fs::read_dir(&cache).expect("cache").count(),
        4,
        "light and dark roles retain independent cache entries even with identical arguments and bytes"
    );

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full)
            .allow_external_renderers()
            .refresh_renderers(),
    )
    .expect("refresh rebuilds both picture appearances");
    assert_eq!(
        fs::read_dir(&cache).expect("cache").count(),
        4,
        "refresh replaces both appearance cache entries"
    );
}

#[test]
fn quick_build_updates_a_picture_renderer_request() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", &stream_picture_copy_profile("diagram"));
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture\"]\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let options = heine::BuildOptions::new(BuildMode::Full).allow_external_renderers();
    heine::build_at_with_options(fixture.path(), options).expect("initial picture build");
    let output_path = fixture.path().join("public/index.html");
    let before = fs::read_to_string(&output_path).expect("initial output");

    let markdown = fixture.path().join("content/en/index.md");
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture\"]\n<svg xmlns=\"http://www.w3.org/2000/svg\"><rect/></svg>\n:::\n",
    );
    fs::File::open(markdown)
        .expect("changed Markdown")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("changed Markdown timestamp");
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Quick).allow_external_renderers(),
    )
    .expect("quick build updates picture request");

    let after = fs::read_to_string(output_path).expect("quick output");
    assert_ne!(after, before, "quick build publishes the new picture URL");
}

#[cfg(unix)]
#[test]
fn workspace_picture_renderers_use_checked_output_extensions_and_identify_failed_appearances() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.diagram]
protocol = "workspace"
program = "sh"
timeout_seconds = 10
cache_identity = "workspace-picture-1"
inputs = ["body"]
input_extension = "svg"
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.diagram.block.light]
arguments = ["-c", "case \"$2\" in *.svg) cp \"$1\" \"$2\" ;; *) exit 17 ;; esac", "workspace", "{input_file}", "{output_file}"]
[renderers.diagram.block.dark]
arguments = ["-c", "case \"$2\" in *.svg) cp \"$1\" \"$2\" ;; *) exit 17 ;; esac", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture\"]\n<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let options = heine::BuildOptions::new(BuildMode::Full).allow_external_renderers();
    heine::build_at_with_options(fixture.path(), options).expect("workspace picture builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<picture>"), "{output}");

    fixture.write(
        "heine.toml",
        r#"
[renderers.diagram]
protocol = "workspace"
program = "sh"
timeout_seconds = 10
cache_identity = "workspace-picture-2"
inputs = ["body"]
input_extension = "svg"
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.diagram.block.light]
arguments = ["-c", "cp \"$1\" \"$2\"", "workspace", "{input_file}", "{output_file}"]
[renderers.diagram.block.dark]
arguments = ["-c", "exit 1", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("failed dark appearance rejects the picture request");
    assert!(
        format!("{error:?}").contains("dark picture renderer failed"),
        "{error:?}"
    );
}

#[cfg(unix)]
#[test]
fn picture_renderer_requests_check_profile_shape_and_picture_only_options() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.diagram]
protocol = "stream"
program = "sh"
timeout_seconds = 10
cache_identity = "diagram-1"
inputs = ["body"]
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.diagram.block]
arguments = []
"#,
    );
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("single picture variant fails");
    assert!(format!("{error:?}").contains("must declare light and dark variants"));

    fixture.write(
        "heine.toml",
        r#"
[renderers.diagram]
protocol = "stream"
program = "sh"
timeout_seconds = 10
cache_identity = "diagram-1"
inputs = ["body"]
result = "picture"
media_type = "image/svg+xml"
extension = "svg"
[renderers.diagram.inline.light]
arguments = ["-c", "printf '<svg xmlns=\"http://www.w3.org/2000/svg\"/>'"]
[renderers.diagram.inline.dark]
arguments = ["-c", "printf '<svg xmlns=\"http://www.w3.org/2000/svg\"/>'"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":render[diagram alt=\"diagram\" caption=\"no\"]`input`\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("inline caption fails");
    assert!(
        format!("{error:?}")
            .contains("caption is allowed only in a block picture renderer request")
    );
}

#[test]
fn stream_renderer_rejects_an_invalid_typed_embedded_result() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &stream_copy_profile("diagram", "[\"body\"]", "svg").replacen(
            "result = \"svg\"\n",
            "result = \"svg\"\nmaterializations = [\"embedded\"]\n",
            1,
        ),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram]\n<p>not SVG</p>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("typed SVG result is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("different document element"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
}

#[test]
fn stream_renderer_publishes_checked_svg_resources() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &stream_copy_profile("diagram", "[\"body\"]", "svg").replacen(
            "result = \"svg\"\n",
            "result = \"svg\"\nmaterializations = [\"resource\"]\n",
            1,
        ),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram alt=\"Architecture overview\"]\n<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("SVG resource renderer builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    let src = output
        .split("src=\"")
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .expect("SVG image");
    assert!(output.contains("alt=\"Architecture overview\""), "{output}");
    assert_eq!(
        fs::read_to_string(
            fixture
                .path()
                .join("public")
                .join(src.strip_prefix('/').expect("site-relative SVG URL")),
        )
        .expect("SVG resource"),
        "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"
    );
}

#[cfg(unix)]
#[test]
fn stream_renderer_times_out_at_the_request() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.wait]
protocol = "stream"
program = "sh"
timeout_seconds = 1
cache_identity = "wait-1"
inputs = ["body"]
result = "html"
[renderers.wait.block]
arguments = ["-c", "sleep 3"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", ":::render[wait]\nignored\n:::\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("slow direct child times out");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("exceeded its 1-second timeout"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn workspace_renderer_stages_body_files_and_bundles_in_an_isolated_layout() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.workspace]
protocol = "workspace"
program = "sh"
timeout_seconds = 10
cache_identity = "workspace-copy-1"
inputs = ["body", "file", "bundle"]
input_extension = "txt"
result = "html"
[renderers.workspace.block]
arguments = ["-c", "cat \"$1\" > \"$2\"", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[render.file]\nfile = '../assets/file.html'\n\n[render.bundle]\nroot = 'bundle'\nentry = 'main.html'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[workspace]\n<p>body</p>\n:::\n\n:::render[workspace source=\"file\"]\n:::\n\n:::render[workspace source=\"bundle\"]\n:::\n",
    );
    fixture.write("content/assets/file.html", "<p>file</p>\n");
    fixture.write("content/en/bundle/main.html", "<p>bundle</p>\n");
    fixture.write("content/en/bundle/imported.txt", "kept with bundle\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("workspace inputs render");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    for fragment in ["<p>body</p>", "<p>file</p>", "<p>bundle</p>"] {
        assert!(output.contains(fragment), "{output}");
    }
    assert!(
        !output.contains("<p><p>"),
        "adjacent block renderer replacements must remain separate blocks: {output}"
    );
    let cache = fixture.path().join(".heine/cache/renderers/v1");
    assert_eq!(
        fs::read_dir(&cache)
            .expect("workspace renderer cache")
            .count(),
        6,
        "each checked input form receives one cache entry and integrity record"
    );

    fixture.write(
        "content/en/bundle/imported.txt",
        "changed bundle dependency\n",
    );
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("changed bundle input rebuilds");
    assert_eq!(
        fs::read_dir(&cache)
            .expect("workspace renderer cache")
            .count(),
        8,
        "every checked bundle file contributes to the cache key"
    );
}

#[cfg(unix)]
#[test]
fn workspace_renderer_rejects_extra_output_files_and_times_out() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.workspace]
protocol = "workspace"
program = "sh"
timeout_seconds = 1
cache_identity = "workspace-check-1"
inputs = ["body"]
input_extension = "txt"
result = "html"
[renderers.workspace.block]
arguments = ["-c", "cp \"$1\" \"$2\"; touch \"$(dirname \"$2\")/extra\"", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[workspace]\n<p>body</p>\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("workspace extra output fails");
    assert!(format!("{error:?}").contains("besides the primary output"));

    fixture.write(
        "heine.toml",
        r#"
[renderers.workspace]
protocol = "workspace"
program = "sh"
timeout_seconds = 1
cache_identity = "workspace-timeout-1"
inputs = ["body"]
input_extension = "txt"
result = "html"
[renderers.workspace.block]
arguments = ["-c", "sleep 3", "workspace", "{output_file}"]
"#,
    );
    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("workspace timeout fails");
    assert!(format!("{error:?}").contains("exceeded its 1-second timeout"));
}

#[cfg(unix)]
#[test]
fn workspace_renderer_diagnostics_preserve_the_staged_body_location() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.workspace]
protocol = "workspace"
program = "sh"
timeout_seconds = 10
cache_identity = "workspace-diagnostic-1"
inputs = ["body"]
input_extension = "txt"
result = "html"
[renderers.workspace.block]
arguments = ["-c", "printf 'renderer error\\n  --> ../input/input.txt:2:3\\n' >&2; exit 1", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[workspace]\nfirst\nsecond\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("workspace renderer fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("../input/input.txt:2:3"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("paths and line numbers below refer to that input"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("renderer output:"), "{diagnostic}");
    assert!(
        diagnostic.contains("renderer exited with exit status"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn stream_renderer_diagnostics_preserve_program_output() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[renderers.stream]
protocol = "stream"
program = "sh"
timeout_seconds = 10
cache_identity = "stream-diagnostic-1"
inputs = ["body"]
result = "html"
[renderers.stream.block]
arguments = ["-c", "cat >/dev/null; printf 'stream renderer error\\n  at source input:2:3\\n' >&2; exit 1"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[stream]\nfirst\nsecond\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect_err("stream renderer fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("stream renderer error"), "{diagnostic}");
    assert!(diagnostic.contains("at source input:2:3"), "{diagnostic}");
    assert!(diagnostic.contains("renderer output:"), "{diagnostic}");
    assert!(
        diagnostic.contains("renderer exited with exit status"),
        "{diagnostic}"
    );
    assert!(
        !diagnostic.contains("isolated staged input"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn workspace_renderer_publishes_digest_resources_with_semantic_replacements() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        r#"
[site]
base_path = "/guide/"

[renderers.document]
protocol = "workspace"
program = "sh"
timeout_seconds = 10
cache_identity = "document-1"
inputs = ["body"]
input_extension = "txt"
result = "link"
media_type = "text/plain"
extension = "txt"
[renderers.document.block]
arguments = ["-c", "cp \"$1\" \"$2\"", "workspace", "{input_file}", "{output_file}"]
"#,
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[document label=\"Download report\"]\nreport bytes\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("resource renderer builds");
    let cache = fixture.path().join(".heine/cache/renderers/v1");
    assert_eq!(
        fs::read_dir(&cache)
            .expect("workspace renderer cache")
            .count(),
        2,
        "workspace result and integrity record are cached"
    );
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("cached workspace resource renderer builds");
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full)
            .allow_external_renderers()
            .refresh_renderers(),
    )
    .expect("refresh reruns and replaces the workspace resource cache entry");
    assert_eq!(
        fs::read_dir(&cache)
            .expect("workspace renderer cache")
            .count(),
        2,
        "refresh replaces the matching cache entry"
    );
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    let href = output
        .split("href=\"")
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .expect("resource link");
    assert!(
        href.starts_with("/guide/heine-renderers/sha256-"),
        "{output}"
    );
    assert!(output.contains(">Download report</a>"), "{output}");
    let path = href.strip_prefix("/guide/").expect("base path");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public").join(path)).expect("resource"),
        "report bytes"
    );

    let markdown = fixture.path().join("content/en/index.md");
    fixture.write(
        "content/en/index.md",
        ":::render[document label=\"Download report\"]\nupdated report bytes\n:::\n",
    );
    fs::File::open(&markdown)
        .expect("changed Markdown")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("changed Markdown timestamp");
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Quick).allow_external_renderers(),
    )
    .expect("quick build updates a changed renderer request");
    let output =
        fs::read_to_string(fixture.path().join("public/index.html")).expect("quick output");
    let href = output
        .split("href=\"")
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .expect("updated resource link");
    let path = href.strip_prefix("/guide/").expect("base path");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public").join(path)).expect("updated resource"),
        "updated report bytes"
    );
}

#[test]
fn renderer_requests_resolve_declared_files_without_publishing_them() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        &stream_copy_profile("diagram", "[\"file\"]", "html"),
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[render.architecture]\nfile = '../assets/architecture.txt'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::render[diagram source=\"architecture\"]\n:::\n",
    );
    fixture.write("content/assets/architecture.txt", "<p>diagram source</p>\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let permission = build_at(fixture.path(), BuildMode::Full)
        .expect_err("selected renderer requires explicit permission");
    assert!(
        format!("{permission:?}").contains("external renderer permission is required"),
        "{permission:?}"
    );
    heine::build_at_with_options(
        fixture.path(),
        heine::BuildOptions::new(BuildMode::Full).allow_external_renderers(),
    )
    .expect("declared file streams to the permitted renderer");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<p>diagram source</p>"), "{output}");

    fixture.write(
        "content/en/index.md",
        ":::render[diagram source=\"missing\"]\n:::\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown source fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("renderer source \"missing\" is not declared on this page"),
        "{diagnostic}"
    );

    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[render.report]\nroot = 'report'\nentry = 'main.txt'\n",
    );
    fixture.write("content/en/report/main.txt", "bundle entry\n");
    fixture.write(
        "content/en/index.md",
        ":::render[diagram source=\"report\"]\n:::\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unsupported bundle fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("does not accept the declared bundle source \"report\""),
        "{diagnostic}"
    );

    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[render.first]\nfile = '../assets/architecture.txt'\n\n[render.second]\nfile = '../assets/architecture.txt'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("overlapping sources fail");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:8"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("renderer source \"second\"")
            && diagnostic.contains("renderer source \"first\""),
        "{diagnostic}"
    );
}

#[test]
fn renderer_source_declarations_check_bundle_boundaries() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "[render.report]\nroot = 'report'\nentry = '../outside.typ'\n",
    );
    fixture.write("content/en/report/outside.typ", "report");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("bundle entry escapes");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:3"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("escapes the content directory"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_exposes_a_toc_and_matching_heading_ids() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 3\n",
    );
    fixture.write("content/en/index.page", "content.main = 'index.md'\n");
    fixture.write(
        "content/en/index.md",
        "# Title\n\n## Über **view**\n\n### Über view\n\n## Über view\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }},{{ heading.depth }},{{ heading.fragment }}:{{ heading.text }}]{% endfor %}|{{ markdown(content='main') }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("[2,1,über-view:Über view]"), "{output}");
    assert!(output.contains("[3,2,über-view-2:Über view]"), "{output}");
    assert!(output.contains("[2,1,über-view-3:Über view]"), "{output}");
    assert!(output.contains("<h1 id=\"title\">Title</h1>"), "{output}");
    assert!(
        output.contains("<h2 id=\"über-view\">Über <strong>view</strong></h2>"),
        "{output}"
    );
}

#[test]
fn rejects_the_plain_filter_for_selected_toc_content_at_its_call_site() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write("content/en/index.md", "## Heading\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("filter use fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("markdown(content="), "{diagnostic}");
}

#[test]
fn selected_toc_content_is_rendered_exactly_once() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write("content/en/index.md", "## Heading\n");
    fixture.write("templates/page.tera", "<p>No content.</p>");

    let missing = build_at(fixture.path(), BuildMode::Full).expect_err("source must render");
    assert!(
        missing
            .to_string()
            .contains("selected table-of-contents source \"main\" was not rendered"),
        "{missing:?}"
    );
    assert!(
        missing.to_string().contains("markdown(content=\"main\")"),
        "{missing:?}"
    );

    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ markdown(content='main') }}",
    );
    let repeated = build_at(fixture.path(), BuildMode::Full).expect_err("source renders twice");
    assert!(
        repeated
            .to_string()
            .contains("selected table-of-contents source \"main\" was rendered more than once"),
        "{repeated:?}"
    );
}

#[test]
fn page_toc_replaces_a_directory_default_and_ignores_raw_html_headings() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 2\n",
    );
    fixture.write(
        "content/en/index.page",
        "content.main = 'index.md'\n\n[toc]\ncontent = 'main'\nmin_level = 3\nmax_level = 3\n",
    );
    fixture.write(
        "content/en/index.md",
        "## Outside\n\n### Inside\n\n<h3>Raw heading</h3>\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }}:{{ heading.fragment }}]{% endfor %}{{ markdown(content='main') }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("[3:inside]"), "{output}");
    assert!(!output.contains("[2:outside]"), "{output}");
    assert!(
        output.contains("<h2 id=\"outside\">Outside</h2>"),
        "{output}"
    );
    assert!(output.contains("<h3 id=\"inside\">Inside</h3>"), "{output}");
    assert!(output.contains("<h3>Raw heading</h3>"), "{output}");
}

#[test]
fn nearest_directory_toc_default_wins() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 2\n",
    );
    fixture.write(
        "content/en/guide/_directory.toml",
        "[pages.toc]\ncontent = 'main'\nmin_level = 3\nmax_level = 3\n",
    );
    fixture.write("content/en/guide/index.page", "content.main = 'main.md'\n");
    fixture.write("content/en/guide/main.md", "## Outer\n\n### Inner\n");
    fixture.write(
        "templates/page.tera",
        "{% for heading in page.toc.headings %}[{{ heading.level }}:{{ heading.fragment }}]{% endfor %}{{ markdown(content='main') }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output =
        fs::read_to_string(fixture.path().join("public/guide/index.html")).expect("output");
    assert!(output.contains("[3:inner]"), "{output}");
}

#[test]
fn toc_opt_out_requires_an_inherited_default() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntoc = false\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("inert opt-out fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:2"),
        "{diagnostic}"
    );
    assert!(
        error
            .to_string()
            .contains("otherwise effective inherited table of contents"),
        "{diagnostic}"
    );
}

#[test]
fn toc_configuration_errors_point_to_the_authored_field() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n\n[toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:6"),
        "{diagnostic}"
    );
    assert!(
        error
            .to_string()
            .contains("toc.min_level must be an integer"),
        "{diagnostic}"
    );
}

#[test]
fn toc_diagnostics_do_not_label_an_unrelated_extra_key() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n\n[extra]\nmin_level = 2\n\n[toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:9"),
        "{diagnostic}"
    );
}

#[test]
fn directory_toc_diagnostics_do_not_label_an_unrelated_extra_key() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[extra]\nmin_level = 2\n\n[pages]\ntemplate = 'page.tera'\n\n[pages.toc]\ncontent = 'main'\nmin_level = 7\n",
    );
    fixture.write("content/en/index.page", "content.main = 'main.md'\n");
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid level fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/_directory.toml:9"),
        "{diagnostic}"
    );
}

#[test]
fn toc_requires_one_unsegmented_markdown_content_file() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "<!-- segment: first -->\n## Heading\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("segmented source fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.page:5"),
        "{diagnostic}"
    );
    assert!(
        error
            .to_string()
            .contains("one unsegmented Markdown document"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_requires_current_page_markdown_content() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.data = 'data.toml'\n",
    );
    fixture.write("content/en/data.toml", "name = 'Heine'\n");
    fixture.write("templates/page.tera", "{{ markdown(content='data') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("non-Markdown content fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        error
            .to_string()
            .contains("declared unsegmented .md content name"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_without_a_toc_renders_ordinary_markdown() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<h2>Heading</h2>"), "{output}");
    assert!(!output.contains("id=\"heading\""), "{output}");
}

#[test]
fn margin_footnotes_render_local_checked_note_instances() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes', 'tables']\nmath = 'mathml'\n\n[markdown.tables]\nscroll_message = 'table-scroll'\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\ntable-scroll = Scrollable table\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "## Heine footnote note 1\n\nA locally explained claim.[^one] Another claim.[^two] The first claim again.[^one]\n\n[^one]: Its local explanation with a [checked page](page:index).\n\n    A second paragraph has inline math $x^2$, an ![asset](asset:assets/images/note.svg), and a list:\n\n    - a local item\n    - another item\n\n    | Term | Meaning |\n    | --- | --- |\n    | Note | Local |\n\n    ```rust\n    let note = true;\n    ```\n\n[^two]: A second explanation.\n",
    );
    fixture.write("content/assets/images/note.svg", "<svg></svg>");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<h2 id=\"heine-footnote-note-1\">Heine footnote note 1</h2>"),
        "{output}"
    );
    assert_eq!(
        output.matches("id=\"__heine-footnote-note-1\"").count(),
        1,
        "{output}"
    );
    assert!(
        output.contains("<label for=\"__heine-footnote-toggle-1\">1</label>"),
        "{output}"
    );
    assert!(output.contains("aria-label=\"Footnote"), "{output}");
    assert!(
        output.contains(
            "<aside class=\"heine-footnote\" id=\"__heine-footnote-note-1\" role=\"doc-footnote\">"
        ),
        "{output}"
    );
    assert!(
        output.contains("<a href=\"/index.html\">checked page</a>"),
        "{output}"
    );
    assert!(
        output.contains("<p>A second paragraph has inline math <math display=\"inline\">"),
        "{output}"
    );
    assert!(
        output.contains("src=\"/assets/images/note.svg\" alt=\"asset\""),
        "{output}"
    );
    assert!(output.contains("<ul>\n<li>a local item</li>"), "{output}");
    assert!(
        output.contains("<div class=\"heine-table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Scrollable table\"><table>"),
        "{output}"
    );
    assert!(
        output.contains("<pre><code class=\"language-rust\">let note = true;"),
        "{output}"
    );
    assert!(
        output.contains("<label for=\"__heine-footnote-toggle-2\">2</label>"),
        "{output}"
    );
    assert!(
        output.contains("<div class=\"heine-table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Scrollable table\"><table>"),
        "{output}"
    );
    assert!(
        output.contains("<label for=\"__heine-footnote-toggle-3\">1</label>"),
        "{output}"
    );
    assert!(
        output.contains("id=\"__heine-footnote-note-3\" role=\"doc-footnote\""),
        "{output}"
    );
    assert!(!output.contains("footnote-definition"), "{output}");
}

#[test]
fn margin_footnotes_require_one_source_aware_render() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "A note.[^one]\n\n[^one]: Explanation.\n",
    );
    fixture.write("templates/page.tera", "<p>Nothing renders the source.</p>");

    let missing = build_at(fixture.path(), BuildMode::Full).expect_err("source must render");
    assert!(
        missing
            .to_string()
            .contains("was not rendered with markdown"),
        "{missing:?}"
    );

    fixture.write(
        "templates/page.tera",
        "{{ markdown(content='main') }}{{ markdown(content='main') }}",
    );
    let repeated = build_at(fixture.path(), BuildMode::Full).expect_err("source must render once");
    assert!(
        repeated.to_string().contains("was rendered more than once"),
        "{repeated:?}"
    );
}

#[test]
fn margin_footnote_relationships_and_placement_are_checked_at_markdown() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    fixture.write("content/en/index.md", "A missing note.[^missing]\n");

    let missing = build_at(fixture.path(), BuildMode::Full).expect_err("definition is required");
    assert!(
        missing.to_string().contains("has no definition"),
        "{missing:?}"
    );

    fixture.write(
        "content/en/index.md",
        "- A list note.[^one]\n\n[^one]: Explanation.\n",
    );
    let placement =
        build_at(fixture.path(), BuildMode::Full).expect_err("list placement is rejected");
    assert!(
        placement
            .to_string()
            .contains("need an ordinary paragraph or heading"),
        "{placement:?}"
    );

    fixture.write(
        "content/en/index.md",
        "A note.[^one]\n\n[^one]: First definition.\n\n[^one]: Second definition.\n",
    );
    let duplicate = build_at(fixture.path(), BuildMode::Full).expect_err("definition is unique");
    assert!(
        duplicate.to_string().contains("declared more than once"),
        "{duplicate:?}"
    );

    fixture.write(
        "content/en/index.md",
        "A footnote definition remains.[^used]\n\n[^used]: Used.\n\n[^unused]: Unused.\n",
    );
    let unused = build_at(fixture.path(), BuildMode::Full).expect_err("definition is used");
    assert!(
        unused.to_string().contains("is not referenced"),
        "{unused:?}"
    );

    fixture.write(
        "content/en/index.md",
        "A note.[^one]\n\n[^one]: A nested unresolved note.[^missing]\n",
    );
    let nested_missing =
        build_at(fixture.path(), BuildMode::Full).expect_err("nested references are rejected");
    assert!(
        nested_missing
            .to_string()
            .contains("cannot contain another footnote reference"),
        "{nested_missing:?}"
    );

    fixture.write(
        "content/en/index.md",
        "A [linked unresolved note[^missing]](https://example.test).\n",
    );
    let linked_missing =
        build_at(fixture.path(), BuildMode::Full).expect_err("linked references are rejected");
    assert!(
        linked_missing
            .to_string()
            .contains("need an ordinary paragraph or heading"),
        "{linked_missing:?}"
    );
}

#[test]
fn margin_footnote_sources_render_checked_picture_blocks() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");
    fixture.write(
        "content/en/index.md",
        ":::picture\nalt = \"A portrait\"\nlight = \"asset:images/light.png\"\ndark = \"asset:images/dark.png\"\n:::\n\nA note.[^one]\n\n[^one]: Explanation.\n",
    );
    write_bytes(&fixture, "content/en/images/light.png", PIXEL_PNG);
    write_bytes(&fixture, "content/en/images/dark.png", PIXEL_PNG);

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<picture>"), "{output}");
    assert!(output.contains("heine-annotated-block"), "{output}");
}

#[test]
fn margin_footnote_defaults_and_marker_configuration_are_checked() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/guide/_directory.toml",
        "[pages]\ntemplate = 'page.tera'\n\n[pages.footnotes]\ncontent = 'main'\n",
    );
    fixture.write("content/en/guide/index.page", "content.main = 'index.md'\n");
    fixture.write(
        "content/en/guide/index.md",
        "A note.[^one]\n\n[^one]: Explanation.\n",
    );
    fixture.write("content/en/guide/other.page", "content.main = 'other.md'\n");
    fixture.write(
        "content/en/guide/other.md",
        "Another note.[^one]\n\n[^one]: Explanation.\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("inherited setting builds");

    fixture.write(
        "content/en/guide/index.page",
        "content.main = 'index.md'\nfootnotes = false\n",
    );
    build_at(fixture.path(), BuildMode::Full).expect("page may suppress inherited setting");

    let unused = Fixture::new();
    unused.write(
        "heine.toml",
        "[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    unused.write("content/en/index.page", "template = 'page.tera'\n");
    unused.write("templates/page.tera", "<p>Ordinary page.</p>");
    let error = build_at(unused.path(), BuildMode::Full).expect_err("inactive global setting");
    assert!(
        error
            .to_string()
            .contains("invalid margin-footnote marker-message configuration"),
        "{error:?}"
    );
}

#[test]
fn margin_footnotes_share_source_aware_toc_rendering_and_reject_the_raw_filter() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "## Chronology[^one]\n\nProse.\n\n[^one]: A note heading\n\n    ## Not a page heading\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("shared source builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<h2 id=\"chronology\">Chronology"),
        "{output}"
    );
    assert!(!output.contains("id=\"not-a-page-heading\""), "{output}");

    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("raw filter is ambiguous");
    assert!(
        error.to_string().contains("must use markdown(content"),
        "{error:?}"
    );
}

#[test]
fn source_aware_markdown_reports_every_unrendered_selected_source() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[content]\nmain = 'main.md'\nnotes = 'notes.md'\n\n[toc]\ncontent = 'main'\n\n[footnotes]\ncontent = 'notes'\n",
    );
    fixture.write("content/en/main.md", "## Heading\n");
    fixture.write(
        "content/en/notes.md",
        "A note.[^one]\n\n[^one]: Explanation.\n",
    );
    fixture.write(
        "templates/page.tera",
        "<p>Nothing renders either source.</p>",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("both sources must render");
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains("table-of-contents source \"main\" was not rendered"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("markdown(content=\"main\")"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("margin-footnote source \"notes\" was not rendered"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("markdown(content=\"notes\")"),
        "{diagnostic}"
    );
}

#[test]
fn source_aware_markdown_keeps_tables_plain_without_responsive_configuration() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes', 'tables']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "| Term | Meaning |\n| --- | --- |\n| Note | Local |\n\nA claim.[^one]\n\n[^one]: Its local explanation.\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("source-aware Markdown builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<table>"), "{output}");
    assert!(!output.contains("heine-table-scroll"), "{output}");
}

#[test]
fn markdown_extensions_and_mathml_are_rendered_when_enabled() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['tables', 'strikethrough', 'task_lists', 'footnotes']\nmath = 'mathml'\n\n[markdown.tables]\nscroll_message = 'table-scroll'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("locales/en/site.ftl", "table-scroll = Scrollable table\n");
    fixture.write(
        "content/en/index.md",
        "~~old~~\n\n| Name |\n| --- |\n| Heine |\n\n<table><tr><td>Raw table</td></tr></table>\n\n- [x] **Publish** `site`\n  - [ ] Nested task\n- [ ] <span>Quote \"and\" escape</span>\n- [ ]\n\nA note[^one].\n\n[^one]: Footnote\n\n$x^2$\n\n$$\\frac{1}{2}$$\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert!(output.contains("<del>old</del>"), "{output}");
    assert!(
        output.contains("<div class=\"heine-table-scroll\" tabindex=\"0\" role=\"region\" aria-label=\"Scrollable table\"><table>"),
        "{output}"
    );
    assert_eq!(output.matches("heine-table-scroll").count(), 1, "{output}");
    assert!(
        output.contains("<table><tr><td>Raw table</td></tr></table>"),
        "{output}"
    );
    assert!(
        output.contains("type=\"checkbox\" checked=\"\" aria-label=\"Publish site\""),
        "{output}"
    );
    assert!(output.contains("aria-label=\"Nested task\""), "{output}");
    assert!(
        output.contains("aria-label=\"Quote &quot;and&quot; escape\""),
        "{output}"
    );
    assert!(
        output.contains("<input disabled=\"\" type=\"checkbox\">"),
        "{output}"
    );
    assert!(output.contains("footnote-reference"), "{output}");
    assert!(output.contains("<math"), "{output}");
    assert!(output.contains("display=\"block\""), "{output}");
}

#[test]
fn ordinary_footnote_references_need_definitions() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nextensions = ['footnotes']\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "A missing note.[^first] Another missing note.[^second]\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("reference needs a definition");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/index.md:1:16"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("reference without") && diagnostic.contains("matching definition"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("add one [^label]") && diagnostic.contains("remove this reference"),
        "{diagnostic}"
    );

    fixture.write(
        "content/en/index.md",
        "A literal \\[^missing].\n\n`[^also-literal]`\n\n```markdown\nA documented [^example].\n```\n",
    );
    build_at(fixture.path(), BuildMode::Full)
        .expect("escaped, code-span, and fenced footnote syntax are literal text");
}

#[test]
fn ordinary_footnotes_use_generated_fragments_separate_from_headings() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nextensions = ['footnotes']\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[toc]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        "## Audio\n\nA note.[^audio]\n\n[^audio]: An ordinary footnote.\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<h2 id=\"audio\">Audio</h2>"), "{output}");
    assert!(
        output.contains("<a href=\"#__heine-footnote-definition-1\">1</a>"),
        "{output}"
    );
    assert!(
        output.contains("<div class=\"footnote-definition\" id=\"__heine-footnote-definition-1\">"),
        "{output}"
    );
    assert_eq!(output.matches("id=\"audio\"").count(), 1, "{output}");
}

#[test]
fn tex_math_preserves_delimiters_without_markdown_or_html_interpretation() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'tex'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "Inline $a_b * c <tag>$.\n\n$$\\unknown{a_b} * <tag>$$\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("site builds without TeX validation");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert!(output.contains("$a_b * c &lt;tag&gt;$"), "{output}");
    assert!(
        output.contains("$$\\unknown{a_b} * &lt;tag&gt;$$"),
        "{output}"
    );
    assert!(!output.contains("<math"), "{output}");
}

#[test]
fn reports_invalid_math_in_markdown_content_at_its_source() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'mathml'\n");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "The prose also contains a^b^c. The invalid expression is $a^b^c$.",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("math should be invalid");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/en/index.md:1:58"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("invalid TeX here"), "{diagnostic}");
    assert!(diagnostic.contains("invalid TeX here"), "{diagnostic}");
}

#[test]
fn reports_invalid_math_in_a_template_at_the_filter_call_site() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[markdown]\nmath = 'mathml'\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", r"{{ '$a^b^c$' | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("math should be invalid");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(diagnostic.contains("a^b^c"), "{diagnostic}");
}

#[test]
fn generates_one_preference_aware_stylesheet_for_class_based_code() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\nbase_path = '/guide/'\n\n[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```rust\nfn main() {}\n```\n");
    fixture.write(
        "templates/page.tera",
        "<link rel='stylesheet' href='{{ highlight_css() }}'>{{ page.content.main | markdown }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    let html = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    let css = fs::read_to_string(fixture.path().join("public/styles/highlighting.css"))
        .expect("generated stylesheet");

    assert!(html.contains("/guide/styles/highlighting.css"), "{html}");
    assert!(html.contains("heine-l-code heine-d-code"), "{html}");
    assert!(!html.contains("<pre class=\"giallo\" style="), "{html}");
    assert!(css.contains("@media (prefers-color-scheme: dark)"), "{css}");
    assert!(
        css.contains(".giallo > code { background: transparent; }"),
        "{css}"
    );
    assert!(css.contains("heine-l-"), "{css}");
    assert!(css.contains("heine-d-"), "{css}");
}

#[test]
fn highlights_unlabelled_and_indented_code_as_plain() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "```\nunlabelled fence\n```\n\n    indented block\n",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("plain code blocks build");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");

    assert_eq!(output.matches("<pre class=\"giallo").count(), 2, "{output}");
    assert!(output.contains("unlabelled fence"), "{output}");
    assert!(output.contains("indented block"), "{output}");
    assert!(!output.contains("<pre><code>"), "{output}");
}

#[test]
fn reports_unknown_fenced_languages_at_the_markdown_source() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```not-a-language\ntext\n```\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown language fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md"), "{diagnostic}");
    assert!(diagnostic.contains("not-a-language"), "{diagnostic}");
}

#[test]
fn highlight_css_must_not_be_called_when_highlighting_is_disabled() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ highlight_css() }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("function is unavailable");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");
    assert!(
        error
            .to_string()
            .contains("syntax highlighting is not enabled"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_a_highlighting_stylesheet_that_conflicts_with_a_copied_asset() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/styles/highlighting.css", "authored CSS");
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("syntax-highlighting") && diagnostic.contains("stylesheet"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("styles/highlighting.css"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_a_highlighting_stylesheet_that_would_be_an_asset_directory() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/en/styles/highlighting.css/override.css",
        "authored CSS",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("directory collision fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("stylesheet"), "{diagnostic}");
}

#[test]
fn reports_an_unsafe_highlighting_output_path_at_its_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting?.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("stylesheet is unsafe");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("portable filename components"),
        "{diagnostic}"
    );
}

#[test]
fn reports_both_configuration_and_page_locations_for_a_stylesheet_collision() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/styles/highlighting.page",
        "template = 'page.tera'\nsuffix = '.css'\n",
    );
    fixture.write("templates/page.tera", "page");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("collision fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/en/styles/highlighting.page:2"),
        "{diagnostic}"
    );
}

#[test]
fn plain_missing_language_policy_keeps_unknown_fences_renderable() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\nmissing_language = 'plain'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "```unknown\ntext\n```\n");
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    build_at(fixture.path(), BuildMode::Full).expect("plain fallback builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<pre class=\"giallo"), "{output}");
    assert!(!output.contains("style="), "{output}");
}

#[test]
fn reports_an_unknown_highlighting_theme_at_the_configuration_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown.highlighting]\nstylesheet = 'styles/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'not-a-theme'\ndark = 'github-dark'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("theme is invalid");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:5"), "{diagnostic}");
    assert!(diagnostic.contains("not-a-theme"), "{diagnostic}");
}

#[test]
fn renders_markdown_block_components_inside_the_checked_document_plan() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/_directory.toml",
        "[pages.toc]\ncontent = 'main'\nmin_level = 2\nmax_level = 3\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "# Before\n\n:::component[site.note kind = \"warning\"]\n## Inside\n\nA [checked page](page:index).\n:::\n\n# After\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note(kind: string) %}<aside class=\"note-{{ kind }}\">{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("component body renders with its page");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(output.contains("<h1 id=\"before\">Before</h1>"), "{output}");
    assert!(
        output.contains("<aside class=\"note-warning\"><h2 id=\"inside\">Inside</h2>"),
        "{output}"
    );
    assert!(output.contains("href=\"/index.html\""), "{output}");
    assert!(output.contains("<h1 id=\"after\">After</h1>"), "{output}");
}

#[test]
fn keeps_all_block_directive_syntax_literal_in_documentation_contexts() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        r#"Use `:::component[site.note]`, `:::picture`, and `:::render[copy]` when documenting syntax.

```markdown
:::component[site.note]
:::picture
:::render[copy]
```

<div>
:::component[site.note]
:::picture
:::render[copy]
</div>

<span data-component=":::component[site.note]" data-picture=":::picture" data-render=":render[copy]`body`">literal</span>
"#,
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("literal directive syntax builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<code>:::component[site.note]</code>"),
        "{output}"
    );
    assert!(output.contains("<code>:::picture</code>"), "{output}");
    assert!(output.contains("<code>:::render[copy]</code>"), "{output}");
    assert!(
        output.contains("data-render=\":render[copy]`body`\""),
        "{output}"
    );
}

#[test]
fn reserves_block_directives_inside_lists_and_quotes() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "- :::component[site.note]\n  A note.\n  :::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("a component in a list is reserved for a future contract");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("block directive in a list item"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("ordinary top-level block"),
        "{diagnostic}"
    );
}

#[test]
fn reserves_block_directives_inside_paragraph_continuations() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "Before the directive\n:::component[site.note]\nA note.\n:::\n",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("a component in a paragraph continuation is reserved");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("block directive in a paragraph continuation"),
        "{diagnostic}"
    );
}

#[test]
fn renders_markdown_block_components_from_crlf_source() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note kind = \"warning\"]\r\nA checked note.\r\n:::\r\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note(kind: string) %}<aside class=\"note-{{ kind }}\">{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("CRLF component source builds");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert!(
        output.contains("<aside class=\"note-warning\"><p>A checked note.</p>"),
        "{output}"
    );
}

#[test]
fn reports_an_invalid_markdown_component_contract_at_its_markdown_call() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA note.\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() {\"heine.body_may_hide\": false} %}<aside>{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid contract fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(diagnostic.contains("heine.body_may_hide"), "{diagnostic}");
}

#[test]
fn rejects_headings_in_a_markdown_component_that_may_hide_its_body() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.disclosure]\n## Hidden heading\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.disclosure() {\"heine.body_may_hide\": true} %}<details>{{ body }}</details>{% endcomponent site.disclosure %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("hidden heading fails the component contract");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("cannot contain Markdown"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("headings"), "{diagnostic}");
}

#[test]
fn reports_component_template_failures_with_the_markdown_call() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA note.\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}{{ missing }}{{ body }}{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("component failure fails");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("templates/components.tera"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
}

#[test]
fn rejects_a_markdown_component_that_does_not_preserve_its_body_once() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA note.\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside>{{ body }}{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicated body fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("content/en/index.md:1"), "{diagnostic}");
    assert!(
        diagnostic.contains("preserve its Markdown body exactly once"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_the_markdown_filter_for_block_component_content() {
    let fixture = Fixture::new();
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA note.\n:::\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside>{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("filter cannot use plan");
    assert!(
        error
            .to_string()
            .contains("must use markdown(content=\"…\")"),
        "{error:?}"
    );
}

#[test]
fn keeps_margin_footnotes_local_when_a_component_wraps_their_prose() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[markdown]\nextensions = ['footnotes']\n\n[markdown.margin_footnotes]\nmarker_message = 'footnote-marker'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "footnote-marker = Footnote { $number }\n",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n\n[footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/index.md",
        ":::component[site.note]\nA note.[^one]\n:::\n\n[^one]: Explanation.\n",
    );
    fixture.write(
        "templates/components.tera",
        "{% component site.note() %}<aside class=\"note\">{{ body }}</aside>{% endcomponent site.note %}",
    );
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full).expect("component preserves local notes");
    let output = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    let note_start = output
        .find("<aside class=\"note\">")
        .expect("component output");
    let marker = output
        .find("heine-annotated-block")
        .expect("local note output");
    assert!(marker > note_start, "{output}");
    assert!(output.contains("Explanation."), "{output}");
}
