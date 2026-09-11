use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn publishes_license_texts_and_exposes_asset_licensing_to_templates() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[site]\nbase_path = '/guide/'\n");
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MIT\nSPDX-FileComment: https://example.test/logo\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT license text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/logo.svg') %}{{ logo.licensing.copyrights[0] }}|{{ logo.licensing.expressions[0] }}|{{ logo.licensing.license_texts[0].url }}|{% for item in attributions() %}{{ item.asset.output_path }}:{{ item.asset.url }}{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("rendered page"),
        "Example Authors|MIT|/guide/LICENSES/MIT.txt|assets/logo.svg:/guide/assets/logo.svg"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/LICENSES/MIT.txt"))
            .expect("published license text"),
        "MIT license text\n"
    );
    assert!(
        !fixture
            .path()
            .join("public/assets/logo.svg.license")
            .exists()
    );
}

#[test]
fn distinguishes_published_attribution_paths_from_locale_relative_asset_ids() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    fixture.write("content/de/logo.svg", "<svg/>");
    fixture.write(
        "content/de/logo.svg.license",
        "SPDX-FileCopyrightText: Beispielautor\nSPDX-License-Identifier: MIT\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT license text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% if locale.code == 'de' %}{% set logo = asset(id='logo.svg') %}{{ logo.url }}|{% for item in attributions() %}{{ item.asset.output_path }}:{{ item.asset.url }}{% endfor %}{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German page"),
        "/de/logo.svg|de/logo.svg:/de/logo.svg"
    );
}

#[test]
fn exposes_null_licensing_for_assets_without_a_sidecar() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/logo.svg') %}{% if logo.licensing %}licensed{% else %}unlicensed{% endif %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("site builds");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("rendered page"),
        "unlicensed"
    );
}

#[test]
fn groups_attach_shared_facts_and_required_coverage_to_copied_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[licensing]\nrequired = true\n\n[[licensing.groups]]\nassets = ['assets/fonts/roman.woff2', 'assets/fonts/bold.woff2']\nSPDX-FileCopyrightText = ['2000 Example Foundry', '2001 Example Contributors']\nSPDX-License-Identifier = ['OFL-1.1', 'MIT']\nSPDX-FileComment = ['https://example.test/fonts']\n",
    );
    fixture.write("content/assets/fonts/roman.woff2", "roman");
    fixture.write("content/assets/fonts/bold.woff2", "bold");
    fixture.write("LICENSES/OFL-1.1.txt", "OFL text\n");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% for item in attributions() %}{{ item.asset.output_path }}={{ item.licensing.copyrights | join(sep=',') }}:{{ item.licensing.expressions | join(sep=',') }};{% endfor %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("grouped assets build");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("rendered page"),
        "assets/fonts/bold.woff2=2000 Example Foundry,2001 Example Contributors:OFL-1.1,MIT;assets/fonts/roman.woff2=2000 Example Foundry,2001 Example Contributors:OFL-1.1,MIT;"
    );
}

#[test]
fn required_coverage_reports_every_uncovered_copied_asset() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[licensing]\nrequired = true\n");
    fixture.write("content/assets/first.svg", "<svg/>");
    fixture.write("content/assets/second.pdf", "%PDF");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("coverage must fail");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("missing required licensing declarations"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("assets/first.svg"), "{diagnostic}");
    assert!(diagnostic.contains("assets/second.pdf"), "{diagnostic}");
    assert!(
        diagnostic.contains("content/assets/first.svg"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("content/assets/second.pdf"),
        "{diagnostic}"
    );
    assert!(
        !diagnostic.contains("\nError:\n"),
        "coverage uses one checklist rather than nested error reports: {diagnostic}"
    );
    assert!(
        diagnostic.find("assets/first.svg") < diagnostic.find("assets/second.pdf"),
        "{diagnostic}"
    );
}

#[test]
fn quick_build_checks_new_assets_against_required_coverage() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[licensing]\nrequired = true\n\n[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    build_at(fixture.path(), BuildMode::Full).expect("initial strict build");

    fixture.write("content/assets/new.svg", "<svg/>");
    let error = build_at(fixture.path(), BuildMode::Quick).expect_err("new asset must fail");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("assets/new.svg"), "{diagnostic}");
}

#[test]
fn groups_reject_duplicate_declaration_sources_with_both_locations() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MIT\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("conflict must fail");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("more than one licensing declaration"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("logo.svg.license:1"), "{diagnostic}");
}

#[test]
fn groups_use_published_locale_qualified_output_paths() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n\n[licensing]\nrequired = true\n\n[[licensing.groups]]\nassets = ['assets/shared.svg', 'de/assets/local.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("content/assets/shared.svg", "<svg/>");
    fixture.write("content/de/assets/local.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    build_at(fixture.path(), BuildMode::Full).expect("locale-qualified group builds");
}

#[test]
fn group_spdx_errors_point_to_the_authored_group_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT OR'\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid SPDX must fail");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("invalid licensing group declaration"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:4"), "{diagnostic}");
}

