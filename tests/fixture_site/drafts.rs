use super::*;

#[test]
fn ordinary_builds_exclude_drafts_but_preview_builds_include_them() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "{{ page.title }}:{{ page.draft }}");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'Home'\n",
    );
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\ntitle = 'Draft'\ncontent.main = 'draft.md'\n",
    );
    fixture.write("content/en/draft.md", "unpublished source");

    build_at(fixture.path(), BuildMode::Full).expect("published site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("published output"),
        "Home:false"
    );
    assert!(!fixture.path().join("public/draft.html").exists());
    assert!(!fixture.path().join("public/draft.md").exists());

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("draft preview builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("preview output"),
        "Home:false"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/draft.html")).expect("draft output"),
        "Draft:true"
    );
    assert!(!fixture.path().join("public/draft.md").exists());

    build_at(fixture.path(), BuildMode::Full).expect("published build replaces preview output");
    assert!(!fixture.path().join("public/draft.html").exists());
}

#[test]
fn ordinary_page_links_to_drafts_are_checked_against_the_published_catalog() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "{% set target = page(id='draft') %}");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("published pages cannot resolve an excluded draft");
    let report = format!("{error:?}");
    assert!(report.contains("declares `draft = true`"), "{report}");

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("preview resolves the same draft page");
}

#[test]
fn excluded_non_rendered_drafts_do_not_promise_preview_output() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "{% set target = page(id='draft') %}");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/en/draft.page", "draft = true\ntemplate = false\n");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("published pages cannot resolve an excluded draft");
    let report = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("ordinary builds cannot link to drafts"),
        "{report}"
    );

    let error = build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect_err("a non-rendered draft has no page target in a preview either");
    assert!(
        error
            .to_string()
            .contains("rendered page \"draft\" was not found"),
        "{error:?}"
    );
}

#[test]
fn page_exists_treats_an_excluded_draft_as_unavailable() {
    let fixture = Fixture::new();
    fixture.write(
        "templates/page.tera",
        "{% if page_exists(id='draft') %}present{% else %}absent{% endif %}",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\n",
    );

    build_at(fixture.path(), BuildMode::Full).expect("published site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html"))
            .expect("published page_exists output"),
        "absent"
    );

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("preview site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html"))
            .expect("preview page_exists output"),
        "present"
    );
}

#[test]
fn markdown_links_to_excluded_drafts_explain_the_publication_boundary() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'main.md'\n",
    );
    fixture.write("content/en/main.md", "Read [the draft](page:draft).\n");
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\ncontent.main = 'draft.md'\n",
    );
    fixture.write("content/en/draft.md", "Unfinished.\n");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("published Markdown cannot link to an excluded draft");
    let report = format!("{error:?}");
    assert!(report.contains("declares `draft = true`"), "{report}");
    assert!(report.contains("draft page referenced here"), "{report}");
    assert!(report.contains("heine serve --drafts"), "{report}");

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("preview resolves the Markdown link to the draft");
}

#[test]
fn drafts_enter_collection_relationships_only_in_a_preview() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[collections.posts]\norder = 'id-asc'\n");
    fixture.write(
        "templates/page.tera",
        "{% set posts = collection(name='posts') %}{{ posts.count }}",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncollections = ['posts']\n",
    );
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\ncollections = ['posts']\n",
    );

    build_at(fixture.path(), BuildMode::Full).expect("published site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("published count"),
        "1"
    );

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("draft preview builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("preview count"),
        "2"
    );
}

#[test]
fn draft_declarations_are_checked_while_incomplete_draft_details_wait_for_preview() {
    let fixture = Fixture::new();
    fixture.write("templates/page.tera", "{{ page.title }}");
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'missing.tera'\ncontent.main = '../outside.md'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("unsafe draft content claims are still checked");
    let report = format!("{error:?}");
    assert!(
        error.to_string().contains("escapes the content directory"),
        "{report}"
    );

    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'missing.tera'\ncontent.main = 'draft.md'\n",
    );
    build_at(fixture.path(), BuildMode::Full).expect("incomplete draft stays unpublished");
    let error = build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect_err("preview checks draft template details");
    assert!(format!("{error:?}").contains("missing.tera"));
}

#[test]
fn rejects_false_and_descriptor_draft_settings_at_the_authored_field() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "draft = false\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("false is not a setting");
    let report = format!("{error:?}");
    assert!(report.contains("field `draft`"), "{report}");
    assert!(report.contains("published page"), "{report}");

    fixture.write("content/en/index.page", "");
    fixture.write("content/en/_directory.toml", "draft = true\n");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("descriptors do not inherit draft state");
    let report = format!("{error:?}");
    assert!(report.contains("field `draft`"), "{report}");
    assert!(report.contains("directory descriptors"), "{report}");
}

#[test]
fn draft_only_margin_footnotes_keep_marker_configuration_intentional() {
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
        "content/en/_directory.toml",
        "[pages.footnotes]\ncontent = 'main'\n",
    );
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\ncontent.main = 'draft.md'\n",
    );
    fixture.write("content/en/draft.md", "Text[^note]\n\n[^note]: Note.\n");
    fixture.write("templates/page.tera", "{{ markdown(content='main') }}");

    build_at(fixture.path(), BuildMode::Full)
        .expect("draft-only footnote configuration remains intentional");
    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("preview checks the selected footnote source");
}

#[test]
fn preview_series_views_expose_draft_members_and_neighbors() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[series]\npath = 'series'\nindex_template = 'series-index.tera'\nlisting_template = 'series.tera'\nindex_title_key = 'series-index-title'\nper_page = 10\n\n[series.definitions.guide]\ntitle_key = 'series-guide-title'\n",
    );
    fixture.write(
        "locales/en/site.ftl",
        "series-index-title = Series\nseries-guide-title = Guide\n",
    );
    fixture.write(
        "templates/page.tera",
        "{% for membership in page.series %}{% if membership.next %}{{ membership.next.draft }}{% endif %}{% endfor %}",
    );
    fixture.write("templates/series-index.tera", "index");
    fixture.write(
        "templates/series.tera",
        "{% for item in pagination.pages %}{{ item.draft }}{% endfor %}",
    );
    fixture.write(
        "content/en/first.page",
        "template = 'page.tera'\ntitle = 'First'\n[[series]]\nid = 'guide'\nposition = 10\n",
    );
    fixture.write(
        "content/en/draft.page",
        "draft = true\ntemplate = 'page.tera'\ntitle = 'Draft'\n[[series]]\nid = 'guide'\nposition = 20\n",
    );

    build_at(fixture.path(), BuildMode::Full).expect("published site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/first.html")).expect("published page"),
        ""
    );

    build_at_with_options(
        fixture.path(),
        BuildOptions::new(BuildMode::Full).preview_drafts(),
    )
    .expect("draft preview builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/first.html")).expect("preview page"),
        "true"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/series/guide/index.html"))
            .expect("preview Series listing"),
        "falsetrue"
    );
}
