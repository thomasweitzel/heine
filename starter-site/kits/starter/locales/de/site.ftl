# Labels for the primary site navigation.
starter-nav-label = Hauptnavigation
starter-nav-blog = Blog
starter-nav-tags = Schlagwörter
starter-nav-search = Suche
starter-nav-reading = Lektüre
starter-nav-heine = Heinrich Heine
starter-nav-series = Reihen

# Labels and metadata for generated Series indexes.
starter-series-index-title = Reihen
starter-series-index-description = Geordnete Lesepfade durch diese Beispielseite.
starter-series-index-empty = In dieser Sprache gibt es derzeit keine Reihen.
starter-series-navigation = Reihennavigation
starter-series-label = Reihe
starter-series-position = Teil { $number } von { $total }
starter-series-listing-description = Lies die Reihe { $series } in ihrer vorgesehenen Reihenfolge.

# Beschriftungen für Beitragsmetadaten.
starter-draft-marker = ENTWURF
starter-post-published = Veröffentlicht am
starter-post-byline = Von
starter-post-updated = Aktualisiert am
starter-post-dates = Datumsangaben zum Beitrag
starter-post-reading-facts = Wortzahl und Lesezeit
starter-word-count = { $words ->
   [one] Ein Wort
  *[other] { NUMBER($words) } Wörter
}
starter-reading-time = etwa { $minutes ->
   [one] eine Minute
  *[other] { NUMBER($minutes) } Minuten
} Lesezeit
starter-reading-facts = { $words ->
   [one] Ein Wort
  *[other] { NUMBER($words) } Wörter
} · etwa { $minutes ->
   [one] eine Minute
  *[other] { NUMBER($minutes) } Minuten
} Lesezeit

# Beschriftungen für lokale Navigation und Fußnotensteuerungen.
starter-toc-label = Inhaltsverzeichnis
starter-footnote-marker = Fußnote { $number }
starter-table-scroll = Scrollbare Tabelle
starter-pagination-label = Seitennavigation
starter-page-navigation = Seitennavigation

# Accessible labels for switching to a page's available language versions.
starter-language = Sprache
starter-language-en = Englisch
starter-language-de = Deutsch

# Jede konfigurierte Taxonomie braucht ihre pluralfähige Bezeichnung sowie
# Beschreibungen für Index und Begriff. Diese Gruppe enthält außerdem
# Sammlungshinweise mit derselben Pluralregel.
starter-tags = { $count ->
    [one] Schlagwort
   *[other] Schlagwörter
}
starter-authors = { $count ->
    [one] Autor
   *[other] Autorinnen und Autoren
}
starter-blog-post-summary = { $count ->
   [one] Bisher gibt es einen Beitrag auf Deutsch.
   *[other] Bisher gibt es { NUMBER($count) } Beiträge auf Deutsch.
}
starter-tags-index-description = Durchsuche die Schlagwörter der Beiträge in dieser Heine-Starter-Site.
starter-tags-term-description = Beiträge mit dem Schlagwort { $term } in dieser Heine-Starter-Site.
starter-authors-index-description = Durchsuche die Autorinnen und Autoren der Lektüren in dieser Heine-Starter-Site.
starter-authors-term-description = Lektüren von { $term } in dieser Heine-Starter-Site.

# Atom feed metadata for the locale-local post collection.
starter-feed-posts-title = Heine-Beiträge
starter-feed-posts-subtitle = Notizen über einen kleinen Static-Site-Generator
starter-feed-subscribe = Atom abonnieren

# Labels and status text for the client-side search example.
starter-search-label = Beiträge und Dokumentation durchsuchen
starter-search-placeholder = Suchbegriff eingeben
starter-search-submit = Suchen
starter-search-loading = Suchindex wird geladen…
starter-search-results-count =
    .zero = { $count } Ergebnisse
    .one = { $count } Ergebnis
    .two = { $count } Ergebnisse
    .few = { $count } Ergebnisse
    .many = { $count } Ergebnisse
    .other = { $count } Ergebnisse
starter-search-no-results = Keine Ergebnisse gefunden.
starter-search-failure = Der Suchindex konnte nicht geladen werden.
starter-search-pagination-label = Suchergebnisseiten
starter-search-javascript-required = Die Suche benötigt auf dieser Beispielseite JavaScript.

# Footer text and configured standalone-page links.
starter-footer-label = Fußnavigation
starter-footer-about = Über Heine
starter-footer-terms = Nutzungsbedingungen
starter-footer-privacy = Datenschutz
starter-footer-imprint = Impressum
starter-attributions = Lizenzhinweise
starter-attributions-introduction = Diese Seite listet kopierte Dateien mit ihren angegebenen Lizenzinformationen auf.
starter-license-texts = Lizenztexte
# Behalte das <a>-Element bei und übersetze seinen sichtbaren Text.
starter-footer-description = Eine kompakte Beispielseite mit <a href="https://heine.rs">Heine</a>.
