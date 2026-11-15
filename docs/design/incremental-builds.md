# Dependency-aware incremental builds

## Status

Exploratory. This record proposes a shared incremental build model for the
development server and command-line builds. It does not define a command,
configuration setting, cache format, or implementation commitment. The
[roadmap](../roadmap.md) identifies dependency-aware builds as later work.
It specifies a possible in-memory feasibility test for analysis reuse; that
test has not been authorized or started.

The preferred direction is a conservative Heine-owned build-state contract
using the existing resolved site relationships. Whether Heine should implement
the reuse engine itself or use Salsa remains open. Neither approach has been
integrated or tested against Heine's pipeline. A graph crate alone would not
establish correct dependencies or publication.

## Purpose and boundaries

Editing a page should not require repeating work that cannot have changed the
result. A subsequent CLI build should be able to use the same dependency
semantics as `heine serve`, even though the server can keep state in memory and
the CLI may need to load it from disk.

An incremental build must preserve Heine's checked relationships and complete
managed output. It must not ask the author to decide whether an edit affects a
collection, a link, or a generated file. When the available state cannot prove
reuse correct, Heine performs the necessary broader work, up to a full build.

The goal is not a general plugin or build system, remote execution, shared
network caches, parallel rendering, or processing external CSS and JavaScript
graphs. External renderers keep their existing explicit permission and cache
contracts. Streaming output is a separate [roadmap](../roadmap.md) item.

## Current pipeline

The [architecture overview](../architecture.md) describes the public build
guarantees. The current code gives this proposal the following boundaries:

1. [`build_at_with_publication_in_runtime`](../../src/lib.rs) loads site
   configuration, the selected kit, renderer runtime, data, deployment files,
   and license texts for each build.
2. [`Site::load`](../../src/site/mod.rs) discovers localized content, resolves
   pages and collections, expands pagination, resolves taxonomies and Series,
   discovers assets, and analyzes declared Markdown against current page and
   asset catalogs. It then resolves feeds and search, renderer resources, and
   output claims. These are domain-specific structures, not one complete
   output-dependency graph.
3. [`Site::render`](../../src/site/render.rs) validates template references
   and renders tasks and generated resources. [`TemplateResources` and
   `RenderContext`](../../src/template.rs) expose resolved views and checked
   functions to Tera. The render context is serialized; Heine does not thereby
   record every field that a template reads.
4. [`OutputWriter`](../../src/output/writer.rs) stages the complete full-build
   tree and publishes it. Existing-output replacement uses two renames, with
   restoration attempted on failure; it is not an atomic directory swap and
   does not guarantee crash recovery. Quick builds instead update in place.
5. [`serve`](../../src/server/mod.rs) currently makes complete builds. Its
   [watcher](../../src/server/watch.rs) starts before the initial build and
   publishes a new server generation after a rebuild installs output, including
   when subsequent backup cleanup fails. An error during the initial build
   prevents the server from starting. The server keeps the renderer runtime
   for subsequent builds, not a reusable resolved Site.

Quick-build tasks carry `latest_input` timestamps, with some wider inputs and
collection members propagated into those timestamps. The timestamp does not
identify all contributing inputs or record removed files. Quick builds remain
intentionally incomplete; they do not update every generated resource and can
leave stale files. This proposal must not silently present that mechanism as a
complete incremental dependency model.

The [renderer cache](../../src/renderer/cache.rs) is an existing, separate
example of persistent reuse. It keys results by the declared renderer identity,
request, and inputs and checks cached bytes. `--refresh-renderers` deliberately
changes reuse behavior. A site-build cache must respect that contract rather
than treating a renderer's output as determined by the Markdown file alone.

## Required behavior

For the same stable site inputs, build options, and permitted renderer
behavior, an incremental full build must produce the same managed paths and
file bytes as a clean full build. It must not hide an error because a
previously successful result was reused. A checked result may be reused only
when every input relevant to that check remains valid; otherwise the check
must run again. Preserve the existing precedence of checks and useful authored
locations by default. A deliberate change to the order or presentation of
independent errors needs separate evaluation and documentation; no build may
silently accept input that a full build rejects.

