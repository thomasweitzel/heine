use std::path::Path;

use super::*;
use walkdir::WalkDir;

const COMPATIBLE_HEINE: &str = concat!("=", env!("CARGO_PKG_VERSION"));

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn write_page(fixture: &Fixture) {
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "kit selection works");
}

fn write_manifest(fixture: &Fixture, contents: &str) {
    fixture.write("kits/paper/kit.toml", contents);
    fs::create_dir_all(fixture.path().join("kits/paper/templates"))
        .expect("kit templates directory");
    fs::create_dir_all(fixture.path().join("kits/paper/assets")).expect("kit assets directory");
    fs::create_dir_all(fixture.path().join("kits/paper/locales")).expect("kit locales directory");
    fixture.write("kits/paper/site/heine.toml", "[kit]\nname = 'paper'\n");
}

fn manifest(version: &str, heine: &str) -> String {
    format!("name = 'paper'\nversion = '{version}'\nheine = '{heine}'\nmessage_prefix = 'paper'\n")
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in WalkDir::new(source).sort_by_file_name() {
        let entry = entry.expect("reference-site tree entry");
        let relative = entry
            .path()
            .strip_prefix(source)
            .expect("tree entry remains below copied root");
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target).expect("reference-site directory");
        } else {
            fs::create_dir_all(target.parent().expect("reference-site file has parent"))
                .expect("reference-site parent directory");
            fs::copy(entry.path(), target).expect("reference-site file");
        }
    }
}

#[test]
fn resolves_a_selected_compatible_kit_before_loading_site_resources() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\nversion = '^1.2'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    write_page(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("compatible kit builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "kit selection works"
    );
}

#[test]
fn materialized_kit_reference_site_selects_and_builds_with_its_kit() {
    let package = Fixture::new();
    write_manifest(
        &package,
        &format!(
            "{}\n[public]\ntemplates = ['base.tera']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    package.write(
        "kits/paper/templates/base.tera",
        "<main>{% block content %}{% endblock %}</main>",
    );
    package.write(
        "kits/paper/site/heine.toml",
        "[kit]\nname = 'paper'\n\n[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n",
    );
    package.write(
        "kits/paper/site/content/en/index.page",
        "template = 'page.tera'\n",
    );
    package.write(
        "kits/paper/site/templates/page.tera",
        "{% extends 'kit/paper/base.tera' %}{% block content %}reference site{% endblock %}",
    );

    let consumer = Fixture::new();
    copy_tree(
        &package.path().join("kits/paper"),
        &consumer.path().join("kits/paper"),
    );
    copy_tree(&package.path().join("kits/paper/site"), consumer.path());

    build_at(consumer.path(), BuildMode::Full).expect("materialized reference site builds");
    assert_eq!(
        fs::read_to_string(consumer.path().join("public/index.html")).expect("page output"),
        "<main>reference site</main>"
    );
}

#[test]
fn ignores_reference_site_inputs_during_an_ordinary_site_build() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write(
        "kits/paper/site/content/en/unused.page",
        "not valid TOML = [",
    );
    write_page(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("reference site is not a runtime input");
}

#[test]
fn requires_a_non_symlinked_reference_site_that_selects_its_containing_kit() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    write_page(&fixture);

    fs::remove_dir_all(fixture.path().join("kits/paper/site"))
        .expect("remove reference site from fixture");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("reference site is required");
    assert!(
        error.to_string().contains("kit reference site") && error.to_string().contains("missing"),
        "{error:?}"
    );

    fixture.write("kits/paper/site/heine.toml", "[kit]\nname = 'other'\n");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("reference site must select its containing kit");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("kit reference site selects \"other\"")
    );
    assert!(
        diagnostic.contains("kits/paper/site/heine.toml:2"),
        "{diagnostic}"
    );
}

#[test]
fn reports_a_missing_selected_kit_manifest() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    fs::create_dir_all(fixture.path().join("kits/paper")).expect("kit directory");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("kit manifest is required");
    assert!(
        error.to_string().contains("kit manifest") && error.to_string().contains("is missing"),
        "{error:?}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_at_the_selected_kit_package_boundary() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    write_page(&fixture);

    let package = fixture.path().join("kits/paper");
    let relocated = fixture.path().join("kits/paper-package");
    fs::rename(&package, &relocated).expect("relocate kit package");
    symlink("paper-package", &package).expect("symlink kit package");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("selected kit package cannot be a symbolic link");
    assert!(
        error.to_string().contains("selected kit directory")
            && error.to_string().contains("symbolic link"),
        "{error:?}"
    );

    fs::remove_file(&package).expect("remove kit package symlink");
    fs::rename(&relocated, &package).expect("restore kit package");
    let manifest = package.join("kit.toml");
    let relocated_manifest = package.join("kit.manifest.toml");
    fs::rename(&manifest, &relocated_manifest).expect("relocate manifest");
    symlink("kit.manifest.toml", &manifest).expect("symlink manifest");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("kit manifest cannot be a symbolic link");
    assert!(
        error.to_string().contains("kit manifest") && error.to_string().contains("symbolic link"),
        "{error:?}"
    );
}

