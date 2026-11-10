# Creating a new site

## Status

This record describes the design of `heine new` and its starting site. The
existing [starter site](../../starter-site/README.md) remains the larger
example of Heine's features.

## Purpose

A person who has installed Heine should be able to create a small site and
start editing it immediately. The site should also be ready for common
publishing needs as it grows. Its default pages should stay short and useful;
they do not need to demonstrate every feature its templates support.

## Command

```sh
heine new my-site
cd my-site
heine serve
```

The command requires a destination path, creates a new site there, and prints
the next steps. It works offline and does not ask questions, run a build,
initialize Git, fetch files, or execute external renderers. The generated
files are determined by the Heine version: their relative paths and bytes do
not depend on the destination name, current time, or runtime environment.

The parent directory must already exist and the destination must not exist,
including when the destination is `.` or an empty directory. Heine reports
filesystem failures with the affected path and does not overwrite or merge
existing files. Creation must not follow a destination symlink. Claim the new
directory with one exclusive creation operation; that operation is the
authoritative existence check. Create its files without replacing existing
entries.
The implementation must account for a destination appearing while the command
runs. Existing parent directories may be reached through symlinks; Heine does
not claim protection against a concurrently hostile filesystem.

On failure, Heine attempts to remove only entries it created for this attempt,
in reverse order, using non-recursive operations. It must not recursively
remove unrecognized content. Cleanup is best effort when another process
changes the filesystem: a recorded path alone does not prove that the same
entry is still there. If Heine cannot safely identify or remove an entry, it
leaves it in place and reports what remains.

`new` does not discover a site root. Reject `--root` and build or serve options
for this command rather than ignoring them. Print the created path on its own
line and describe the next steps in words. Do not print a shell command with
quoting that may be wrong for the user's shell or destination path.
Interruption or forced termination can leave a partial site; automatic cleanup
is best effort for failures Heine handles.

## The site

The repository's top-level `new-site/` directory is an ordinary, buildable
Heine site and the canonical source for the generated files. The command embeds
its authored inputs in the executable, so an installed binary needs neither the
repository nor a network connection. The directory does not select or contain
a kit. Templates, configuration, messages, styles, scripts, and content belong
to the generated site and can be edited directly. Include a `.gitignore` for
`public/` and `.heine/`; the command does not initialize a repository.

A generated site is an independent snapshot of the inputs shipped with that
Heine release. Heine does not synchronize or upgrade its files afterward.
Authors maintain their own copies and use the changelog to follow any future
configuration or template migrations.

The initial navigation leads to a home page, About, a blog with two short posts,
and search. The profile illustration appears naturally in the layout. Sample
identity and prose should make replacement straightforward, without implying a
real author or publication history. There is one English locale; reader-facing
interface text lives in Fluent. Locale-aware formatting remains available, and
the site can be extended with other locales without extracting template text.
It does not claim to supply translations.

The stylesheet uses system fonts and follows the system light or dark color
scheme. The site ships neither a font nor a math example. Built-in MathML is
available when an author adds mathematics; its appearance depends on the
reader's browser and available fonts. The only authored image is the supplied
profile SVG. The site also contains the CSS and JavaScript needed for its
layout and search.

## Capabilities without showcase content

Configure the blog collection, tags, search, and listing pagination so the
two posts form a working site. A second pagination page need not exist until
there are enough posts. Supply Series listing templates, page navigation, and
messages, but do not configure a Series or generate an empty Series index.
The article layout includes a direct Series navigation hook, and the README
explains how to configure a Series when the author needs one. An unconfigured
site has no empty navigation or Series links.

Enable built-in MathML and ordinary Markdown footnotes. Page templates support
a selected table of contents and Series navigation. The new site does not
include citation or margin-footnote configuration, templates, or styles.
Authors can add those features later using the relevant guides. Calls that
depend on a page or site setting are conditional, so an ordinary post builds
without those settings.

Do not invent a deployment origin merely to enable features that require an
absolute URL. The README explains that canonical URLs need an origin, and
that a sitemap and feed also require their own configuration.

