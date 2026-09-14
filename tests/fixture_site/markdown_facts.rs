use super::*;

#[test]
fn exposes_per_input_unicode_word_counts_and_opt_in_reading_time() {
    let fixture = Fixture::new();
    fixture.write(
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[markdown]\nmath = 'mathml'\nextensions = ['footnotes']\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[reading_time.words_per_minute]\nen = 4\nde = 3\n",
    );
    fixture.write(
        "templates/page.tera",
        "{{ page.markdown.main.word_count }}|{{ page.markdown.main.reading_time }}|{{ page.markdown.picture.word_count }}|{{ page.markdown.picture.reading_time }}|{{ page.markdown.segmented.word_count }}|{{ page.markdown.segmented.reading_time }}",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[content]\nmain = 'index.md'\npicture = 'picture.md'\nsegmented = 'segmented.md'\n",
    );
    fixture.write(
        "content/en/index.md",
        "One two `three` four.[^note] Again.[^note]\n\n[^note]: Footnote body.\n\n<div>HTML words <script>hidden words</script><style>also hidden</style><template>template hidden</template></div>\n\n```rust\nlet excluded = true;\n```\n\n$also_excluded$\n",
    );
    fixture.write(
        "content/en/picture.md",
        ":::picture\nalt = 'Light and dark portrait'\nlight = 'asset:assets/light.svg'\ndark = 'asset:assets/dark.svg'\ncaption = 'A portrait in two colors'\n:::\n",
    );
    fixture.write(
        "content/assets/light.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
    );
    fixture.write(
        "content/assets/dark.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
    );
    fixture.write(
        "content/en/segmented.md",
        "<!-- segment: lead -->\nFirst segment words.\n<!-- segment: body -->\nSecond segment words.\n",
    );
    fixture.write(
        "content/de/index.page",
        "template = 'de.tera'\n[content]\nmain = 'index.md'\n",
    );
    fixture.write(
        "templates/de.tera",
        "{{ page.markdown.main.word_count }}|{{ page.markdown.main.reading_time }}",
    );
    fixture.write("content/de/index.md", "Eins zwei drei vier fünf sechs.\n");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("English output"),
        "9|3|9|3|6|2"
    );
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/de/index.html")).expect("German output"),
        "6|2"
    );
}

#[test]
fn reading_time_configuration_requires_exact_positive_locale_rates() {
    let fixture = Fixture::new();
    fixture.write("heine.toml", "[reading_time.words_per_minute]\nen = 0\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("zero rate is rejected");
    assert!(
        format!("{error:?}").contains("values must be positive integers"),
        "{error:?}"
    );

    fixture.write(
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[reading_time.words_per_minute]\nen = 200\n",
    );
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("missing locale is rejected");
    assert!(
        format!("{error:?}").contains("must define a positive rate for configured locale de"),
        "{error:?}"
    );

    fixture.write("heine.toml", "[reading_time.words_per_minute]\nfr = 200\n");
    let error = build_at(fixture.path(), BuildMode::Full).expect_err("unknown locale is rejected");
    assert!(
        format!("{error:?}").contains("which is not configured"),
        "{error:?}"
    );
}

#[test]
fn absent_reading_time_policy_exposes_a_null_fact() {
    let fixture = Fixture::new();
    fixture.write(
        "templates/page.tera",
        "{{ page.markdown.main.word_count }}|{% if page.markdown.main.reading_time %}present{% else %}none{% endif %}",
    );
    fixture.write(
        "content/en/index.page",
        "template = 'page.tera'\n[content]\nmain = 'index.md'\n",
    );
    fixture.write("content/en/index.md", "Only prose.\n");

    build_at(fixture.path(), BuildMode::Full).expect("site builds");
    assert_eq!(
        fs::read_to_string(fixture.path().join("public/index.html")).expect("output"),
        "2|none"
    );
}
