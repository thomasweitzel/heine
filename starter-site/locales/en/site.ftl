# Labels for the primary site navigation.
nav-label = Primary
nav-blog = Blog
nav-tags = Tags
nav-search = Search
nav-quiz = Quiz
nav-heine = Heinrich Heine
toc-label = Table of contents

# Accessible labels for switching to a page's available language versions.
language = Language
language-en = English
language-de = German

# Labels for listing and quiz navigation controls.
pagination-label = Pagination
page-navigation = Page navigation
open = Open
quiz-menu = Quiz Menu

# Taxonomy labels and plural-aware summaries for collection listings.
tags = { $count ->
    [one] Tag
   *[other] Tags
}
blog-post-summary = { $count ->
   [one] So far there is one post.
   *[other] So far there are { $count } posts.
}

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
footer-description = A compact example site built with <a href="https://heine.rs">Heine</a>.
