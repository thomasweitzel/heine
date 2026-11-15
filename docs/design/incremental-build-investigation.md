# Incremental-build pipeline investigation

This supporting record traces candidate reuse boundaries through Heine’s current
pipeline. It informs the [incremental-build design](incremental-builds.md);
it does not specify an implemented cache or a public behavior.

## Pipeline traces

These traces follow the current full-build code. They identify what Heine
already retains and what an incremental implementation would still need; they
do not claim that a cache or a dependency recorder exists today.

### Edit a declared Markdown body

Consider an English page that declares `content.main = "post.md"`, belongs
to a `posts` collection, and is included in a search index. The author changes
only the body text. The page declaration, route, and collection membership do
not change.

1. [`load_page`](../../src/site/loader.rs) reads the page declaration and its
   declared content. It records declared content paths to distinguish inputs
   from copied assets and folds their modification times into the task's
   `latest_input`. That time does not record the content's identity or bytes.
2. [`Site::load`](../../src/site/mod.rs) resolves collections and search
   selections, constructs locale-aware page and asset links, and calls
   `analyze_markdown` for each page. [`Page::analyze_markdown`](../../src/site/page.rs)
   derives word counts and selected search sections and checks source-aware
   features. Depending on the page, the same pass also handles components,
   citations, and renderer requests and resources. It is not currently a
   standalone pure "count words" computation.
3. Search builds its JSON from selected pages and their sections. If the index
   is versioned, changed JSON changes its output path and the search view
   exposed to templates. [`search::Catalog`](../../src/site/search.rs) creates
   those facts and claims. A template that uses `search()` may therefore need
   a new URL even if it does not render the edited page.
4. [`Site::render`](../../src/site/render.rs) constructs template resources
   from the resolved catalogs, loads the template engine, and renders tasks.
   A paginated listing's view uses its member IDs and the page catalog. A
   template can also use checked functions to look up other pages or site
   resources. Heine does not currently record every such read per output.

The collection's ordered IDs may remain unchanged while a rendered listing or
search URL changes. Treating collection membership as the listing's only
dependency would therefore be unsound. A coarse first policy could rebuild the
resolved model and invalidate all renders that might read changed page or
search views. A narrower policy requires a complete record of which views each
render actually consumed. Existing tests distinguish quick-build behavior from
the full-build contract, including
[`quick_build_keeps_the_complete_search_index`](../../tests/fixture_site/search.rs)
and [`quick_build_rerenders_every_affected_pager_when_a_collection_member_changes`](../../tests/fixture_site/pagination.rs).

### Delete a page

Now remove the `.page` declaration for `guide/setup`, while another page still
contains `[Setup](page:guide/setup)`. Leave its formerly declared Markdown file
on disk.

1. [`locales::load`](../../src/site/locales.rs) discovers the current content
   trees and loads present page declarations. It also collects their declared
   content paths. The new discovery state no longer includes the deleted page
   or its declaration of the Markdown file.
2. Collection resolution, pagination, translations, taxonomy and Series
   resolution, and page targets use the new page set. The previous successful
   analysis cannot be reused merely because the referring source did not change.
3. The orphaned Markdown file may now be an ordinary copied asset: declared
   content is excluded from assets, while other regular content files are
   assets. Its changed classification can affect asset licensing, output
   claims, and collisions. A design that records only "remove the page's HTML"
   misses this change. The exact result depends on the remaining declarations
   and site configuration.
4. If the earlier checks succeed, source analysis reports the broken `page:`
   reference at the authored link. Asset discovery and licensing run before
   source analysis and can instead report an earlier error. An incremental
   build must follow the clean build's applicable failure behavior.
5. If the source reference is corrected, full output planning must omit the
   removed page and pager paths while accounting for any newly copied asset.
   Full publication replaces the managed tree; an in-place quick build can
   leave old paths behind.

A sound coarse approach reruns discovery, relationship checks, source analysis
against the new catalogs, and complete output planning. A finer approach needs
both previous and current discovery state, including the absence of the page
and the changed content/asset classification. An in-memory server and a CLI
cache need the same meaning for these facts, although their retained values and
storage may differ.

### Edit a template used through an include

Consider `post.tera` including `navigation.tera`. An author changes only
`navigation.tera`, leaving page and content files unchanged.

1. Every current build loads templates through [`TemplateEngine::load`](../../src/template.rs).
   `Site::render` validates configured page, taxonomy, and Series template
   references, including those with no current tasks. Full mode then renders
   the planned tasks. The current quick render decision instead compares a
   task's `latest_input` with the output file's time; that summary does not
   establish whether an included template changed.
2. Tera receives serialized render contexts and can evaluate conditional
   includes and Heine functions. A static list of selected top-level template
   names is insufficient to prove which prior outputs remain valid. A template
   edit can also change source-aware rendering calls or introduce a Tera error,
   even if its current rendered bytes would otherwise look reusable.
3. A first complete incremental policy can reload and check templates and
   rerender all template-produced outputs on any template or selected-kit
   template change. This is deliberately broad. Refinement requires an
   established dependency mechanism for inheritance, includes, components,
   dynamic choices, and called Heine functions, including the absence of a
   previously missing template.

