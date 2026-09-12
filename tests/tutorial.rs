//! The tutorial is an executable, step-by-step documentation contract.

use std::{fs, path::Path};

use heine::{BuildMode, build_at};

#[path = "support/temp_dir.rs"]
mod temp_dir;

use temp_dir::TestDirectory;

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("tutorial file has a parent"))
        .expect("tutorial parent directory");
    fs::write(path, contents).expect("tutorial file");
}

fn build_step(root: &Path, step: u8) {
    build_at(root, BuildMode::Full).unwrap_or_else(|error| {
        panic!("tutorial step {step} must build:\n{error:?}");
    });
}

fn output(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join("public").join(relative)).expect("tutorial output")
}

#[test]
fn every_authored_tutorial_step_builds_the_evolving_site() {
    let temporary = TestDirectory::new();
    let root = temporary.path();

    // 1. Start with one page.
    write(
        root,
        "heine.toml",
        "[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n",
    );
    write(
        root,
        "content/en/index.page",
        "template = 'page.tera'\ntitle = 'My site'\n",
    );
    write(
        root,
        "templates/page.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{{ page.title }}</title></head><body><h1>{{ page.title }}</h1></body></html>",
    );
    build_step(root, 1);
    assert!(output(root, "index.html").contains("<h1>My site</h1>"));

    // 2. Add a shared template and Markdown content.
    write(
        root,
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n",
    );
    write(
        root,
        "templates/base.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{% block title %}{{ page.title }}{% endblock %}</title></head><body>{% set home = page(id='index') %}<header><a href=\"{{ home.url }}\">Home</a></header><main>{% block content %}{% endblock %}</main></body></html>",
    );
    write(
        root,
        "templates/post.tera",
        "{% extends 'base.tera' %}{% block content %}<article><h1>{{ page.title }}</h1>{{ page.content.main | markdown }}</article>{% endblock %}",
    );
    write(
        root,
        "content/en/blog/first.page",
        "template = 'post.tera'\ntitle = 'First post'\nsummary = 'A short first post.'\npublished = 2026-08-14T09:30:00\n\n[content]\nmain = 'first.md'\n",
    );
    write(root, "content/en/blog/first.md", "Hello from **Heine**.\n");
    write(
        root,
        "content/en/index.page",
        "template = 'post.tera'\ntitle = 'My site'\n\n[content]\nmain = 'index.md'\n",
    );
    write(
        root,
        "content/en/index.md",
        "Welcome to my site. [Read the first post](page:blog/first).\n",
    );
    build_step(root, 2);
    let home = output(root, "index.html");
    assert!(home.contains("Welcome to my site."), "{home}");
    assert!(home.contains("href=\"/blog/first.html\""), "{home}");

    // 3. Add a copied asset.
    write(
        root,
        "content/assets/css/site.css",
        "body { color: #222; background: #f3f7fb; }\n",
    );
    write(
        root,
        "templates/base.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{% block title %}{{ page.title }}{% endblock %}</title>{% set site_css = asset(id='assets/css/site.css') %}<link rel=\"stylesheet\" href=\"{{ site_css.url }}\"></head><body>{% set home = page(id='index') %}<header><a href=\"{{ home.url }}\">Home</a></header><main>{% block content %}{% endblock %}</main></body></html>",
    );
    build_step(root, 3);
    assert!(root.join("public/assets/css/site.css").is_file());
    assert!(output(root, "index.html").contains("assets/css/site.css"));

    // 4. Add shared structured data.
    write(
        root,
        "data/site.toml",
        "name = 'My site'\n\n[[navigation]]\nlabel = 'Start'\npage = 'index'\n",
    );
    write(
        root,
        "templates/base.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{% block title %}{{ page.title }}{% endblock %}</title>{% set site_css = asset(id='assets/css/site.css') %}<link rel=\"stylesheet\" href=\"{{ site_css.url }}\"></head><body><header><nav aria-label=\"Main navigation\">{% for item in data.site.navigation %}{% set target = page(id=item.page) %}<a href=\"{{ target.url }}\">{{ item.label }}</a>{% endfor %}</nav></header><main>{% block content %}{% endblock %}</main></body></html>",
    );
    build_step(root, 4);
    assert!(output(root, "index.html").contains("Main navigation"));
    assert!(output(root, "index.html").contains(">Start</a>"));

    // 5. Add another language and localized UI text.
    write(
        root,
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n",
    );
    write(
        root,
        "content/en/index.page",
        "template = 'post.tera'\ntitle = 'My site'\ntranslationid = 'home'\n\n[content]\nmain = 'index.md'\n",
    );
    write(
        root,
        "content/de/index.page",
        "template = 'post.tera'\ntitle = 'Meine Seite'\ntranslationid = 'home'\n\n[content]\nmain = 'index.md'\n",
    );
    write(
        root,
        "content/de/index.md",
        "Willkommen auf meiner Seite.\n",
    );
    write(
        root,
        "locales/en/site.ftl",
        "nav-home = Home\nnav-main = Main navigation\ntable-scroll = Scrollable table\n",
    );
    write(
        root,
        "locales/de/site.ftl",
        "nav-home = Startseite\nnav-main = Hauptnavigation\ntable-scroll = Scrollbare Tabelle\n",
    );
    write(
        root,
        "templates/base.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{% block title %}{{ page.title }}{% endblock %}</title>{% set site_css = asset(id='assets/css/site.css') %}<link rel=\"stylesheet\" href=\"{{ site_css.url }}\"></head><body><header>{% set home = page(id='index') %}<nav aria-label=\"{{ t(id='nav-main') }}\"><a href=\"{{ home.url }}\">{{ t(id='nav-home') }}</a></nav></header><main>{% block content %}{% endblock %}</main></body></html>",
    );
    build_step(root, 5);
    assert!(output(root, "de/index.html").contains("Hauptnavigation"));

    // 6. Create an ordered post collection.
    write(
        root,
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'published-desc'\n",
    );
    write(
        root,
        "content/en/blog/_directory.toml",
        "[pages]\ntemplate = 'post.tera'\ncollections = ['posts']\n",
    );
    write(
        root,
        "content/en/blog/first.page",
        "title = 'First post'\nsummary = 'A short first post.'\npublished = 2026-08-14T09:30:00\n\n[content]\nmain = 'first.md'\n",
    );
    write(
        root,
        "content/en/blog/index.page",
        "template = 'blog-index.tera'\ncollections = []\n",
    );
    write(
        root,
        "templates/blog-index.tera",
        "{% extends 'base.tera' %}{% block content %}{% set posts = collection(name='posts') %}{% for id in posts.page_ids %}{% set post = page(id=id) %}<article><h2><a href=\"{{ post.url }}\">{{ post.title }}</a></h2></article>{% endfor %}{% endblock %}",
    );
    build_step(root, 6);
    assert!(output(root, "blog/index.html").contains("First post"));

    // 7. Paginate the blog index.
    write(
        root,
        "content/en/blog/second.page",
        "title = 'Second post'\nsummary = 'A short second post.'\npublished = 2026-08-15T09:30:00\n\n[content]\nmain = 'second.md'\n",
    );
    write(
        root,
        "content/en/blog/second.md",
        "Another post for the paginated listing.\n",
    );
    write(
        root,
        "content/en/blog/index.page",
        "template = 'blog-index.tera'\ncollections = []\n\n[pagination]\ncollection = 'posts'\nper_page = 1\n\n[pagination.navigation]\nwindow = 1\n",
    );
    write(
        root,
        "templates/blog-index.tera",
        "{% extends 'base.tera' %}{% block content %}{% for post in pagination.pages %}<a href=\"{{ post.url }}\">{{ post.title }}</a>{% endfor %}<nav aria-label=\"Pagination\">{% for item in pagination.navigation %}{% if item.kind == 'gap' %}<span aria-hidden=\"true\">…</span>{% elif item.current %}<span aria-current=\"page\">{{ item.page_number }}</span>{% else %}<a href=\"{{ item.url }}\">{{ item.page_number }}</a>{% endif %}{% endfor %}</nav>{% endblock %}",
    );
    build_step(root, 7);
    assert!(output(root, "blog/index.html").contains("aria-current=\"page\""));
    assert!(root.join("public/blog/page/2.html").is_file());

    // 8. Add tags.
    write(
        root,
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'published-desc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 20\n",
    );
    write(
        root,
        "content/en/blog/first.page",
        "title = 'First post'\nsummary = 'A short first post.'\npublished = 2026-08-14T09:30:00\n\n[content]\nmain = 'first.md'\n\n[taxonomies]\ntags = ['Rust', 'Static sites']\n",
    );
    write(
        root,
        "templates/taxonomy-index.tera",
        "{% extends 'base.tera' %}{% block title %}Tags{% endblock %}{% block content %}<h1>Tags</h1><ul>{% for term in taxonomy.terms %}<li><a href=\"{{ term.relative_url }}\">{{ term.name }}</a> ({{ term.count }})</li>{% endfor %}</ul>{% endblock %}",
    );
    write(
        root,
        "templates/taxonomy-term.tera",
        "{% extends 'base.tera' %}{% block title %}{{ taxonomy.term.name }}{% endblock %}{% block content %}<h1>{{ taxonomy.term.name }}</h1>{% for post in pagination.pages %}<article><h2><a href=\"{{ post.relative_url }}\">{{ post.title }}</a></h2></article>{% endfor %}{% endblock %}",
    );
    build_step(root, 8);
    assert!(output(root, "tags/index.html").contains("Rust"));
    assert!(root.join("public/tags/Rust.html").is_file());

    // 9. Enable optional Markdown features.
    write(
        root,
        "heine.toml",
        "[site]\ntime_zone = 'Europe/Berlin'\n\n[i18n]\ndefault_locale = 'en'\n\n[i18n.locales.en]\ndirection = 'ltr'\n\n[i18n.locales.de]\ndirection = 'ltr'\n\n[collections.posts]\norder = 'published-desc'\n\n[taxonomies.tags]\ncollection = 'posts'\npath = 'tags'\nindex_template = 'taxonomy-index.tera'\nterm_template = 'taxonomy-term.tera'\nper_page = 20\n\n[markdown]\nextensions = ['tables', 'strikethrough', 'footnotes']\nmath = 'mathml'\n\n[markdown.tables]\nscroll_message = 'table-scroll'\n\n[markdown.highlighting]\nstylesheet = 'css/highlighting.css'\n\n[markdown.highlighting.theme]\nlight = 'github-light'\ndark = 'github-dark'\n",
    );
    write(
        root,
        "templates/base.tera",
        "<!doctype html><html lang=\"{{ locale.code }}\" dir=\"{{ locale.direction }}\"><head><title>{% block title %}{{ page.title }}{% endblock %}</title>{% set site_css = asset(id='assets/css/site.css') %}<link rel=\"stylesheet\" href=\"{{ site_css.url }}\"><link rel=\"stylesheet\" href=\"{{ highlight_css() }}\"></head><body><header>{% set home = page(id='index') %}<nav aria-label=\"{{ t(id='nav-main') }}\"><a href=\"{{ home.url }}\">{{ t(id='nav-home') }}</a></nav></header><main>{% block content %}{% endblock %}</main></body></html>",
    );
    write(
        root,
        "content/en/blog/second.md",
        "```rust\nfn main() {\n    println!(\"Hello, Heine!\");\n}\n```\n",
    );
    build_step(root, 9);
    assert!(root.join("public/css/highlighting.css").is_file());
    let highlighted_post = output(root, "blog/second.html");
    assert!(highlighted_post.contains("css/highlighting.css"));
    assert!(highlighted_post.contains("<pre"));

    // 10 and 11 point to focused deployment and licensing guides. They do
    // not prescribe another file for this tutorial site, so its current
    // state must continue to build unchanged.
    build_step(root, 10);
    build_step(root, 11);
}