#[test]
fn group_spdx_array_errors_point_to_the_individual_value() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = [\n  'MIT',\n  'MadeUp-License',\n]\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid SPDX must fail");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("heine.toml:6"), "{diagnostic}");
    assert!(!diagnostic.contains("heine.toml:4"), "{diagnostic}");
}

#[test]
fn group_missing_license_text_names_the_project_relative_text_path() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing license text");
    let diagnostic = format!("{error:?}");

    assert!(diagnostic.contains("LICENSES/MIT.txt"), "{diagnostic}");
    assert!(
        !diagnostic.contains(&fixture.path().display().to_string()),
        "group diagnostics must not expose the project filesystem path: {diagnostic}"
    );
}

#[test]
fn licensing_group_configuration_is_strict_and_source_aware() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    fixture.write("heine.toml", "[licensing]\nrequired = false\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("false is redundant");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("required = false is redundant"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/missing.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\nprecedence = 'override'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown group field");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("unknown field `precedence`"),
        "{diagnostic}"
    );

    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/missing.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing asset");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("does not resolve to a copied asset"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
}

#[test]
fn groups_reject_duplicate_unsafe_and_overlapping_members() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = [\n  'assets/logo.svg',\n  'assets/logo.svg',\n]\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("duplicate member");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:4"), "{diagnostic}");
    assert!(diagnostic.contains("more than once"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['../escape.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unsafe member");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("relative"), "{diagnostic}");

    fixture.write(
        "heine.toml",
        "[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n\n[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("overlapping groups");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("heine.toml:7"), "{diagnostic}");
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(
        diagnostic.contains("more than one licensing declaration"),
        "{diagnostic}"
    );
}

#[test]
fn group_order_does_not_change_resolved_attributions() {
    let fixture = Fixture::new();
    fixture.write("content/assets/first.svg", "<svg/>");
    fixture.write("content/assets/second.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("LICENSES/Apache-2.0.txt", "Apache text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% for item in attributions() %}{{ item.asset.output_path }}={{ item.licensing.expressions | join(sep=',') }};{% endfor %}",
    );
    let first = "[[licensing.groups]]\nassets = ['assets/first.svg']\nSPDX-FileCopyrightText = 'First Authors'\nSPDX-License-Identifier = 'MIT'\n\n[[licensing.groups]]\nassets = ['assets/second.svg']\nSPDX-FileCopyrightText = 'Second Authors'\nSPDX-License-Identifier = 'Apache-2.0'\n";
    let second = "[[licensing.groups]]\nassets = ['assets/second.svg']\nSPDX-FileCopyrightText = 'Second Authors'\nSPDX-License-Identifier = 'Apache-2.0'\n\n[[licensing.groups]]\nassets = ['assets/first.svg']\nSPDX-FileCopyrightText = 'First Authors'\nSPDX-License-Identifier = 'MIT'\n";

    fixture.write("heine.toml", first);
    build_at(fixture.path(), BuildMode::Full).expect("first group order builds");
    let first_output =
        fs::read_to_string(fixture.path().join("public/index.html")).expect("first output");

    fixture.write("heine.toml", second);
    build_at(fixture.path(), BuildMode::Full).expect("second group order builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("second output"),
        first_output
    );
}

#[test]
fn required_coverage_isolated_to_the_new_unrelated_asset() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[licensing]\nrequired = true\n\n[[licensing.groups]]\nassets = ['assets/logo.svg']\nSPDX-FileCopyrightText = 'Example Authors'\nSPDX-License-Identifier = 'MIT'\n",
    );
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write("LICENSES/MIT.txt", "MIT text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");
    build_at(fixture.path(), BuildMode::Full).expect("initial strict build");

    fixture.write("content/assets/new.svg", "<svg/>");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("new asset is uncovered");
    let diagnostic = format!("{error:?}");

    assert!(
        diagnostic.contains("content/assets/new.svg"),
        "{diagnostic}"
    );
    assert!(
        !diagnostic.contains("content/assets/logo.svg"),
        "{diagnostic}"
    );
}

#[test]
fn required_coverage_includes_assets_below_a_draft_page() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[licensing]\nrequired = true\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "content/en/draft/index.page",
        "draft = true\ntemplate = 'page.tera'\n",
    );
    fixture.write("content/en/draft/image.svg", "<svg/>");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("draft asset is uncovered");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("content/en/draft/image.svg"),
        "{diagnostic}"
    );
}

#[test]
fn reports_invalid_spdx_expressions_at_the_sidecar_value() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MadeUp-License\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("invalid SPDX expression");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("logo.svg.license"), "{diagnostic}");
    assert!(diagnostic.contains("MadeUp-License"), "{diagnostic}");
    assert!(
        diagnostic.contains("invalid SPDX expression"),
        "{diagnostic}"
    );
}

