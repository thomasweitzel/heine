# Labels for the primary site navigation.
nav-label = Primary
nav-blog = Blog
nav-tags = Tags
nav-search = Search
nav-quiz = Quiz
nav-heine = Heinrich Heine
nav-series = Series

# Labels and metadata for generated Series indexes and member navigation.
series-index-title = Series
series-index-description = Ordered reading paths through this example site.
series-index-empty = There are currently no Series in this language.
series-heinrich-heine-title = Heinrich Heine
series-heinrich-heine-description = Three short chapters on the poet and prose writer.
series-navigation = Series navigation
series-label = Series
series-position = Part { $number } of { $total }
series-listing-description = Read the { $series } Series in order.

# Labels for post publication facts.
post-published = Published
post-byline = By { $authors }
post-updated = Updated

# Labels for page-local navigation controls.
toc-label = Table of contents
pagination-label = Pagination
page-navigation = Page navigation
quiz-menu = Quiz Menu

# Accessible labels for switching to a page's available language versions.
language = Language
language-en = English
language-de = German

# Taxonomy labels, metadata descriptions, and plural-aware collection summaries.
tags = { $count ->
    [one] Tag
   *[other] Tags
}
blog-post-summary = { $count ->
   [one] So far there is one post.
   *[other] So far there are { $count } posts.
}
taxonomy-index-description = Browse the tags used by posts in this Heine starter site.
taxonomy-term-description = Posts filed under { $term } in this Heine starter site.

# Atom feed metadata for the locale-local post collection.
feed-posts-title = Heine posts
feed-posts-subtitle = Notes about a small static-site generator
feed-subscribe = Subscribe with Atom

# Labels and status text for the client-side search example.
search-label = Search posts and documentation
search-placeholder = Enter a search term
search-submit = Search
search-loading = Loading search index…
search-results-count =
    .zero = { $count } results
    .one = { $count } result
    .two = { $count } results
    .few = { $count } results
    .many = { $count } results
    .other = { $count } results
search-no-results = No results found.
search-failure = The search index could not be loaded.
search-pagination-label = Search result pages
search-javascript-required = Search needs JavaScript in this example site.

# Footer text and configured standalone-page links.
footer-label = Footer navigation
footer-about = About Heine
footer-terms = Terms
footer-privacy = Privacy
footer-imprint = Imprint
attributions = Attributions
attributions-introduction = This page lists copied assets with declared licensing information.
license-texts = License texts
# Keep the <a> element while translating its visible text.
footer-description = A compact example site built with <a href="https://heine.rs">Heine</a>.
