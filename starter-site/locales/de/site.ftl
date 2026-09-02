# Labels for the primary site navigation.
nav-label = Hauptnavigation
nav-blog = Blog
nav-tags = Schlagwörter
nav-search = Suche
nav-quiz = Quiz
nav-heine = Heinrich Heine
nav-series = Reihen

# Labels and metadata for generated Series indexes.
series-index-title = Reihen
series-index-description = Geordnete Lesepfade durch diese Beispielseite.
series-index-empty = In dieser Sprache gibt es derzeit keine Reihen.
series-navigation = Reihennavigation
series-label = Reihe
series-position = Teil { $number } von { $total }

# Labels for post publication facts.
post-published = Veröffentlicht am
post-byline = Von { $authors }
post-updated = Aktualisiert am

# Labels for page-local navigation controls.
toc-label = Inhaltsverzeichnis
pagination-label = Seitennavigation
page-navigation = Seitennavigation
quiz-menu = Quiz-Menü

# Accessible labels for switching to a page's available language versions.
language = Sprache
language-en = Englisch
language-de = Deutsch

# Taxonomy labels and plural-aware summaries for collection listings.
tags = { $count ->
    [one] Schlagwort
   *[other] Schlagwörter
}
blog-post-summary = { $count ->
   [one] Bisher gibt es einen Beitrag auf Deutsch.
   *[other] Bisher gibt es { $count } Beiträge auf Deutsch.
}

# Atom feed metadata for the locale-local post collection.
feed-posts-title = Heine-Beiträge
feed-posts-subtitle = Notizen über einen kleinen Static-Site-Generator
feed-subscribe = Atom abonnieren

# Labels and status text for the client-side search example.
search-label = Beiträge und Dokumentation durchsuchen
search-placeholder = Suchbegriff eingeben
search-submit = Suchen
search-loading = Suchindex wird geladen…
search-results-count =
    .zero = { $count } Ergebnisse
    .one = { $count } Ergebnis
    .two = { $count } Ergebnisse
    .few = { $count } Ergebnisse
    .many = { $count } Ergebnisse
    .other = { $count } Ergebnisse
search-no-results = Keine Ergebnisse gefunden.
search-failure = Der Suchindex konnte nicht geladen werden.
search-pagination-label = Suchergebnisseiten
search-javascript-required = Die Suche benötigt auf dieser Beispielseite JavaScript.

# Footer text and configured standalone-page links.
footer-label = Fußnavigation
footer-about = Über Heine
footer-terms = Nutzungsbedingungen
footer-privacy = Datenschutz
footer-imprint = Impressum
attributions = Namensnennungen
attributions-introduction = Diese Seite listet kopierte Dateien mit ihren angegebenen Lizenzinformationen auf.
license-texts = Lizenztexte
# Behalte das <a>-Element bei und übersetze seinen sichtbaren Text.
footer-description = Eine kompakte Beispielseite mit <a href="https://heine.rs">Heine</a>.
