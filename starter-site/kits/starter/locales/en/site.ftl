# Labels for the primary site navigation.
starter-nav-label = Primary
starter-nav-blog = Blog
starter-nav-tags = Tags
starter-nav-search = Search
starter-nav-heine = Heinrich Heine
starter-nav-series = Series

# Labels and metadata for generated Series indexes and member navigation.
starter-series-index-title = Series
starter-series-index-description = Ordered reading paths through this example site.
starter-series-index-empty = There are currently no Series in this language.
starter-series-heinrich-heine-title = Heinrich Heine
starter-series-heinrich-heine-description = Three short chapters on the poet and prose writer.
starter-series-navigation = Series navigation
starter-series-label = Series
starter-series-position = Part { $number } of { $total }
starter-series-listing-description = Read the { $series } Series in order.

# Labels for post metadata.
starter-draft-marker = DRAFT
starter-post-published = Published
starter-post-byline = By
starter-post-updated = Updated
starter-post-dates = Publication dates
starter-post-reading-facts = Word count and reading time
starter-word-count = { $words ->
   [one] One word
  *[other] { NUMBER($words) } words
}
starter-reading-time = about { $minutes ->
   [one] one minute
  *[other] { NUMBER($minutes) } minutes
} read
starter-reading-facts = { $words ->
   [one] One word
  *[other] { NUMBER($words) } words
} · about { $minutes ->
   [one] one minute
  *[other] { NUMBER($minutes) } minutes
} read

# Labels for page-local navigation and margin-footnote controls.
starter-toc-label = Table of contents
starter-footnote-marker = Footnote { $number }
starter-table-scroll = Scrollable table
starter-pagination-label = Pagination
starter-page-navigation = Page navigation

# Accessible labels for switching to a page's available language versions.
starter-language = Language
starter-language-en = English
starter-language-de = German

# Every configured taxonomy needs its plural-aware label plus index and term descriptions.
# This group also contains collection summaries that use the same plural convention.
starter-tags = { $count ->
    [one] Tag
   *[other] Tags
}
starter-authors = { $count ->
    [one] Author
   *[other] Authors
}
starter-blog-post-summary = { $count ->
   [one] So far there is one post.
   *[other] So far there are { NUMBER($count) } posts.
}
starter-tags-index-description = Browse the tags used by posts in this Heine starter site.
starter-tags-term-description = Posts filed under { $term } in this Heine starter site.
starter-authors-index-description = Browse the authors represented by readings in this Heine starter site.
starter-authors-term-description = Readings by { $term } in this Heine starter site.

# Atom feed metadata for the locale-local post collection.
starter-feed-posts-title = Heine posts
starter-feed-posts-subtitle = Notes about a small static-site generator
starter-feed-subscribe = Subscribe with Atom

# Labels and status text for the client-side search example.
starter-search-label = Search posts and documentation
starter-search-placeholder = Enter a search term
starter-search-loading = Loading search index…
starter-search-results-count =
    .zero = { $count } results
    .one = { $count } result
    .two = { $count } results
    .few = { $count } results
    .many = { $count } results
    .other = { $count } results
starter-search-no-results = No results found.
starter-search-failure = The search index could not be loaded.
starter-search-pagination-label = Search result pages
starter-search-javascript-required = Search needs JavaScript in this example site.

# Footer text and configured standalone-page links.
starter-footer-label = Footer navigation
starter-footer-about = About Heine
starter-footer-terms = Terms
starter-footer-privacy = Privacy
starter-footer-imprint = Imprint
starter-attributions = Attributions
starter-attributions-introduction = This page lists copied assets with declared licensing information.
starter-license-texts = License texts
# Keep the <a> element while translating its visible text.
starter-footer-description = A compact example site built with <a href="https://heine.rs">Heine</a>.