#[test]
fn reports_name_and_version_mismatches_at_the_authored_declarations() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\nversion = '^2'\n");
    write_manifest(
        &fixture,
        &manifest("1.2.3", COMPATIBLE_HEINE).replace("paper", "other"),
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("name mismatch is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error.to_string().contains("does not match manifest kit"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:2"), "{diagnostic}");
    assert!(diagnostic.contains("kits/paper/kit.toml:1"), "{diagnostic}");

    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("version mismatch is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("does not satisfy site requirement"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("heine.toml:3"), "{diagnostic}");
}

#[test]
fn reports_an_incompatible_heine_requirement_at_the_manifest() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", ">=0.8.0, <0.9.0"));
    fixture.write(
        "kits/paper/templates/broken.tera",
        "{% this template would fail if Heine loaded it %}",
    );

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("Heine compatibility is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("does not satisfy kit requirement"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("kits/paper/kit.toml:3"), "{diagnostic}");
}

#[test]
fn root_adapters_extend_declared_public_kit_templates_by_namespace() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ntemplates = ['base.tera']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write(
        "kits/paper/templates/base.tera",
        "<main>{% block content %}{% endblock %}</main>",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% extends 'kit/paper/base.tera' %}{% block content %}root adapter{% endblock %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("public kit template is available");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "<main>root adapter</main>"
    );
}

#[test]
fn rejects_missing_public_kit_templates_at_the_manifest_declaration() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ntemplates = ['missing.tera']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    write_page(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("public template is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error.to_string().contains("declared public kit template"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("kits/paper/kit.toml:7"), "{diagnostic}");
}

#[test]
fn keeps_private_kit_templates_and_root_templates_in_separate_namespaces() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write("kits/paper/templates/private.tera", "private");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% include 'kit/paper/private.tera' %}",
    );

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("private template is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error
            .to_string()
            .contains("is private and cannot be referenced"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("templates/page.tera:1"), "{diagnostic}");

    fixture.write(
        "templates/page.tera",
        "{% include `kit/paper/private.tera` %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("raw string references cannot reach a private kit template");
    assert!(
        error
            .to_string()
            .contains("is private and cannot be referenced"),
        "{error:?}"
    );

    fixture.write(
        "kits/paper/templates/private.tera",
        "{% include `page.tera` %}",
    );
    fixture.write("templates/page.tera", "root");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("kit templates cannot reference root templates");
    assert!(
        error
            .to_string()
            .contains("may reference only templates in its own kit namespace"),
        "{error:?}"
    );
}

#[test]
fn reserves_the_kit_template_namespace_even_without_a_selected_kit() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'kit/page.tera'\n");
    fixture.write("templates/kit/page.tera", "root");

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("kit namespace is reserved");
    assert!(
        error.to_string().contains("reserved kit namespace"),
        "{error:?}"
    );
}

#[test]
fn root_template_settings_cannot_select_a_kit_original_directly() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ntemplates = ['base.tera']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/templates/base.tera", "kit original");
    fixture.write(
        "content/en/index.page",
        "template = 'kit/paper/base.tera'\n",
    );

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("page settings must select root templates");
    assert!(error.to_string().contains("was not found"), "{error:?}");
}

#[test]
fn publishes_public_kit_assets_through_the_ordinary_asset_namespace() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\nassets = ['assets/css/paper.css']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/assets/css/paper.css", "kit stylesheet");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set stylesheet = asset(id='assets/css/paper.css') %}{{ stylesheet.url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("public kit asset builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "/assets/css/paper.css"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/assets/css/paper.css"))
            .expect("kit asset output"),
        "kit stylesheet"
    );
}

#[test]
fn resolves_a_public_kit_asset_from_every_configured_locale() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\nassets = ['assets/css/paper.css']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/assets/css/paper.css", "kit stylesheet");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set stylesheet = asset(id='assets/css/paper.css') %}{{ stylesheet.url }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("public kit asset builds in every locale");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("English page"),
        "/assets/css/paper.css"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German page"),
        "/assets/css/paper.css"
    );
}

