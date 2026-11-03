//! Citation page-plan integration cases used in the processor investigation.

use super::*;

#[test]
fn apa_distinct_authors_can_render_ambiguous_citations_in_page_plan() {
    use crate::markdown::{self, citation::Collector};
    use hayagriva::archive::ArchivedStyle;

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n").unwrap();
    let config = crate::config::SiteConfig::load(
        &dir.path().join("heine.toml"),
        &crate::source_diagnostic::DiagnosticPaths::new(dir.path()),
    )
    .unwrap();
    fs::write(dir.path().join("a.bib"), "@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    fs::write(
        dir.path().join("style.csl"),
        ArchivedStyle::AmericanPsychologicalAssociation
            .get()
            .to_xml()
            .unwrap(),
    )
    .unwrap();
    let source = "First [@a]. Second [@c].";
    let setting = settings("content='main'\nbibliography=['a.bib']\nstyle='style.csl'\n");
    let content = BTreeMap::from([("main".to_owned(), serde_json::json!(source))]);
    let names = BTreeSet::from(["main".to_owned()]);
    let mut plan = InputCache::default()
        .plan(
            &setting,
            dir.path(),
            &content,
            &names,
            (Path::new("page"), ""),
        )
        .unwrap();
    let mut collector = Collector::default();
    markdown::observe_citations(source, config.markdown(), &mut collector).unwrap();
    plan.groups = collector.groups;
    plan.format(source, "en").unwrap();
    let view = plan.into_view().unwrap();
    assert_eq!(view.replacements.len(), 2);
    assert!(
        view.replacements
            .iter()
            .all(|item| item.html.contains("(Smith, 2020)"))
    );
    for title in ["Alpha", "Gamma"] {
        assert!(
            view.bibliography
                .contains(&format!("Smith. (2020). <em>{title}</em>")),
            "{}",
            view.bibliography
        );
    }
}

#[test]
fn chicago_ambiguous_citations_remain_ambiguous_in_both_footnote_modes() {
    use crate::markdown::{self, citation::Collector, margin_footnotes};
    use hayagriva::archive::ArchivedStyle;

    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n[markdown]\nextensions=['footnotes']\n").unwrap();
    let config = crate::config::SiteConfig::load(
        &dir.path().join("heine.toml"),
        &crate::source_diagnostic::DiagnosticPaths::new(dir.path()),
    )
    .unwrap();
    let renderer = markdown::Renderer::new(config.markdown().clone()).unwrap();
    fs::write(dir.path().join("a.bib"), "@book{a,author={Smith, Alice},title={Alpha},year={2020}}\n@book{b,author={Smith, Alice},title={Beta},year={2020}}\n@book{c,author={Smith, Bob},title={Gamma},year={2020}}\n").unwrap();
    fs::write(
        dir.path().join("style.csl"),
        ArchivedStyle::ChicagoAuthorDate.get().to_xml().unwrap(),
    )
    .unwrap();
    let source = "First [@a, p. 12].[^n] Then [@b, p. 13]. Again[^n]. Finally [@c].\n\n[^n]: Note [@a, p. 14].\n";
    let setting = settings("content='main'\nbibliography=['a.bib']\nstyle='style.csl'\n");
    let content = BTreeMap::from([("main".to_owned(), serde_json::json!(source))]);
    let names = BTreeSet::from(["main".to_owned()]);
    let mut plan = InputCache::default()
        .plan(
            &setting,
            dir.path(),
            &content,
            &names,
            (Path::new("page"), ""),
        )
        .unwrap();
    let mut collector = Collector::default();
    markdown::observe_citations(source, config.markdown(), &mut collector).unwrap();
    plan.groups = collector.groups;
    let Style::Independent(selected_style) = &plan.inputs.style else {
        panic!("independent style");
    };
    assert!(matches!(
        style::check(selected_style),
        Err(style::Error::Position(_))
    ));
    // Exercise the proposed relaxation without changing the production guard.
    assert!(
        plan.inputs
            .positions
            .set(Ok(style::StyleFacts {
                strips_locator_periods: false
            }))
            .is_ok()
    );
    plan.format(source, "en").unwrap();
    let view = plan.into_view().unwrap();
    let ordinary = markdown::render::render_with_citations(
        source,
        &renderer,
        None,
        None,
        None,
        None,
        &[],
        &view.replacements,
    )
    .unwrap();
    let margin_plan = margin_footnotes::plan(
        Path::new("main.md"),
        "main".into(),
        (source, source),
        config.markdown(),
        "note".into(),
    )
    .unwrap();
    let margin = margin_footnotes::render(
        source,
        margin_footnotes::RenderContext {
            renderer: &renderer,
            assets: None,
            pages: None,
            anchors: None,
            table_scroll_label: None,
            replacements: &[],
            citations: &view.replacements,
        },
        &margin_plan,
        &["Footnote 1".into()],
    )
    .unwrap();
    for (rendered, note_copies) in [(&ordinary, 1), (&margin, 2)] {
        for citation in ["(Smith 2020, 12)", "(Smith 2020, 13)", "(Smith 2020)"] {
            assert_eq!(rendered.matches(citation).count(), 1, "{rendered}");
        }
        assert_eq!(
            rendered.matches("(Smith 2020, 14)").count(),
            note_copies,
            "{rendered}"
        );
    }
    for title in ["Alpha", "Beta", "Gamma"] {
        assert!(
            view.bibliography
                .contains(&format!("Smith. 2020. <em>{title}</em>")),
            "{}",
            view.bibliography
        );
    }
}