#[test]
fn rejects_spdx_references_outside_the_local_license_text_model() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: DocumentRef-external:LicenseRef-custom\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("external SPDX document reference fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("logo.svg.license:2"), "{diagnostic}");
    assert!(
        error
            .to_string()
            .contains("external DocumentRef license references"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("use a local LicenseRef"),
        "{diagnostic}"
    );

    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MIT WITH AdditionRef-custom\n",
    );
    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("local addition reference fails");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("logo.svg.license:2"), "{diagnostic}");
    assert!(
        error.to_string().contains("AdditionRef exceptions"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("recognized SPDX exception identifier"),
        "{diagnostic}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_for_the_license_root_and_its_contents() {
    use std::os::unix::fs::symlink;

    let root_link = Fixture::new();
    root_link.write("content/en/index.page", "template = 'page.tera'\n");
    root_link.write("templates/page.tera", "site");
    let external_licenses = root_link.path().join("external-licenses");
    fs::create_dir_all(&external_licenses).expect("external licenses directory");
    symlink(&external_licenses, root_link.path().join("LICENSES")).expect("license root link");

    let error = build_at(root_link.path(), BuildMode::Full).expect_err("license root link");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("LICENSES"), "{diagnostic}");
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");

    let nested_link = Fixture::new();
    nested_link.write("content/en/index.page", "template = 'page.tera'\n");
    nested_link.write("templates/page.tera", "site");
    nested_link.write("LICENSES/MIT.txt", "MIT license text\n");
    let external_text = nested_link.path().join("external.txt");
    fs::write(&external_text, "external license text\n").expect("external license text");
    symlink(
        &external_text,
        nested_link.path().join("LICENSES/Apache-2.0.txt"),
    )
    .expect("license text link");

    let error = build_at(nested_link.path(), BuildMode::Full).expect_err("license text link");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("LICENSES/Apache-2.0.txt"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("symbolic link"), "{diagnostic}");
}

#[test]
fn rejects_sidecars_without_matching_license_text_or_asset() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MIT\n",
    );
    fixture.write(
        "content/assets/orphan.svg.license",
        "SPDX-License-Identifier: MIT\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT license text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("orphan sidecar");
    let diagnostic = format!("{error:?}");
    assert!(error.to_string().contains("asset sibling"), "{diagnostic}");
    assert!(diagnostic.contains("orphan.svg.license:1"), "{diagnostic}");

    fs::remove_file(fixture.path().join("content/assets/orphan.svg.license"))
        .expect("remove orphan");
    fs::remove_file(fixture.path().join("LICENSES/MIT.txt")).expect("remove license text");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing license text");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("LICENSES/MIT.txt"), "{diagnostic}");
    assert!(diagnostic.contains("SPDX identifier"), "{diagnostic}");
}

#[test]
fn requires_license_texts_for_every_compound_spdx_requirement() {
    for operator in ["AND", "OR"] {
        let fixture = Fixture::new();
        fixture.write("content/assets/logo.svg", "<svg/>");
        let sidecar = format!(
            "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: MIT {operator} Apache-2.0\n"
        );
        fixture.write("content/assets/logo.svg.license", &sidecar);
        fixture.write("LICENSES/MIT.txt", "MIT license text\n");
        fixture.write("content/en/index.page", "template = 'page.tera'\n");
        fixture.write("templates/page.tera", "site");

        let error = build_at(fixture.path(), BuildMode::Full)
            .expect_err("every SPDX expression requirement needs a text");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("LICENSES/Apache-2.0.txt"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains(operator), "{diagnostic}");
    }
}

#[test]
fn validates_license_text_layout_and_spdx_exceptions() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Example Authors\nSPDX-License-Identifier: Apache-2.0 WITH LLVM-exception\n",
    );
    fixture.write("LICENSES/Apache-2.0.txt", "Apache license text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "site");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing exception text");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("LLVM-exception.txt"), "{diagnostic}");
    assert!(diagnostic.contains("WITH LLVM-exception"), "{diagnostic}");

    fixture.write("LICENSES/LLVM-exception.txt", "LLVM exception text\n");
    fixture.write("LICENSES/nested/MIT.txt", "not allowed\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("nested license text");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("LICENSES/nested"), "{diagnostic}");
    assert!(diagnostic.contains("direct `.txt` file"), "{diagnostic}");
}

#[test]
fn quick_build_rerenders_attribution_views_after_a_sidecar_change() {
    let fixture = Fixture::new();
    fixture.write("content/assets/logo.svg", "<svg/>");
    let sidecar = fixture.path().join("content/assets/logo.svg.license");
    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: First Author\nSPDX-License-Identifier: MIT\n",
    );
    fixture.write("LICENSES/MIT.txt", "MIT license text\n");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set logo = asset(id='assets/logo.svg') %}{{ logo.licensing.copyrights[0] }}",
    );
    build_at(fixture.path(), BuildMode::Full).expect("initial site builds");

    fixture.write(
        "content/assets/logo.svg.license",
        "SPDX-FileCopyrightText: Second Author\nSPDX-License-Identifier: MIT\n",
    );
    fs::File::options()
        .write(true)
        .open(&sidecar)
        .expect("sidecar source")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set sidecar modification time");
    build_at(fixture.path(), BuildMode::Quick).expect("quick build updates licensing view");

    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("rendered page"),
        "Second Author"
    );
}