#[test]
fn a_root_shared_asset_overlays_a_public_kit_asset() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\nassets = ['assets/css/paper.css']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/assets/css/paper.css", "kit stylesheet");
    fixture.write("content/assets/css/paper.css", "root stylesheet");
    write_page(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("root asset overlay builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/assets/css/paper.css"))
            .expect("overlaid asset output"),
        "root stylesheet"
    );
}

#[test]
fn quick_builds_rerender_pages_after_a_selected_kit_asset_changes() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[assets]\nintegrity = 'sha256'\n",
    );
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\nassets = ['assets/script.js']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/assets/script.js", "before");
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% set script = asset(id='assets/script.js') %}{{ script.integrity }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("initial kit build");
    let before = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    fixture.write("kits/paper/assets/script.js", "after");
    fs::File::options()
        .write(true)
        .open(fixture.path().join("kits/paper/assets/script.js"))
        .expect("kit asset")
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(1)))
        .expect("set kit asset modification time");

    build_at(fixture.path(), BuildMode::Quick).expect("quick kit build");
    let after = fs::read_to_string(fixture.path().join("public/index.html")).expect("output");
    assert_ne!(after, before);
}

#[test]
fn validates_public_kit_assets_at_the_manifest_declaration() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\nassets = ['assets/css/missing.css']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    write_page(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full).expect_err("public asset is checked");
    let diagnostic = format!("{error:?}");
    assert!(
        error.to_string().contains("declared asset")
            && error.to_string().contains("does not exist"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("kits/paper/kit.toml:7"), "{diagnostic}");
}

#[test]
fn applies_required_licensing_and_root_license_overlays_to_kit_assets() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[licensing]\nrequired = true\n",
    );
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write("kits/paper/assets/images/mark.svg", "kit mark");
    fixture.write(
        "kits/paper/assets/images/mark.svg.license",
        "SPDX-FileCopyrightText: Paper Authors\nSPDX-License-Identifier: MIT\n",
    );
    fixture.write("kits/paper/LICENSES/MIT.txt", "kit MIT text\n");
    fixture.write("LICENSES/MIT.txt", "root MIT text\n");
    write_page(&fixture);

    build_at(fixture.path(), BuildMode::Full).expect("licensed kit asset builds");
    assert_eq!(
        fs::read_to_string(
            fixture
                .path()
                .join("public/kit/paper/assets/images/mark.svg")
        )
        .expect("private kit asset output"),
        "kit mark"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/LICENSES/MIT.txt"))
            .expect("root license text overlay"),
        "root MIT text\n"
    );
}

#[test]
fn kit_templates_can_resolve_private_assets_but_root_templates_cannot() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ntemplates = ['base.tera']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("kits/paper/assets/images/mark.svg", "kit mark");
    fixture.write(
        "kits/paper/templates/base.tera",
        "{% block content %}{% set mark = asset(id='kit/paper/assets/images/mark.svg') %}{{ mark.url }}{% endblock %}",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{% extends 'kit/paper/base.tera' %}{% block content %}{{ super() }}{% endblock %}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("kit-private asset builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "/kit/paper/assets/images/mark.svg"
    );

    fixture.write(
        "templates/page.tera",
        "{% set mark = asset(id='kit/paper/assets/images/mark.svg') %}{{ mark.url }}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root templates cannot resolve private kit assets");
    assert!(
        error.to_string().contains("private to its kit"),
        "{error:?}"
    );
}

#[test]
fn reserves_the_internal_kit_asset_function_from_root_templates() {
    let fixture = Fixture::new();
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ __a__(id='assets/logo.svg') }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root source cannot access the internal kit asset function");
    assert!(
        error
            .to_string()
            .contains("reserved for kit-private asset resolution"),
        "{error:?}"
    );
}

#[test]
fn root_markdown_cannot_resolve_private_kit_assets() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write("kits/paper/assets/images/mark.svg", "kit mark");
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\ncontent.main = 'index.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "[private mark](asset:kit/paper/assets/images/mark.svg)",
    );
    fixture.write("templates/page.tera", "{{ page.content.main | markdown }}");

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root Markdown cannot resolve a private kit asset");
    assert!(
        format!("{error:?}").contains("private to its kit"),
        "{error:?}",
    );
}

#[test]
fn root_messages_override_kit_messages_with_default_locale_term_fallback() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[i18n]\ndefault_locale = 'en'\n[i18n.locales.en]\ndirection = 'ltr'\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write(
        "kits/paper/locales/en/kit.ftl",
        "paper-label = Kit label\n-paper-brand = Paper\n",
    );
    fixture.write(
        "locales/de/site.ftl",
        "paper-label = Deutsche { -paper-brand }\n",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write("content/de/index.page", "template = 'page.tera'\n");
    fixture.write("templates/page.tera", "{{ t(id='paper-label') }}");

    build_at(fixture.path(), BuildMode::Full).expect("kit message layering builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("English page"),
        "Kit label"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German page"),
        "Deutsche Paper"
    );
}