This is an equivalence target for Heine's existing contracts. It does not make
raw HTML, external URLs, CSS or JavaScript references checked, or make an
external renderer deterministic. Heine does not claim protection against a
hostile process changing files concurrently. Handling ordinary edits during a
build, watcher event loss, and input changes while reading must be defined and
tested rather than assumed away.

Reuse is an optimization. Missing, stale, corrupt, incompatible, or incomplete
state must not authorize reuse that cannot be justified. A cold build and a
conservative full rebuild remain available. The design must account for a
changed or missing output file as well as changed inputs; the output directory
is not automatically trusted as a cache of prior bytes.

## One model, two lifetimes

The server and CLI need the same meanings for identities, dependencies,
invalidation, and a successful generation. They need not use the same storage
representation. A server can retain build state in memory; a later CLI process
can load a compatible persistent representation. A cold or rejected cache
starts from a full build. Neither mode may rely solely on watcher events.

An initial milestone may implement reuse only in the running server and keep
its state in memory. The CLI can continue to perform full builds until durable
reuse is justified. Both must still follow the same correctness rules, and the
server must handle changes to its published output, failed publication, missed
events, and other builds that use the same destination. In-memory state alone
does not resolve those concerns.

The model needs to distinguish:

- **Inputs:** discovered files and relevant non-file settings, including their
  identities and a reliable way to detect changes. Directory membership is an
  input where adding or removing a file changes discovery. An absent file,
  directory, or other failed lookup is also an input when its later appearance
  can change a selection, fallback, validation, or output claim.
- **Resolved facts:** page identities, memberships, routes, assets, localized
  values, and other checked relationships. Existing domain types retain their
  meaning and validation; an incremental layer does not replace them.
- **Reusable results:** work whose dependencies are known, such as a checked
  analysis or rendered output, with enough identity and state to justify reuse.
- **Output ownership:** the complete new set of checked claims, including
  paths that existed only in the previous generation.
- **Published state:** the last successful generation, distinct from a
  candidate build and from individual renderer-cache entries.

This list describes responsibilities, not a required generic graph or set of
Rust types. A dependency is a relationship between a computation and a value it
actually needs. A domain link between two pages is not automatically the full
dependency set of the template that presents either page.

## Representative changes

The table traces consequences that a design must account for. It does not claim
that Heine currently records all of them or that every listed output changes
for every site.

| Change                                                         | Existing information                                                                                                                                        | Work an incremental build must account for                                                                                                                                                     |
|----------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Markdown body edit                                             | A page declares its content; analysis derives facts and checked links.                                                                                      | Recheck the changed source and affected relationships; update page output, search, and other consumers of changed facts. A template can render the content or consume its facts.               |
| Page title, date, taxonomy, collection, or route edit          | Page metadata, memberships, pagination, taxonomies, Series, and output claims are resolved during loading.                                                  | Recheck the model and affected pages and generated resources. Ordering, neighbors, links, feeds, search, sitemap, and collisions can change even when their own files did not.                 |
| Template, include, component, or kit template edit             | Heine resolves templates and validates references at rendering.                                                                                             | Identify affected renders only if the entire template dependency is known; otherwise render broadly. Source-aware call-count and placement checks must still run when required.                |
| Asset, sidecar, or license edit                                | Asset and license catalogs resolve copied assets, metadata, and licensing. Quick builds currently propagate present asset timestamps broadly.               | Recheck asset identity, dimensions, integrity, versioned URLs, licensing, claims, copies, and consumers as applicable. A changed asset can affect page HTML without changing its Markdown.     |
| Data or Fluent edit                                            | Global and locale-local data and messages are loaded before rendering.                                                                                      | Recompute affected presentations. Without complete read tracking, invalidate a sufficiently broad scope, including diagnostics for messages or data consumed by templates.                     |
| Add, remove, or rename a page or asset                         | Current discovery and output claims describe the new site.                                                                                                  | Compare discovery and output inventories with the previous successful generation; recheck broken relationships and remove stale managed output through complete publication.                   |
| Bibliography, CSL style, or renderer input edit                | Citation inputs have build-scoped sharing; renderer requests have their own checked inputs and result cache.                                                | Recheck consumers and renderer behavior, including `--refresh-renderers`, permissions, result kind, and resources. Do not infer external program determinism beyond its existing contract.     |
| Configuration, locale, output path, or publication-mode change | Configuration and publication mode establish the model and destinations.                                                                                    | Re-resolve all affected boundaries. Begin with a full rebuild where a narrower rule is not proven, and do not reuse state across incompatible output roots or modes.                           |
| Failed build followed by a correction                          | Before publication, a failed build normally leaves the previous output in place. A cleanup error can instead be reported after the new output is published. | The next attempt must establish which generation is present, recheck the correction and any other changes, and not mistake a partial or uncertain result for a reusable successful generation. |

