# Citation investigation findings

The citation investigation used Hayagriva 0.10.1, `biblatex` 0.12.0, and
`pulldown-cmark` 0.13.5. The findings below describe sampled inputs, not full
CSL conformance. Heine's production tests cover the behaviors on which the
current implementation relies. See the [citation design](citations.md) for
the feature's scope and remaining decisions.

## Inputs and shared Markdown analysis

Raw BibTeX files can be resolved together, allowing an abbreviation in one
file to be used in another. The library resolves duplicate abbreviation
definitions according to their input order. Heine checks cycles and bounded
expansion before invoking its recursive resolver, and preserves source spans
for diagnostics. Production tests cover cross-file references, cycles, and the
depth and work limits.

Dependent CSL styles identify their parent and can use a locally supplied
parent. The host must select that parent without a network request. In a
sampled German dependent style, selecting its locale changed the conjunction
used by both citation and bibliography output.

The existing Markdown analysis walk can collect citations alongside headings
and footnotes. Heine's regression tests exercise a real build with all three,
including component and repeated margin-footnote cases. Inline code, fenced
code, links, images, raw HTML, and renderer bodies need their own boundaries;
production tests cover these cases. The broken-link callback alone cannot
recognize all citation-shaped input or detect resolved-reference conflicts.

## Formatting and links

Hayagriva exposes structured formatting that Heine can turn into Markdown
renderer events. The processor's HTML writer did not escape authored markup
in a sampled bibliography, so Heine escapes text and attributes through its
own output adapter. Tests cover hostile values and representative APA and
IEEE output; these examples do not establish conformance for every style.

In sampled numeric styles, collapse could omit a middle citation's locator.
Disabling collapse for the affected request preserved that locator, while also
expanding other ranges in that request. Splitting one group into separate
processor requests can alter position semantics and is not an equivalent fix.

The library retained a BibTeX `\\&` URL escape in a sample, but silently
dropped a malformed URL during conversion. Heine checks URLs before conversion.
The investigation did not establish a general normalization rule. Heine
reports escapes such as `\&` and asks authors to write URL characters
literally; it also rejects consecutive hyphens that the parser would change.
`biblatex` exposes resolved chunks and permits replacing an entry's fields,
but a replacement would have to recover the intended URL across authored
fields, abbreviations, and inherited values before Hayagriva converts it.
That API alone does not establish that such recovery preserves every URL, so
the checks remain in place.

The `resolved_url_spans_do_not_always_identify_only_the_url` and
`url_span_can_include_unrelated_hyphens` tests show why the hyphen check must
use raw input. A resolved chunk assembled from an abbreviation and a literal
can have a span that covers unrelated entries between them. Heine checks the
actual raw URL fields and referenced abbreviations, avoiding a false rejection
when unrelated text contains `--`.
After bibliography parsing, URL checks follow entry order. A hyphen error is
ordered at the entry that declares the URL, even when an earlier entry inherits
it. Its diagnostic points to the actual fragment containing the hyphens.

Per-item prefixes and suffixes need more work: collapsed and reordered output
does not always expose the mapping needed to attach them without changing
punctuation or formatting. The first implementation leaves them out.

## Note styles and position conditions

Hayagriva can treat the final item of a multi-item group as the previous item
for `ibid`, even when CSL's cross-group rule does not allow that result. A
sampled comparison with Pandoc selected the non-`ibid` branch. Sorting and
locators can affect the relevant processor sequence, so a document-specific
exception needs further evidence. Heine conservatively rejects reachable
position conditions that it cannot handle reliably.

In Hayagriva 0.10.1, the final position calculation compares each sorted item
with the preceding item, including items within a group and the last item of a
previous group. A narrower check would need to account for that ordering,
repeated keys, locators, and processor transformations before it could safely
permit a style such as Chicago author-date. The existing style-level check
therefore remains in place.

A structural scan of 80 independent bundled styles found reachable `ibid` or
`ibid-with-locator` conditions in four of 75 in-text styles and all five note
styles. No scanned style reached `near-note`. This sample was not weighted by
usage and did not resolve dependent styles. Note-generating styles also need
an integrated numbering and punctuation design for both Heine footnote modes.

## Large bibliography study

On November 1, 2026, local tests resolved the published CryptoBib files with
the limits used by Heine. `abbrev3.bib` with `crypto.bib` contained 90,839
entries; with `crypto_crossref.bib` it contained 92,193. The maximum reachable
abbreviation depth was two links and inheritance depth was one link. This is
one corpus, not a general bound.

A single release-build run rendered one numeric citation using `abbrev3.bib`
and `crypto.bib` in 3.63 seconds, with a peak resident set of 1,030,920 KiB.
A small citation-free fixture took less than 0.01 seconds and 11,776 KiB with
that binary. The installed Heine 0.10.1 binary took 0.02 seconds and 10,880 KiB
on the same small fixture. The binaries were 19,193,240 and 15,736,856 bytes
respectively. These single runs compare different versions and are not a
controlled benchmark or an estimate for typical sites.

After changing the inheritance check to copy only the fields it needs, three
sequential release-build runs of the same fixture took 3.45, 3.43, and 3.47
seconds. Their peak resident sets were 1,030,956, 1,030,952, and 1,030,904
KiB. The earlier single run peaked at 1,030,920 KiB. This measurement shows no
meaningful change in peak memory for this fixture; it does not identify which
stage determines that peak.

A subsequent symbolized debug profile attributed large allocations to
Hayagriva's conversion of bibliography entries and their parents. Heine now
collects converted entries directly into the library, avoiding Hayagriva's
intermediate vector while still converting every entry. Tests compare the
result with the library's conversion and check the first error from an uncited
entry. The profile is an indication of allocation ownership, not a release
benchmark.

With direct collection, three release-build runs of the same fixture took
3.50, 3.59, and 3.45 seconds, with peak resident sets of 947,112, 947,112,
and 946,860 KiB. Compared with the three runs immediately above, peak RSS fell
by approximately 82 MiB; elapsed time remained similar. This is a local
comparison on one unusually large bibliography, not a general performance
guarantee.

The bibliography SHA-256 digests were:

```text
abbrev3.bib         725f1db80eae6800ebf65d5df898027ee9e1339cfcd74c4b955649a23e512c81
crypto.bib          fc7476982c7d18288cc9ea69f5c20bb2328e0c34542c51e6096e8f87c9edf80f
crypto_crossref.bib b0e4c101cf75e11efdb650e7f415121a05c772eeffd8d4090514ac16f8106007
```