For the first policy, a Heine-owned engine records the template-set change and
invalidates a broad render scope. A Salsa-based implementation could represent
the same template set as a tracked input, but it would still need to route the
template data through tracked queries; merely calling Tera inside a tracked
function does not capture arbitrary filesystem or serialized-context reads.
Neither approach is shown to provide fine-grained template invalidation today.

### Edit a bibliography shared by pages

Consider two pages that select the same bibliography and CSL style, and a
third page with no citations. An author edits an entry in the bibliography.

1. [`InputCache`](../../src/site/citation_inputs.rs) shares parsed bibliography
   inputs within one build when their ordered file selections and root match.
   Citation settings also identify the style, optional parent style, selected
   Markdown source, and optional CSL locale. Directory defaults can change a
   page's effective setting; they are part of the input, not a property of the
   bibliography file alone.
2. [`Page::analyze_markdown`](../../src/site/page.rs) collects each selected
   source's citation groups, checks their keys against the resolved
   bibliography, and formats the page's citations and bibliography. A changed
   entry can change formatted results or cause an error even when the Markdown
   has not changed. Formatting also depends on the page's citation sequence,
   effective style, and CSL locale. CSL locale data is separate from Fluent.
3. A template can render the page's bibliography or obtain page and
   publication views through checked functions. Full model construction will
   recheck the citation inputs, but it does not prove that another task's
   previously rendered output is independent of changed page or publication
   data.

The existing build-scoped input cache gives a natural sharing boundary for
parsing, but it does not persist across builds or record every render consumer.
A future retained citation result would need the full effective inputs and
checked source sequence; a retained render also needs its own dependencies.
Do not invalidate a page solely because it shares a file, nor declare it
unaffected solely because it has no citations, without establishing what its
render can read. Broad invalidation remains the safe initial choice.

### Edit a Fluent message

Consider a site with English as its default locale and German pages. An author
edits an English message, then later adds a German override for that message.

1. [`Localization::load`](../../src/localization.rs) reads the configured
   locales and the applicable site and selected-kit resources. It assembles
   message bundles and formatting plans. Message lookup uses the current
   locale and then the configured default locale, so the English edit can
   affect German output that falls back to English.
2. Adding the German override changes which resource supplies the message.
   Tracking only the previously selected English file would miss that newly
   available value. Kit and site resource precedence and message references
   also need to remain part of the result and its diagnostics.
3. Templates, components, and generated-resource planning can consume
   localized values. Citation formatting instead uses CSL locale data, so a
   Fluent edit does not itself change CSL terms. The same page can nevertheless
   combine Fluent labels with a formatted bibliography.

The current localization model already identifies a useful broad boundary:
an effective locale depends on its own resources and the default locale's
resources. Rebuilding and checking all localization data before rendering
preserves current errors. A hypothetical per-locale render dependency could
then be narrower than one global localization dependency, provided all other
observable inputs are accounted for. It is not a complete render dependency:
templates can also consult shared page and site views, and changes to configured
locales or shared template inputs can have wider effects.

### Test the boundary against rendering and publication

The candidate keeps the full model and its checks, but that alone does not
prove that an earlier rendered file is reusable. The following cases identify
additional inputs and work that a reuse decision must account for.

**Markdown block components.** A `:::component` call is more than text in a
Markdown file. [`Page::analyze_markdown`](../../src/site/page.rs) plans the
component and retains its source. During each relevant template render,
[`prepare_component_markdown`](../../src/template.rs) renders its body with
the current page and locale, checked page and asset links, and any applicable
table of contents, citations, or footnotes. The component template also
receives the render context. Heine checks the component's body contract and
reports failures against the authored source. Renderer requests inside a
component body are currently rejected, so they are not a supported interaction
to cache.

An unchanged Markdown file therefore does not establish that its component
output is unchanged or that its checks would still pass. A component template,
its implicit context, a referenced resource, or a source-aware plan can change.
The initial broad template invalidation must include component templates and
their dependencies. A narrower boundary would have to account for the context
and checked operations used by each component as well as its Markdown body.

**Assets.** Current builds discover assets and, when integrity metadata is
configured, [`integrity_values`](../../src/asset.rs) reads their bytes before
rendering. Templates and Markdown can resolve assets, and the resulting views
can include paths, dimensions, and integrity values. A changed asset can also
affect copied output or licensing information without changing a page's own
source. Reconstruct the current asset catalog and output claims; conservatively
invalidate renders that might consume changed asset facts. Reusing their HTML
is safe only when the facts they could observe are unchanged.

**External renderers.** Markdown analysis currently handles renderer requests
as part of building the site model. Heine already caches successful results by
profile, invocation, request and other declared inputs; the site owner supplies
`cache_identity` for external changes Heine cannot observe. The existing
[`renderer_cache_key`](../../src/renderer/cache.rs) hashes argument strings,
not the contents of arbitrary files named by those arguments. An incremental
layer must preserve this documented boundary and must not let reused page HTML
defeat `--refresh-renderers`, which asks Heine to rerun permitted requests.
After a refresh, a reuse decision must reflect the resulting bytes and resource
facts, not just the previous request key.

