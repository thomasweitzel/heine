# Your site

Run `heine serve` in this directory, then open the local address it prints.
Edit the files in `content/en/` to make the site yours. The home page is
`home.md`, About is `about.md`, and the posts live below `blog/`. Each `.page`
file chooses a template and holds the page's title and other settings.

Change the site name and navigation labels in `locales/en/site.ftl`. The
layout is in `templates/`, the stylesheet and search script are in
`content/assets/`, and the profile illustration is in
`content/assets/images/`. The site starts in English; add another configured
locale and its Fluent messages when you need a translation.

## Add a post

Create a directory under `content/en/blog/`, for example `my-post/`. Add an
`index.page` file there:

```toml
template = "article.tera"
title = "My post"
published = 2026-01-03T12:00:00
summary = "A short introduction for the blog listing."
description = "A short description for search results."
collections = ["posts"]
content.main = "main.md"

[taxonomies]
tags = ["Notes"]
```

Write the post in `main.md` in the same directory. The blog lists posts by
publication date, newest first. Posts with the same publication time are
ordered by page ID. Replace the sample dates. Publication times use UTC until
you set `time_zone` in `heine.toml` to your editorial time zone.
The summary appears in listings. Search matches both the description and
summary as well as the post text, and shows relevant context for each result.
See the [collections guide](https://heine.rs/docs/collections/) for other
ordering options.

## Add a Series

Add this to `heine.toml`:

```toml
[series]
path = "series"
index_template = "series-index.tera"
listing_template = "series.tera"
index_title_key = "series-index"
per_page = 10

[series.definitions.notes]
title_key = "series-notes"
```

Add a title to `locales/en/site.ftl`:

```ftl
series-notes = Notes
```

Add a membership to each post you want in the Series:

```toml
[[series]]
id = "notes"
position = 10
```

Give each post a different position. Heine creates a Series index and listing;
the article template shows links between neighboring posts. See the
[Series guide](https://heine.rs/docs/series/) for the full configuration.

## Publish

Set `[site] origin` in `heine.toml` to your real HTTPS origin so Heine can
provide canonical absolute URLs to templates. A sitemap or feed also needs its
own configuration; see the [sitemap](https://heine.rs/docs/sitemap/) and
[Atom feed](https://heine.rs/docs/atom-feeds/) guides.

## Supplied material

The supplied templates, configuration, sample content, styles, scripts, and
profile illustration are available under MIT. See `LICENSES/MIT.txt` and the
asset declarations for their notices. Content you add is yours to license.
This site is a snapshot: Heine does not update its files when you upgrade.
Check the [changelog](https://heine.rs/docs/changelog/) for migrations.