#[test]
fn ibid_page_plan_reuses_first_reference_order_in_both_footnote_modes() {
    use crate::markdown::{self, citation::Collector, margin_footnotes};
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("heine.toml"), "[site]\ntime_zone='Europe/Berlin'\n[i18n]\ndefault_locale='en'\n[i18n.locales.en]\ndirection='ltr'\n[markdown]\nextensions=['footnotes']\n").unwrap();
    let config = crate::config::SiteConfig::load(
        &dir.path().join("heine.toml"),
        &crate::source_diagnostic::DiagnosticPaths::new(dir.path()),
    )
    .unwrap();
    let renderer = markdown::Renderer::new(config.markdown().clone()).unwrap();
    fs::write(dir.path().join("a.bib"), "@book{a,title={Alpha}}\n").unwrap();
    fs::write(dir.path().join("style.csl"), r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0" class="in-text"><info><title>Position</title><id>https://example.org/position</id></info><citation><layout><choose><if position="ibid-with-locator"><text value="CHANGED"/></if><else-if position="ibid"><text value="SAME"/></else-if><else><text value="FIRST"/></else></choose></layout></citation><bibliography><layout><text variable="title"/></layout></bibliography></style>"#).unwrap();
    let source = "First [@a].[^n] Then [@a, p. 13]. Again[^n].\n\n[^n]: Note [@a].\n";
    let setting = settings("content='main'\nbibliography=['a.bib']\nstyle='style.csl'\n");
    let content = BTreeMap::from([("main".to_owned(), serde_json::json!(source))]);
    let names = BTreeSet::from(["main".to_owned()]);
    let mut plan = InputCache::default()
        .plan(
            &setting,
            dir.path(),
            &content,
            &names,
            (Path::new("page"), ""),
        )
        .unwrap();
    let mut collector = Collector::default();
    markdown::observe_citations(source, config.markdown(), &mut collector).unwrap();
    plan.groups = collector.groups;
    let Style::Independent(selected_style) = &plan.inputs.style else {
        panic!("independent style");
    };
    assert!(matches!(
        style::check(selected_style),
        Err(style::Error::Position("ibid-with-locator"))
    ));
    // The production guard remains active. This test supplies the checked
    // facts directly to exercise the later plan and render boundaries.
    assert!(
        plan.inputs
            .positions
            .set(Ok(style::StyleFacts {
                strips_locator_periods: false
            }))
            .is_ok()
    );
    plan.format(source, "en").unwrap();
    let view = plan.into_view().unwrap();
    let ordinary = markdown::render::render_with_citations(
        source,
        &renderer,
        None,
        None,
        None,
        None,
        &[],
        &view.replacements,
    )
    .unwrap();
    let margin_plan = margin_footnotes::plan(
        Path::new("main.md"),
        "main".into(),
        (source, source),
        config.markdown(),
        "note".into(),
    )
    .unwrap();
    let margin = margin_footnotes::render(
        source,
        margin_footnotes::RenderContext {
            renderer: &renderer,
            assets: None,
            pages: None,
            anchors: None,
            table_scroll_label: None,
            replacements: &[],
            citations: &view.replacements,
        },
        &margin_plan,
        &["Footnote 1".into()],
    )
    .unwrap();
    assert_eq!(ordinary.matches(">FIRST</a>").count(), 1, "{ordinary}");
    assert_eq!(ordinary.matches(">SAME</a>").count(), 1, "{ordinary}");
    assert_eq!(ordinary.matches(">CHANGED</a>").count(), 1, "{ordinary}");
    assert_eq!(margin.matches(">FIRST</a>").count(), 1, "{margin}");
    assert_eq!(margin.matches(">SAME</a>").count(), 2, "{margin}");
    assert_eq!(margin.matches(">CHANGED</a>").count(), 1, "{margin}");
}