## Design direction

The [pipeline investigation](incremental-build-investigation.md) compares
candidate boundaries against Heine's current code and representative
sites. None is ready for implementation or established to improve performance.

- **Render reuse** could avoid template and component rendering, but Tera can
  read broad page views and checked resources dynamically. Complete tracking,
  retained artifacts, and full staging add cost. Coarse observations can
  invalidate many outputs for an edit that does not change their visible HTML.
- **Bibliography input reuse** has a smaller, already shared domain boundary.
  It could avoid reparsing unchanged inputs in a running server, but benefits
  only sites with bibliographies and retains potentially large data. Durable
  reuse by separate CLI invocations has not been established.
- **Whole successful Markdown analysis reuse** could retain the complete
  checked result when its source and a conservative identity for every relevant
  input remain equal. This avoids a separate replay path, but the current
  `MarkdownAnalysis` value is only part of the page's analysis work. The
  [input inventory](incremental-build-investigation.md#inventory-whole-successful-analysis-reuse)
  distinguishes reusing the source walk from reusing the broader page operation;
  neither has an established complete key or retained result. A walk result
  must also preserve deferred citation problems, segmented-source checks, and
  current renderer-request diagnostic provenance.
- **Observed-lookup analysis reuse** is a variant of whole successful-walk
  reuse. It records page and asset lookups made by the walk, including
  relevant absent lookups, and rechecks their outcomes against the current
  model. A mismatch reruns the existing walk rather than replaying its checks.
  This could avoid invalidation for unrelated catalog changes, but completeness,
  lookup cost, and all other inputs still need to be established.
- **Markdown source-plan reuse** could apply to more sites. A focused plan of
  authored observations and spans could let Heine recheck current relationships
  without repeating the source-analysis walk. It must preserve the current
  interaction of early TOC and margin-footnote planning with later link,
  citation, renderer, and `:::component` analysis. A shared walk and compact
  representation are plausible, not proven.

Compare whole-catalog and observed-lookup identities for successful-analysis
reuse with a focused source plan before choosing a boundary. Keep full
discovery, current-model checks, output planning, and publication authoritative.
A candidate can reuse a proven successful result, rerun existing checks, or
replay equivalent checks from a source plan;
it must establish equivalence rather than assume one mechanism is sufficient.
When an input or cached result cannot be verified, run the existing path to
produce the result or diagnostic. This fallback is safe only if the reuse key
accounts for every relevant input: an omitted dependency can create a false
cache hit without triggering fallback.

Do not adopt a boundary, build a cache, or choose an engine until correctness,
useful work avoided, and proportionate cold-build and memory costs are
established. Prefer a focused domain model over a general event log when it
provides the needed evidence.

## Proposed feasibility test

The next useful question is whether Heine can reuse a **complete successful
source-walk result** after an ordinary edit using a conservative identity,
without duplicating the existing checks or weakening their diagnostics. This
would test one candidate, not implement an incremental build. The source-plan
alternative remains open if whole-walk invalidation proves too broad.
The observed-lookup variant is another option to compare; this test does not
establish that its observations are complete.

A later, focused test could retain narrow walk results in memory across at
least two model loads in the same process. It would rebuild the current model
and use the existing analysis path whenever a relevant identity changes or
cannot be established. It would not persist state, reuse rendered output,
alter `serve` or `--quick`, or introduce a general dependency engine. The test
must explicitly count both reuse and fallback; matching a clean build while
always falling back does not establish useful reuse.

The source identity and the walk must refer to the same buffer that was
actually analyzed. For components and segmented sources, the test must also
account for normalized input, segment ranges, and their relationship to the
authored bytes. This prevents a separate-read mismatch; it does not by itself
provide a consistent snapshot across files or detect missed watcher events.

The initial test should cover these cases at the relevant analysis boundary:

1. Edit one of two Markdown bodies without changing page targets or asset
   facts. The changed source runs again; the other source is eligible for a
   hit only if all of its relevant inputs match. Compare facts, search
   sections, requests, and citation collector results with a clean analysis.
2. Remove a target linked from the unchanged source. A changed conservative
   target identity causes the existing path to run. Compare the error and
   authored location with a clean build, allowing an earlier discovery or
   licensing error to take precedence where applicable.
3. Include a `:::component` source whose unchanged body contains a checked
   link. Confirm its authored span and surrounding checks remain correct on
   a hit and after a target change forces fallback.
4. Change whether an unchanged page is selected for search. Analysis must
   produce the current search sections without an error, rather than reuse a
   result produced with the old selection.

A walk can return `Ok` while its citation collector contains problems that
the page reports later. The candidate must either exclude such a result from
reuse or retain the problems and let the current caller report them. It must
not treat the walk's `Ok` as a successful page analysis. Failed work must not
become a reusable success.

Before considering an implementation complete, extend the evidence to asset
and dimension changes, Markdown settings, renderer profiles and refresh,
current renderer-request provenance, TOC, citations and margin
footnotes, segmented-source boundaries and rebasing, incompatible identities,
and competing errors. These cases check the wider contract; they need not all
be built into the first focused test.

Use existing fixture patterns and the real analysis path where practical.
Keep any test-only state narrowly scoped and remove it if it cannot express the
candidate without complicating production code. The test must not infer safety
from a successful hit alone: the proposed identity still needs an explicit
review against every input and output in the
[inventory](incremental-build-investigation.md#inventory-whole-successful-analysis-reuse).
The existing analysis-walk probe can show that a walk was avoided, but it does
not count reuse decisions; the test needs evidence of a hit and of fallback.

Before investing in a prototype, consider a focused phase breakdown on a small
site and the existing large fixture to establish how much work analysis could
save. The existing walk probe counts walks; it does not measure their duration.
This is a proposed evidence gate, not a measurement performed here.

Before turning a positive result into a product decision, compare the work and
memory required to construct and retain identities and results with a clean
full build on representative sites. The test can establish correctness for its
cases and demonstrate a hit; it cannot prove complete dependency coverage or a
performance improvement. A separate decision is required before building such
a test or changing production behavior.

Engine selection, render reuse, persistent CLI state, and publication and
cache recovery remain later design decisions. A test of source-walk reuse does
not settle or implement them.

## Decisions already made

- Incremental and clean full builds must agree on managed output and checked
  success or failure for the same stable inputs and permitted renderer behavior.
  Uncertain state falls back to the necessary broader work.
- The server and CLI share dependency and correctness semantics. Their storage
  and the work they can reuse may differ; neither relies solely on watcher
  events or modification times.
- The output domain owns complete claims, staging, and publication. Reuse does
  not change the existing two-rename replacement or renderer-cache contracts.
- Heine's source, model, render, and output boundaries remain distinct. A
  resolved site relationship is not by itself a complete render dependency.
- This work does not make quick builds complete, introduce a plugin system,
  fetch remote content, or promise parallel or streaming execution.

## Reuse and invalidation

Separate the questions of detecting change, deciding what must be recomputed,
executing work, and publishing output. [Build Systems à la Carte](https://www.microsoft.com/en-us/research/wp-content/uploads/2018/03/build-systems-5ab0f42d0f937.pdf)
provides a useful comparison of build-system scheduling and rebuilding choices.

Start with coarse dependencies where a fine-grained relationship cannot be
established. For example, all renders consuming a broad template view may need
invalidation when that view changes. Only narrow that scope when the code can
establish which values a render depends on, including conditional calls,
template inheritance, and includes. If dynamic dependencies are recorded,
recomputing a result must replace its old dependencies with the new ones.
Missing or uncertain dependencies require broader work, not assumed reuse.

Fingerprints are a candidate for comparing persisted inputs and results. They
need canonical identity and unambiguous inputs, including build options,
publication mode, output root, Heine/cache compatibility, configuration, and
any relevant tool or renderer identity. Modification times alone do not prove
content equality. Heine already uses `sha2` and `notify`; a new hashing or
watching dependency needs its own justification. Neither crate supplies the
dependency model. Hashing all source bytes, retaining results, and checking
prior output have I/O and memory costs that must be assessed.

The initial strategy for template dependencies is a design decision. Tera
receives serialized context values and can call Heine functions conditionally.
Do not claim that the existing domain graph or a static list of template names
captures every read. Broad invalidation, explicit function-level recording, and
finer tracking are alternatives to evaluate. An unused conditional branch can
become active when its condition changes; that condition is itself a dependency.

## Comparison: Heine-owned engine and Salsa

Both candidates must satisfy the same correctness, publication, diagnostics,
and two-lifetime requirements. The comparison is architectural, not a result of
an implementation or benchmark.

| Question                    | Heine-owned reuse engine                                                                                                         | Salsa                                                                                                                                                                                 |
|-----------------------------|----------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Integration                 | Can start at existing checked domain and render-task boundaries. Heine must implement and maintain invalidation and reuse rules. | Provides tracked inputs, memoized computations, and dynamic dependency tracking. Heine would adapt suitable operations and data to its query model.                                   |
| Dependency completeness     | Explicit rules and conservative scopes are understandable, but every rule is Heine's responsibility.                             | Tracks reads made through Salsa queries; filesystem reads, Tera behavior, renderer effects, and output publication do not become tracked merely by adopting Salsa.                    |
| In-memory server            | State can be tailored to Heine's synchronous pipeline.                                                                           | Revisions and memoized queries are a natural fit if Heine's computations can use them without excessive adaptation.                                                                   |
| Separate CLI invocations    | Requires a versioned persistent format, compatibility policy, and recovery behavior.                                             | Salsa has opt-in persistence APIs, including persistable tracked queries. That does not yet establish that Heine's values, cache compatibility, and failure recovery fit those APIs.  |
| Diagnostics and determinism | Heine owns ordering, source locations, and replay or recomputation of checks.                                                    | Accumulators and query results offer mechanisms, but Heine must establish equivalent diagnostics and deterministic behavior across cached and fresh work.                             |
| Complexity and cost         | Smaller initial integration is plausible, but custom machinery can grow and hide missed edges.                                   | Avoids writing a general memoization engine, but may require substantial data-model and lifetime changes and adds dependencies. Its own memory and persistence costs need evaluation. |

### Worked comparison

Consider a page `guide/install` whose Markdown links to `guide/setup`, and a
listing template that reads a collection containing `guide/install`. An author
first edits the page body, then removes `guide/setup`. The current full build
loads the page inputs, resolves collections and the page catalog, and analyzes
Markdown against that catalog before rendering. The template boundary can
expose both the page and collection views.

With a Heine-owned engine, the candidate state could record an analysis result
for `guide/install`, dependent on its authored input, Markdown settings, and a
sufficiently broad page-catalog value. It could also record a listing render
dependent on the collection view and templates. Editing the body invalidates
the analysis and any render whose view or content may change. Removing the
target changes the page catalog, so the link check must run again and report
the broken reference at its authored location. A coarse page-catalog dependency
may recheck more sources than necessary, but it is sound if all relevant
catalog changes invalidate it.

With Salsa, candidate tracked inputs and queries could represent the page
source, page catalog, analysis, collection view, and render. If the analysis
query reads the catalog through Salsa, a changed catalog can invalidate that
query. Heine would still need to supply current filesystem and discovery state
as tracked input, express errors and source spans, and determine how the Tera
render observes its collection and other dependencies. A Tera call that reads
an ordinary serialized value does not automatically register a Salsa query.
If that boundary cannot be tracked completely, the Salsa version also needs a
coarse dependency or a broader rebuild. The missing link must fail even when a
previous output is available in either design.

Now consider an unchanged second invocation in a new CLI process. The
Heine-owned candidate must load compatible prior fingerprints, dependencies,
and any retained results it needs. The Salsa candidate must reestablish its
input identities and load compatible persisted query state; persistence of a
query result alone does not establish that the current filesystem or output
matches it. Both must handle a missing or modified output and publish a complete
tree, or perform the required work again. These are integration requirements,
not capabilities established by either candidate today.

[Salsa](https://docs.rs/salsa/latest/salsa/) is designed for incremental,
on-demand computations. Its [persistence API](https://docs.rs/salsa/latest/src/salsa/database.rs.html)
and [persistable query option](https://docs.rs/salsa-macros/latest/src/salsa_macros/lib.rs.html)
make it a serious candidate for both lifetimes. Its published package describes
the framework as experimental. Availability of an API is not evidence that it
meets Heine's Rust version, diagnostic, storage, or recovery requirements;
verify those for an identified release before choosing it.

Heine's release profile uses `panic = "abort"`. Salsa documents [panic-based
cancellation](https://salsa-rs.github.io/salsa/tuning.html#cancellation) and
[default cycle panics](https://salsa-rs.github.io/salsa/cycles.html). Neither
fact establishes that a proposed synchronous Heine integration would trigger
these paths. Before choosing Salsa, establish which paths are reachable, how
they behave in a release build, and whether they preserve the server's failure
handling without relying on panic unwinding. Salsa also documents that memoized
values are unbounded by default and that LRU does not remove query keys or
dependency metadata. Compare memory growth and reclamation for both candidates,
including repeated page additions, deletions, and renames during a long server
session.

[`petgraph`](https://docs.rs/petgraph/latest/petgraph/) provides graph types
and algorithms, not incremental build correctness. Consider it only if the
chosen model needs graph operations that are clearer than Heine's typed maps.
Other small incremental or invalidation libraries need the same assessment of
maintenance, compatibility, persistence, and total integration cost. Do not
add a dependency just to avoid a short traversal.

## Publication and failure

The output domain continues to own output paths, collision checks, staged
writes, and publication. Reusing work cannot omit current claims, leave a
removed file in the new managed tree, overwrite an unowned path, or change
the existing guarantees for failure before replacement. A previous output
file may have been modified or removed outside Heine; establish how its bytes
are verified or retain a separate trusted artifact before reusing it.

Do not assume a hard link is safe: the old and staged paths would share an
inode, so later writes can change both. Copying, a verified immutable artifact
store, and platform capabilities need comparison; no particular reuse method
is selected here. Building a complete staging tree can still cost O(output
bytes), even if most rendering is reused.

Advance durable incremental state only when it corresponds to a successfully
published generation. A failure after publication, a crash between output
and cache updates, an interrupted cache write, or a concurrent build can leave
their states out of step. The implementation must detect or safely recover from
such cases without weakening existing publication behavior. The exact commit,
locking, and recovery mechanism is unresolved; a full rebuild is the fallback
when correspondence cannot be established.

Replacement can succeed and backup cleanup can then return
`PublishedWithBackup`. During a rebuild, the running server updates its
generation and settings for the new output while reporting the cleanup error.
During the initial build, the error prevents the server from starting. A future
incremental model must likewise distinguish publication outcome from subsequent
cleanup when deciding which state can be reused.

## Prior art and consequences

- [Eleventy's incremental-build documentation](https://www.11ty.dev/docs/usage/incremental/)
  describes tracked template relationships and situations that require a full
  rebuild. Its details are version-dependent; it illustrates conservative
  fallback, not a ready-made Heine contract.
- [Hugo's build configuration](https://gohugo.io/configuration/build/) notes
  that partial server builds can retain old build-statistics values until a
  full build or restart. Heine should not silently carry stale derived state
  under a complete-build guarantee.
- [Rust's incremental compilation guide](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation.html)
  explains rechecking dependencies and retaining a result when its computed
  value has not changed. Such early cutoff is a possible later refinement, not
  a prerequisite for Heine's first correct implementation.
- [Detecting Incorrect Build Rules](https://www.cl.cam.ac.uk/~acr31/pubs/licker-buildrules.pdf)
  motivates exercising dependency behavior through changes and rebuilds.
  Heine should compare incremental results with clean full builds rather than
  test only its internal invalidation decisions.

## Evidence and decisions needed before implementation

1. Define the complete reusable unit and conservative inputs for whole
   successful Markdown analysis, and compare whole-catalog and observed-lookup
   identities with a compact source plan.
   Include ordinary and segmented sources and `:::component`, along with
   diagnostics, extra parsing, cold-build work, and retained memory. Select
   no candidate without evidence that it provides worthwhile reuse.
2. For a viable boundary, specify the dependencies and invalidation rules for
   the affected source, asset, component, renderer, citation, and localization
   cases. Explain which checks always run and when uncertainty falls back to a
   full build.
3. Compare a concrete Heine-owned state model and Salsa for that same contract,
   including diagnostic behavior, deletion, failed rebuilds, memory lifecycle,
   separate CLI invocations, dependency compatibility, and release-profile
   panic behavior. Do not choose an engine merely because it provides a graph.
4. Specify change detection for each supported process lifetime, including
   missed watcher events, inputs changed during a build, incompatible cache
   state, and output edited or removed outside Heine. An in-memory first
   milestone need not implement persistent CLI state.
5. Specify publication and cache recovery, including concurrent invocations,
   interrupted builds, and cleanup failures. Preserve the existing two-rename
   publication limitations rather than claiming stronger atomicity.
6. Decide the user-facing relationship to `--quick` and `serve` only after the
   guarantees and costs are known. An incomplete quick update must not be
   relabeled as a complete incremental build.

## Verification before release

Use the same scenario matrix for in-memory state and, when implemented,
persisted state. For each case, build a fixture, change inputs, run the
incremental path, and compare its complete managed output with a clean full
build from the same final inputs.
Compare success or failure and the essential authored diagnostic locations as
well as bytes and output paths. Include unchanged inputs, body and metadata
edits, conditional template dependencies, assets, shared data and messages,
citations and renderers, additions, deletions, renames, route collisions,
draft modes, output tampering, missing or corrupt cache, incompatible versions,
failed builds followed by fixes, and interrupted publication or cache updates.
Include sources with competing errors, such as a bad renderer request and a
broken link, to verify the selected diagnostic-order policy across check stages.
Exercise cache reuse and fallback explicitly so matching output cannot pass
merely because every scenario performed a full build.

Assess complexity, extra filesystem work, memory, cache size, and the cost of
complete staged publication on representative small and large sites. These are
acceptance questions, not a promise of speed or a reason to compromise checks.
For both candidates, include a long-running server session that repeatedly adds,
renames, and removes pages, as well as the existing 50,000-page fixture. Record
retained state and its reclamation rather than only the size of one rebuild.