**Full publication.** A render cache cannot replace the current output plan.
Heine still needs every current claim and a complete staged tree, including
copied assets and generated renderer resources. An old output file is not
necessarily an unmodified copy of the previous result. Reuse therefore needs
verified bytes or a separate trusted artifact, and must preserve the existing
publication and recovery behavior described in the [design record](incremental-builds.md#publication-and-failure).

These boundaries also limit the likely benefit of the candidate. Rebuilding the
model still discovers and analyzes sources, and staging still writes a complete
tree. Reusing renders could save template and component rendering, but broad
invalidation may require many or all of those renders after a common edit,
especially when templates can read other pages or a changed search view. A
retained artifact also costs storage and checking; retaining two complete site
models could substantially increase memory use. No performance improvement is
established yet. The following comparison considers the smallest useful state
and its expected work and memory for both a running server and separate CLI
builds.

### Render reuse: state and costs

A conservative render cache would need compatible build and template inputs,
per-task identity and context, all relevant checked resource observations,
verified output bytes, and a relationship to the published generation. The
server could keep some state in memory; a separate CLI process would need a
compatible durable representation. Both still rebuild the current checked
model and stage complete output. A missing or uncertain dependency requires
rerendering.

For *N* tasks, *D* observation references, and *O* retained output bytes,
records require O(*N* + *D*) entries and retained artifacts require O(*O*)
space. Comparing full contexts and resource values can repeat serialization
or byte processing. Shared comparisons could reduce repeated work only when
their complete identities, including locale and output context, match. This
is a complexity estimate, not a measurement or evidence that the saved
rendering outweighs these costs.

### Representative template observations

Heine's two supplied sites illustrate what this boundary would have to cover.
These are static traces of their current templates, not measurements or a
complete inventory of all templates a site can author.

The generated site's [`base.tera`](../../new-site/templates/base.tera) calls
`page()` for home, About, blog, and search navigation on every extending
render. It uses each returned view's URL. The article template additionally
uses its current page, localization, and `markdown(content="main")`; its
optional table of contents, taxonomy, and Series sections add further inputs
when present. The blog listing uses its pagination view, dates, summaries,
taxonomy lookup, and included pagination template. Its search page calls
`search(name="site")` to obtain the index view.

For an edit to an ordinary post body, unrelated article renders could
potentially be reused if their complete contexts and observed resources remain
equal. The edited article must reflect its new body, and the search output and
its consumer must be reconsidered when the index view changes. A listing must
also be reconsidered when its supplied pagination view changes; unchanged
ordering alone is not enough to prove equality. These are candidate effects,
not an invalidation rule established by the current implementation.

An About body edit exposes a cost of coarse observations. The shared base
template reads only `about.url`, but `page()` returns a broader page view that
includes authored content. Comparing the whole returned view would be
conservative and could invalidate every render using that navigation even when
its URL is unchanged. Recording only the URL actually read could narrow this
case, but Tera receives a serialized value and Heine does not currently record
individual field reads. Such field tracking needs its own feasibility and cost
assessment; it must not be assumed from recording the `page()` call.

The starter kit's [`base.tera`](../../starter-site/kits/starter/templates/base.tera)
adds data-driven navigation, conditional `page_exists()` and `page()` calls,
locale-dependent translations and assets, a feed, and an included Series
navigation template. Its [`section-index.tera`](../../starter-site/kits/starter/templates/section-index.tera)
can use pagination or look up each sibling page and display derived reading
facts. These examples require observations of current context and runtime
lookups, including negative existence results and conditional choices. A
change in an earlier condition must cause the render to run again so it can
record the lookups of the newly selected branch.

The starter site's [Heine guide](../../starter-site/content/en/heine/main.md)
also uses `:::component[starter.note]`. The component's
[template](../../starter-site/kits/starter/templates/components.tera) inserts
its body, while the host checks that body and renders the surrounding Markdown
with the current source-aware context. This particular component is simple,
but the reuse contract must also accommodate components that consume implicit
context or make checked calls. It cannot infer their dependencies from this
one example.

These examples make whole-result observations a credible conservative design
candidate, with a concrete risk of unnecessary invalidation. They do not show
that observing only public function calls is complete, because direct context,
Markdown rendering, component preparation, and generated resources also matter.
Before choosing it, compare the implementation and memory cost of recording
and rechecking the complete boundary against the renders it could avoid. Keep
field-level tracking and any Tera changes as separate options, not prerequisites
assumed by this design.

### Consequences of whole-result observations

The generated site's pagination view contains complete page views, including
content, although its listing displays selected fields. Comparing the whole
view can invalidate the listing after an invisible body change. Likewise, the
About navigation lookup can invalidate many renders even when its URL is
unchanged. Deduplicating comparisons can reduce repeated work but does not
narrow these dependencies.

A component-bearing page can skip component preparation and its host checks
only if the component template, source-aware plans, context, and observations
are all unchanged. Otherwise the candidate must perform the preparation and
checks. These examples do not prove that capturing every required observation
is practical or cheaper than rendering.

### Compare citation input reuse with render reuse

[`InputCache`](../../src/site/citation_inputs.rs) already gives bibliography
resolution a defined ownership boundary. During one build, identical ordered
file selections under the same root share one resolved bibliography, including
between citations and a publication catalog. Style-specific selections share
their own inputs. Conversion to the citation processor's library is lazy and
shared for that bibliography; page-specific citation collection and formatting
remain separate. The cache is created anew by each `Site::load`.

A possible in-memory server design could retain a successfully resolved
bibliography and its lazy conversion across rebuilds. Before reuse, Heine
would still check the selected paths, reject symlinks and non-files, and read
the current files. Equality must include the root, ordered selection, complete
file contents, and the applicable Heine and dependency behavior. File order
matters because abbreviation definitions can depend on it. A changed,
unreadable, missing, or newly symlinked input must follow the normal error or
recomputation path rather than use the old result. Current page and directory
settings must still be resolved, and errors must point to the current authored
locations.

This reuse would avoid bibliography parsing and resolution, and potentially a
repeated conversion, when those inputs have not changed. It would not avoid
reading their bytes, rebuilding the rest of the model, collecting and checking
each page's citation groups, formatting each page, rendering, or full
publication. A change to a bibliography invalidates its retained resolution;
a style change requires separate style handling but need not invalidate an
unchanged bibliography. CSL locale and the citation sequence remain
page-specific. A retained bibliography must also serve publication catalog
consumers with the same semantics as a clean build.

For *B* input bytes and retained resolved data of size *R*, checking a
bibliography still requires O(*B*) byte reading or comparison in this
candidate; retaining its parsed representation adds O(*R*) memory between
server builds. During a rebuild, the old and replacement values might coexist
until all consumers release them. Large bibliographies make both the potential
saved parse and the retained memory relevant. Their exact time and memory costs
are not established here, and citation-free sites gain nothing from this
specific reuse.

The separate CLI lifetime is less straightforward. Serializing a
library-owned parsed representation would require a compatible durable format
and a way to restore the same checked facts and source diagnostics; the
existing in-memory types do not establish that contract. Persisting only the
authored bytes saves neither reading the current inputs nor parsing them. A CLI
could initially use the same identity and invalidation semantics while doing
the parse again, but that would be a performance difference, not persistent
reuse. In either lifetime, incompatible or uncertain state must fall back to
normal loading.

Compared with rendered-output reuse, bibliography reuse has a smaller,
domain-owned input boundary and avoids Tera read tracking and output artifact
management. It is also narrower in value: only sites with a bibliography can
benefit, and every build still does substantial work. It is a credible candidate
for a later focused optimization, but it is not evidence that dependency-aware
incremental builds are solved. The design should not select it as the first
implementation without comparing its likely benefit and retained memory with
other common build costs, or add a serialization layer merely to make the CLI
match the server's performance.

### Markdown analysis as a candidate

Markdown analysis applies to more sites than bibliography reuse. The current
[`analyze_source_with_links`](../../src/markdown.rs) walk combines source
facts with checks against current catalogs and settings; it is not a pure
function of source bytes. The following candidates deserve comparison:

- Retain a complete successful analysis result and reuse it only when its
  source and a conservative identity for every relevant input remain equal.
  A changed or uncertain dependency runs the existing path again. A body edit
  may leave a broad page-target catalog unchanged; a structural change may
  invalidate many sources. The existing `MarkdownAnalysis` is not the entire
  page-level result, so the correct unit and inputs remain to be established.
- As a variant, record the checked page and asset lookups made by that walk,
  including relevant missing or shadowed selections. Recheck their outcomes
  against the current model; a mismatch reruns the original walk. This may
  avoid invalidating sources after unrelated catalog changes without replaying
  individual validation checks. It still needs complete observation of all
  lookup behavior and identities for settings and other non-lookup inputs.
- Retain a source-derived plan with authored spans and selected observations,
  then replay current-model checks for an unchanged source. This could allow
  narrower invalidation, but introduces a separate replay responsibility and
  must not duplicate or drift from the existing checks.

None of these candidates automatically avoids later template-driven Markdown
rendering or full publication. Persisting results for a CLI invocation would
need a separate format and recovery contract. Reusing successful results and
falling back on uncertainty helps preserve diagnostics, but cannot compensate
for an omitted dependency that incorrectly appears unchanged.

For *M* source bytes and *K* recorded constructs, retaining full source or
event data can add O(*M*) space, while a compact plan has O(*K*) entries plus
facts and spans. Inputs still need to be read or compared, and replay costs
depend on observations and lookups. These bounds establish neither a speedup
nor acceptable retained memory. Unlike render reuse, this boundary need not
track arbitrary Tera field reads or retain rendered page artifacts; unlike
bibliography reuse, it crosses links, diagnostics, components, and renderers.

### Inventory whole successful-analysis reuse

There are at least two possible units. The narrow unit is the successful
[`analyze_source_with_links`](../../src/markdown.rs) call for one source or
segment, together with the citation collector output it produces separately.
Its `MarkdownAnalysis` contains facts, search sections, and resolved renderer
requests. The broader unit is the successful
[`Page::analyze_markdown`](../../src/site/page.rs) operation over all declared
Markdown on a page. The latter also formats citations, updates TOC text,
executes renderer requests, rebases source-aware plans, and changes page data.
Neither unit is currently retained across builds.

A conservative comparison for the narrow unit needs to cover at least:

- The authored source bytes, declared content identity and segment boundaries,
  plus component-normalized input and its relationship to authored spans.
- Markdown options affecting parser events or validation, math and syntax
  highlighting behavior, the effective reading rate, search-selection flag,
  and the TOC anchors supplied to section collection.
- Locale-aware page targets and excluded drafts, including target identities,
  routes, locale selection, and lookup behavior. The current validation
  [`PageLinks`](../../src/site/links.rs) is built from targets rather than
  full page content, so another page's body edit need not change this input.
- Locale-aware asset lookup facts, including available IDs, selected versions,
  paths and image dimensions needed to check links and pictures. Presence and
  absence both matter.
- Renderer profiles and named source declarations used to validate requests,
  whether request scanning is enabled, and whether citation collection and
  heading collection are enabled. The separate collector result must also be
  retained or recomputed; `MarkdownAnalysis` alone does not contain it.

A successful return from the narrow walk is not necessarily a successful
page analysis. The citation collector can retain problems that
[`Page::analyze_markdown`](../../src/site/page.rs) reports after the walk
returns. A reusable result must retain those problems with their authored
spans and let the caller process them in the same stage; it must not discard
them or treat the walk's `Ok` as approval of the citations.

For segmented content, [`analyze_segmented_source`](../../src/markdown.rs)
checks directive boundaries outside the individual calls, rebases errors to
the authored source, and combines results with the effective reading rate.
A per-segment hit does not replace those outer checks and combination. A
candidate that reuses the combined result must account for the same checks and
inputs instead.

Resolved renderer requests can also retain provenance, not just effective
settings. A profile-selected licensing record carries its configuration source
and span. If configuration text moves while the selected record stays the
same, retaining the old request could later report a stale authored location.
The reuse identity must include the relevant provenance, or Heine must rebuild
that provenance from the current configuration before the result is used.

The broader page unit additionally depends on effective TOC, margin-footnote,
component and citation plans; selected bibliography and style data and CSL
locale; page output availability and destination context; renderer permission,
profile, declared inputs, cache identity and actual results; resource base
path; and licensing coverage. A successful result must account for transformed
content, facts and search sections, citation and TOC updates, rebased
footnote and component plans, renderer replacements, and generated resources
and their origins. Current source paths and declarations remain necessary for
diagnostics and resource provenance. This list is an inventory to verify, not
a claim that a complete reusable Rust value or canonical key exists today.

Renderer effects need special treatment. A previously successful result does
not authorize skipping `--refresh-renderers` or bypassing permission and
current input checks. Heine must preserve the existing renderer-cache contract
and make any new resource bytes and claims available to the current output
plan. It may be simpler to reuse only the source-walk result and continue the
surrounding page work than to cache the whole page operation. Either choice
must preserve the existing check stages and diagnostic precedence.

With a whole target-catalog identity, editing an unrelated page body could
leave the narrow key unchanged, while adding, deleting, moving, or excluding
a target could invalidate many sources. With a whole asset-catalog identity,
an unrelated asset change could do the same. These broad invalidations are
conservative candidates, not proof that every catalog field is required. They
also avoid recording each lookup. A source plan could instead replay recorded
lookups against the current catalogs and potentially retain more facts, but
needs complete observations, replay logic, spans, and check ordering.

An observed-lookup identity offers a middle alternative: after a catalog
change, compare the outcomes of the lookups the previous walk actually made.
If one differs or cannot be verified, rerun the existing walk. Such a hit is
sound only if every lookup relevant to that result was recorded, including
selection and absence, and every other input still matches. This is not Tera
field tracking, nor is it evidence that instrumenting the Markdown boundary is
trivial or cheaper than broad invalidation.

For all approaches, every source still needs a trustworthy identity and the
current catalogs must be built or checked. Whole-result reuse retains facts,
requests and collector data for successful sources, plus identities; a source
plan retains its observations and source-derived facts. Sharing immutable
catalog identities across consumers could avoid hashing the same catalog once
per source, but identities must cover exactly the semantics being compared.
The storage and comparison costs depend on the representation and on source
and catalog size. No relative speed or memory advantage is established.

In an in-memory server, a candidate can retain successful units while keeping
the existing full path as the fallback. An invalidated or uncertain unit runs
the existing analysis and reports its normal diagnostic. That fallback does
not make an incomplete identity safe. For a future CLI cache, Heine would
additionally need a compatible representation and recovery policy; an
in-memory result alone does not establish one.

### Compare the Markdown candidates on edits

The following examples compare a cached successful **source walk** using a
conservative whole-catalog identity with a source plan that can replay its
recorded checks. Both are hypothetical and must retain the checks outside the
walk, current diagnostics, and full output planning. A cache hit requires a
complete verified identity, not just an unchanged source file.

**Edit another post's body.** If page targets, asset facts, relevant settings,
and this source remain unchanged, the whole-walk candidate could reuse this
source's prior result. The source-plan candidate could retain its source facts
but must still perform any current-model checks its plan requires. Both must
analyze the edited source and update affected search and rendered outputs. The
whole-walk candidate has less replay work on a hit, but still pays for input
comparison and retained results.

**Remove a linked page.** A whole page-target identity changes, so the
whole-walk candidate reruns analysis for every source depending on that
identity, including sources that never linked to the removed page. A source
plan could recheck recorded links and report the missing target at its authored
span without reparsing unrelated unchanged sources. Earlier discovery or
licensing errors can still precede that diagnostic. This narrower behavior is
only sound if the plan captured every relevant lookup and preserves check
stages and their precedence.

**Change an unrelated asset.** If a conservative whole asset identity
changes, the whole-walk candidate may rerun many sources even when none uses
that asset. A source plan could recheck only recorded asset and picture
references, including selected versions and dimensions. It must also notice
newly available assets that change an earlier selection. Neither approach
automatically establishes the right identity or proves that replay is cheaper
than the existing walk.

**Change a cited work or refresh a renderer.** A narrow source-walk result may
remain reusable if its own inputs are unchanged, but the caller must still
check and format citations against current bibliography and style data and
honor `--refresh-renderers`. A source plan must perform the same current work.
Neither may reuse stale rendered citations, renderer output, or resource claims
just because the authored Markdown is unchanged.

**Change a component-bearing source.** Both candidates must process the edited
source again. On an unchanged source, whole-walk reuse needs equivalent
component-normalized input and the other narrow inputs; replay needs the
component's recorded ranges, placement, and related observations. In either
case, early TOC and footnote planning, later renderer rebasing, component
template execution, and body-contract checks remain separate responsibilities
unless another proven reuse boundary covers them. Segmented-source boundary
checks likewise cannot disappear on a per-segment cache hit.

These cases suggest a practical trade-off: whole-walk reuse may be simpler for
ordinary body edits but invalidates broadly when a shared catalog changes. A
source plan could narrow structural invalidation at the cost of replay logic,
recorded observations, and more complex correctness reasoning. Neither saving
nor added cost has been measured, and neither candidate has a proven complete
representation. The examples therefore support comparing them, not selecting
one yet.

### Inventory a reusable Markdown source plan

The following inventory separates information the current pipeline derives
from source from the checks and work that must remain valid for the current
build. It describes a possible boundary, not fields of an existing type.

| Area                              | Source information a plan could retain                                                                                                     | Current-build work or inputs that still matter                                                                                                                                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source and segmentation           | Authored source identity, bytes or a verified identity, segment ranges, and spans in the original source.                                  | Current declared content, segment boundaries, Markdown options, and the checks that reject unsupported directives in segments.                                                               |
| Word and search facts             | Counted text or derived word count, source text sections, and their heading relationships.                                                 | Reading rate, search selection, heading anchors, and picture-derived search text must match the current configuration and checked inputs.                                                    |
| Links and images                  | Authored destinations, event kinds, and source spans, including locale-qualified references.                                               | Resolve against the current page and asset catalogs and build mode; preserve missing-target, draft, path, and locale diagnostics.                                                            |
| Pictures                          | Authored directive, options, relevant spans, and source-derived alternative text.                                                          | Recheck directive validity and asset references against current inputs, including dimensions and any resulting search text.                                                                  |
| Math and code fences              | Math expressions and placement, fence language, and their spans.                                                                           | Apply current MathML and highlighting configuration and checks; do not assume a previously successful expression or language remains accepted after its settings change.                     |
| Ordinary footnotes                | Source-derived references, definitions, and the structural information needed to diagnose unresolved references.                           | Respect current Markdown extensions and segmented-source rules. Do not silently lose the source span or change which authored problem is reported.                                           |
| TOC and margin footnotes          | Source structure and authored spans used to construct the relevant plans.                                                                  | Apply the page's current source selection, levels, footnote mode, labels, and configuration; retain the existing shared-planning behavior where applicable.                                  |
| Citations                         | Citation groups, keys, locators, heading relationships, and source spans in authored order.                                                | Check the current effective selection, bibliography, style, locale, and sequence; format the page and retain source-located errors.                                                          |
| `:::component`                    | Component calls, arguments, body ranges, normalization, placement facts, and authored spans.                                               | Preserve component and segment restrictions; coordinate with current TOC, footnote, citation, and renderer replacement plans. Template and body-contract checks still belong to rendering.   |
| `:::render` and named inputs      | Parsed requests, options, body or named-input references, and source spans.                                                                | Check current profiles, named sources, permissions, output kind, page and licensing rules; run or reuse renderer results under their existing cache and refresh contract.                    |
| Replacement and generated output  | Enough original relationships to reconstruct how source spans relate to requests and component or source-aware plans.                      | Rebase plans against actual current replacements; check overlaps, collect resource bytes and claims, and preserve output ownership and licensing.                                            |

The current [`MarkdownAnalysis`](../../src/markdown/analysis.rs) contains
facts, search sections, and *resolved* renderer requests. It is not this
source plan: link and picture checks have already happened, and the citation
collector and several surrounding plans are separate. Retaining that result
alone would not supply the observations needed to recheck a changed site.

The inventory suggests a coherent responsibility: describe what one authored
Markdown source contains, with spans, while leaving site resolution and
renderer execution at their existing boundaries. It does not establish that
every current check can be replayed cheaply from a compact representation.
Some information, such as text across event boundaries or component-normalized
offsets, may require retained text or structural state. TOC and margin
footnotes are also planned earlier in the loader, so the ownership and lifetime
of a shared plan would need an explicit design rather than a second independent
cache.

The most important unknown is whether one parser walk can produce this plan
without materially complicating the ordinary full-build path or keeping large
event streams alive. Replaying observations must also preserve checked failure
behavior and useful locations; the ordering of independent diagnostics remains
an explicit decision. Before implementation, compare an ordinary source, a
segmented source, and a source combining a component with other allowed
features, including a subsequent change that invalidates an unchanged source.
The comparison must establish both complete replay and a plausible saving. If
that needs a general event log or expensive duplicate traversal, prefer the
existing straightforward full analysis until a better boundary is shown.

### Work through a combined source

The fixture [`citations_keep_heading_component_and_margin_plans_after_renderer_output`](../../tests/fixture_site/citations.rs)
already builds a page with a renderer request, a cited heading, a block
component containing a citation and margin-footnote reference, and a footnote
definition outside the component. The request is outside the component because
renderer requests inside a component body are not supported. The page selects
the same source for its TOC, margin footnotes, and citations. The fixture
checks the resulting heading, renderer output, component wrapper, and citation
links.

For this design trace, add `[Setup](page:guide/setup)` inside that component
body and create the target page. This link is an illustrative addition to the
fixture, not behavior the existing test itself establishes. A first full build
succeeds. The author then removes only the target's `.page` declaration and
leaves the combined Markdown and its settings unchanged.

For a source-plan candidate, the first build would have to retain enough
information from the combined source to identify the component and its
normalized body, the page link and its authored span, the citation groups and
heading relationship, the margin-footnote structure, and the renderer request
and its location. It must preserve the ordering and offset relationships among
them. These records can describe selected constructs; the example does not by
itself require retaining every Markdown parser event.

On the second build, assume discovery, asset and licensing checks, and other
earlier checks succeed. Replaying the link observation against the new page
catalog must reject the missing target at the link in the original Markdown,
even though that link lies inside a normalized component body. If an earlier
check fails, including one caused by the orphaned target content becoming an
asset, the incremental build must instead match the clean build's applicable
failure behavior. It must not reuse the previous successful check to publish a
new generation.

If the target is restored, a later build must recheck against that restored
catalog. A cached component plan must still agree with the current TOC and
margin-footnote plans. If renderer output changes, Heine must apply current
replacement offsets and rebase the affected plans before rendering. It must
then render the component with its current context and enforce the body
contract. Citation formatting and bibliography placement must likewise use the
current page plan. Reusing source observations does not make any of these
site-dependent results reusable automatically.

One possible source-plan direction retains typed
observations and authored spans, and keeps current-model checks and renderer
work outside it. Source order alone is insufficient for replay: renderer
requests are parsed and checked before the Markdown event walk, and some
loader checks run earlier still. The candidate must preserve the applicable
check stages and their error precedence, not just event order. The example also
exposes the main design risk:
TOC and margin-footnote planning already occur earlier, while citations and
renderer replacements interact with the later walk and its offsets. The plan's
owner, construction point, and ability to reconstruct the same checks without
a second Markdown parse are not yet established. Nor does this one example
prove completeness for segmented sources or every event type.

If pursuing this option, prefer a bounded responsibility over a general event
log. First compare it with whole successful-analysis reuse and specify how the
existing early and later consumers would share a plan. If it cannot preserve
diagnostics and full-build behavior without disproportionate cost, leave
analysis unchanged and reconsider the boundary.

### Ownership and construction of a source plan

The current pipeline has two relevant stages. While loading a page,
[`load_page`](../../src/site/loader.rs) selects declared sources and settings.
It plans a TOC and margin footnotes early, sharing a parser walk when both
select the same source. Where necessary, it also prepares a component plan so
those features see normalized Markdown with authored offsets. Later,
[`Site::load`](../../src/site/mod.rs) has discovered pages and assets and
constructed locale-aware link catalogs. It then calls
[`Page::analyze_markdown`](../../src/site/page.rs), which performs the separate
facts and checked-analysis walk and handles renderer results and rebasing.
This is not currently one parser walk across both stages.

A candidate design gives a source-derived plan to the loaded page that owns
the declared Markdown, rather than to a global cache or a template. Its
identity includes the page locale, declared content name, source identity and
bytes, segmentation, and the Markdown options that affect parsing. Relevant
feature selections and settings must also be compared for any retained result
that depends on them. The plan may refer to the page's owned source by ranges;
it should not require a second owned copy merely to retain observations.
Whether a particular Rust representation can do that without awkward
lifetimes or additional copies remains open.

The earliest plausible construction point is after `load_page` has read and
identified the selected source and effective page settings, before TOC or
margin-footnote planning consumes it. A single source-facing pass could feed
the collectors that need early structural facts and record later link,
picture, citation, and renderer observations. The page would retain the
result until the later stage can resolve those observations against the
current catalogs. Sources without those early features, and segmented sources,
would also need a defined path through the same ownership boundary; the
design must not impose extra retained state on every source without evidence
that the trade-off is worthwhile.

Construction alone must not report every observed problem immediately. The
current loader deliberately defers some component errors so that the later
check retains its diagnostic order, while other component and footnote errors
are reported during early planning. A candidate must preserve the applicable
source locations and error timing, or explicitly decide and document any
change after testing it. Recording a deferred problem is different from
accepting it. Site-dependent checks still run after the current catalogs
exist; a reusable plan cannot treat their previous success as current success.

The early consumers need heading structure and, when selected, the checked
footnote graph and component-normalized ranges. The later consumer needs
source facts and spans for checked links and pictures, citation groups,
renderer requests, and the other observations in the inventory above. After
renderer execution it must reconcile current replacement offsets with TOC,
footnote, citation, and component plans. Template rendering remains responsible
for component execution and body-contract checks. A source plan should not
own those site or template results.

This construction point is a hypothesis, not a completed design. In
particular, it may force extra work before other loader errors, duplicate text
already held by the page, or require retaining enough parser state to make it
more costly than the present walks. Before choosing it, establish how the
existing collectors can consume one pass while retaining their current
semantics, and compare a source with both early features against one with
neither. Any extra complexity or cold-build cost must be proportionate to the
reuse the plan enables, including when a particular build never reuses it. If
a compact shared plan cannot meet that condition, keep the existing stage
boundaries and revisit a smaller incremental unit.

### Can the existing collectors share a walk?

The existing collectors make a shared walk plausible, but they do not prove
that the whole pipeline can move to one pass unchanged.
[`TocCollector`](../../src/markdown/toc.rs) accepts borrowed events and
retains headings and anchors. The margin-footnote collector consumes events
and spans to build a checked definition and reference graph; its existing
`plan_with_toc` already feeds both collectors from one offset-aware walk.
The citation collector observes borrowed events and spans, retaining groups and
heading information. These are focused consumers, not consumers that require
an entire event stream to remain in memory. Their current interfaces would
still need adaptation to share ownership of events and preserve their
individual error handling.

The later [`analyze_inner`](../../src/markdown/analysis.rs) does more than
forward each event. It parses and validates renderer requests before the walk,
suppresses citation recognition within those requests, and consumes picture
paragraphs as a unit for checked picture text. It also validates events
against the current renderer and link catalogs while deriving facts. An early
walk does not yet have those catalogs. A shared dispatcher would therefore
need to record the authored information needed for later checks while still
giving each collector the event sequence it expects. In particular, consuming
a picture for one purpose must not silently change what the TOC or
margin-footnote collector observes.

Search sections present another ordering requirement. Current analysis receives
the TOC's completed anchors and assigns them to sections as it walks headings.
If the TOC collector and section collector run together, the design must
establish how each section gets the same anchor, including duplicate headings
and headings outside the displayed TOC levels. Citation formatting may later
update displayed heading text without changing those authored anchors. A
shared pass must preserve both meanings and the current error locations.

`:::component` planning must precede the walk over its normalized source, so
all selected collectors see the same body and authored offsets. It also has
its own scanning and, for example, a body-heading check that invokes a parser.
Consequently, a shared document walk is not a claim that every component or
feature uses exactly one parser invocation. Renderer replacements happen
later and may require rebasing plans. Segmented sources remain a separate case:
their independently rendered segments and directive restrictions must retain
their current meaning and authored diagnostic offsets.

For a source with neither TOC nor margin footnotes, the current loader does not
need their early collectors; later analysis already walks that source. Moving
analysis earlier would not by itself remove a document walk for that common
case. Recording observations for possible future reuse would add work and
memory to a cold full build unless the implementation can keep that cost
small. For a source selecting both early features, sharing their walk with
source observation could eliminate a later analysis walk, but only if replaying
current-model checks does not require parsing again or negate the saving.

One possible source-plan implementation is an optional dispatcher that feeds
only the collectors required by the page and records typed observations for
checks that must wait for the current model. It would retain facts and spans,
not all parser events, and preserve the stages at which errors become visible.
Code inspection has not established equivalent diagnostic ordering or a
performance improvement. Compare its added work and memory with whole
successful-analysis reuse before pursuing either approach.