## Assets and maintenance

Project-authored, copyable `new-site/` material is available under MIT;
separately declared resources retain their own licenses. This includes the
supplied `profile.svg`, whose copyright notice must identify its creator
truthfully. Preserve the applicable notices and license texts with the source
and generated site, and update the repository's licensing documentation when
the site is added. These terms cover the supplied material, not content an
author adds later. Apply Heine's normal asset licensing rules to the
illustration, CSS, and JavaScript. Do not alter the SVG merely to make it
match the site's palette without a separate reason.
Place the MIT text in `LICENSES/MIT.txt`, and explain which supplied files it
covers in the site README or a scoped `LICENSING.md`. Do not add a root
`LICENSE` that appears to set terms for content the author adds later.

The search client is maintained in
`starter-site/kits/starter/assets/js/search.js`. `new-site/` contains a full
copy with its existing ownership comment. A test checks that the files match;
updates copy the whole file. No synchronization script or runtime kit
dependency is needed. The new site owns its other files. Its CSS can adapt
tested starter-site patterns, particularly for search, without requiring the
two stylesheets or templates to remain identical. The copied client's MIT
terms and any separately declared exceptions remain part of the site's
licensing information.

The embedded files come from a deliberate, reviewable inventory of authored
inputs, including dotfiles and license declarations. Directory traversal alone
must not silently include unrelated local files, caches, or generated output.
The inventory and embedding process must work when building from a crate
package without Git or a repository checkout. Preserve file bytes; the command
does not substitute site names or timestamps. Missing or duplicate inventory
entries are errors. A repository check compares the inventory with the intended
source files in both directions, excluding known generated directories such as
`public/` and `.heine/`; ordinary packaged builds use the inventory without
Git. Only inventoried files and their parent directories are created; empty
source directories are not preserved. Heine creates ordinary files and
directories with platform filesystem APIs and permissions. It does not copy
executable bits or other source metadata.

The generated site includes a README that identifies the editable content,
templates, assets, and Fluent messages. It explains Series setup, the site
origin and the separate configuration needed for a sitemap or feed, licensing
of the supplied material, and the independent-snapshot maintenance model.

## Implementation and acceptance checks

- Build `new-site/` directly with Heine and build a newly created site using
  the public command. The created files match the intended source files,
  including licensing declarations, without generated output or caches.
  Two destinations receive identical relative paths and bytes. A repeated
  invocation fails without changing the existing site.
- Inspect the crate's package contents and test a binary built from the package
  outside the repository. It creates a site that builds, including dotfiles
  and license declarations. The embedded file set contains exactly the
  inventoried files and does not require Git at build or runtime.
- Keep project-authored new-site text at LF line endings across checkouts;
  choose and verify suitable repository attributes and embedding rules.
- Check existing destinations, missing or read-only parents, trailing
  separators, symlinks, platform-specific invalid names, case-insensitive
  collisions, creation failures, and cleanup failures without overwriting user
  files. Diagnostics identify the path and underlying failure.
- Exercise templates with temporary fixture content for pagination, tags,
  Series, table of contents, ordinary footnotes, and MathML. The shipped posts
  remain free of artificial demonstrations. Check both enabled and absent
  settings, including an ordinary post with no table of contents or Series.
- Check the generated README's configuration examples against a copy of the
  site, including Series setup and its generated index.
- Check responsive layout, focus visibility, and light and dark contrast in a
  browser at mobile and desktop sizes. Check the prepared presentations with
  temporary content, including print behavior. Check the search template
  against the client contract and exercise a search in the browser.
- Document the command and its destination rules. Include the site README
  topics described above: editable files, localization, Series setup, site
  origin, sitemap and feed configuration, licensing, and snapshot maintenance.
  Update the CLI reference and changelog when the feature is implemented.

## Boundaries

This command creates one project-owned starting site. It is not a kit
installer, a remote template catalog, an interactive site builder, or a general
content-generation command. The existing starter site remains a separate,
more comprehensive example.