#[test]
fn checks_kit_message_prefixes_and_root_override_names() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(&fixture, &manifest("1.2.3", COMPATIBLE_HEINE));
    fixture.write("kits/paper/locales/en/kit.ftl", "label = Missing prefix\n");
    write_page(&fixture);

    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("kit messages require the manifest prefix");
    assert!(error.to_string().contains("must begin with"), "{error:?}");

    fixture.write("kits/paper/locales/en/kit.ftl", "paper-label = Kit label\n");
    fixture.write("locales/en/site.ftl", "paper-missing = Root text\n");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root prefixed messages must override kit declarations");
    assert!(
        error
            .to_string()
            .contains("does not override a kit declaration"),
        "{error:?}"
    );
}

#[test]
fn exposes_only_declared_public_kit_components_to_root_templates() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ncomponents = ['paper.card']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write(
        "kits/paper/templates/components.tera",
        "{% component paper.card(label) %}<aside>{{ label }}</aside>{% endcomponent %}\n{% component paper.private() %}private{% endcomponent %}",
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{{ <paper.card label=\"Public component\" /> }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("root can call a public kit component");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "<aside>Public component</aside>"
    );

    fixture.write("templates/page.tera", "{{ <paper.private /> }}");
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root cannot call a private kit component");
    assert!(error.to_string().contains("is private"), "{error:?}");
}

#[test]
fn checks_component_namespaces_and_public_declarations() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[kit]\nname = 'paper'\n");
    write_manifest(
        &fixture,
        &format!(
            "{}\n[public]\ncomponents = ['paper.card']\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write(
        "kits/paper/templates/components.tera",
        "{% component other.card() %}wrong namespace{% endcomponent %}",
    );
    write_page(&fixture);

    let error =
        build_at(fixture.path(), BuildMode::Full).expect_err("kit component namespace is checked");
    assert!(
        error
            .to_string()
            .contains("must use the kit component namespace")
    );

    fixture.write(
        "kits/paper/templates/components.tera",
        "{% component paper.private() %}private{% endcomponent %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("public components must be defined by the kit");
    let diagnostic = format!("{error:?}");
    assert!(
        error.to_string().contains("declared public kit component"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("kits/paper/kit.toml:7"), "{diagnostic}");

    fixture.write(
        "kits/paper/templates/components.tera",
        "{% component paper.card() %}public{% endcomponent %}",
    );
    fixture.write(
        "templates/page.tera",
        "{% component paper.replacement() %}wrong{% endcomponent %}",
    );
    let error = build_at(fixture.path(), BuildMode::Full)
        .expect_err("root cannot define a component in the kit namespace");
    assert!(
        error
            .to_string()
            .contains("occupies the selected kit component namespace"),
        "{error:?}"
    );
}

#[test]
fn merges_checked_kit_extra_defaults_and_root_overrides() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[kit.extra]\nshow_search = false\nitems = []\n\n[kit.extra.navigation]\ncompact = true\n",
    );
    write_manifest(
        &fixture,
        &format!(
            "{}\n[extra]\nshow_search = true\nitems = ['start']\n\n[extra.navigation]\ncompact = false\naccent = 'violet'\n",
            manifest("1.2.3", COMPATIBLE_HEINE)
        ),
    );
    fixture.write("content/en/index.page", "template = 'page.tera'\n");
    fixture.write(
        "templates/page.tera",
        "{{ kit.extra.show_search }}|{{ kit.extra.items | length }}|{{ kit.extra.navigation.compact }}|{{ kit.extra.navigation.accent }}",
    );

    build_at(fixture.path(), BuildMode::Full).expect("compatible kit settings build");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("page output"),
        "false|0|true|violet"
    );

    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[kit.extra]\nunknown = true\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown setting is checked");
    assert!(
        error
            .to_string()
            .contains("kit.extra.unknown is not declared"),
        "{error:?}"
    );

    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[kit.extra]\nshow_search = 'no'\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("type change is checked");
    assert!(
        error.to_string().contains("must remain a boolean"),
        "{error:?}"
    );

    fixture.write(
        "heine.toml",
        "[kit]\nname = 'paper'\n\n[kit.extra]\nitems = [1]\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("array members are checked");
    assert!(
        error.to_string().contains("array entries must be string"),
        "{error:?}"
    );
}
