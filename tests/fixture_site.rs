use std::{
    fs,
    time::{Duration, SystemTime},
};

use heine::{BuildMode, BuildOptions, build_at, build_at_with_options};

mod support;

use support::Fixture;

const PIXEL_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 31, 0, 3, 3, 2, 0,
    239, 191, 79, 242, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn write_bytes(fixture: &Fixture, relative: &str, contents: &[u8]) {
    let path = fixture.path().join(relative);
    fs::create_dir_all(path.parent().expect("fixture file has a parent"))
        .expect("fixture parent directory");
    fs::write(path, contents).expect("fixture file");
}

#[path = "fixture_site/content.rs"]
mod content;
#[path = "fixture_site/deploy.rs"]
mod deploy;
#[path = "fixture_site/feed.rs"]
mod feed;
#[path = "fixture_site/i18n.rs"]
mod i18n;
#[path = "fixture_site/licensing.rs"]
mod licensing;
#[path = "fixture_site/markdown.rs"]
mod markdown;
#[path = "fixture_site/metadata.rs"]
mod metadata;
#[path = "fixture_site/output.rs"]
mod output;
#[path = "fixture_site/pagination.rs"]
mod pagination;
#[path = "fixture_site/resources.rs"]
mod resources;
#[path = "fixture_site/search.rs"]
mod search;
#[path = "fixture_site/site_data.rs"]
mod site_data;
#[path = "fixture_site/sitemap.rs"]
mod sitemap;
#[path = "fixture_site/taxonomy.rs"]
mod taxonomy;
#[path = "fixture_site/template.rs"]
mod template;
