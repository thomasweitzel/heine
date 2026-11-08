# Development server

Use `heine serve` while writing a site. It builds the site, serves its
[generated output](glossary.md#generated-output), watches its inputs, and
reloads open HTML pages after a successful rebuild. It is a local development
server; use a production web server for TLS, compression, caching, and
conditional requests.

Run the downloaded executable:

```bash
/path/to/heine serve --root /path/to/site
```

The server performs a full build before listening. By default it listens only
on `127.0.0.1`, starts with port `1111`, and tries the next available
unprivileged port if necessary. Select an interface or starting port when
needed:

```bash
/path/to/heine serve --root /path/to/site --host 0.0.0.0 --port 3000
```

Ports below `1024` are rejected. `--host 0.0.0.0` makes the server reachable
from the local network, so use it only on a network you trust. The
[reference](reference.md#site-root-and-commands) lists every command option.

To preview unfinished [draft](drafts.md) pages declared with `draft = true`,
add `--drafts`:

```bash
/path/to/heine serve --root /path/to/site --drafts
```

This includes drafts in the same complete resolved site as published pages, so
their links, collections, feeds, indexes, and navigation are truthful for the
preview. It writes the ordinary configured output directory. Run a normal full
`heine` build before deploying from that directory, which replaces
the preview output without drafts.

## What it serves

The server reads the published `build.output_dir` from the filesystem. It does
not maintain a separate in-memory site representation. It uses the
`site.base_path` from the same successful build, so a site with
`base_path = "/guide/"` and `output_dir = "public/guide"` serves
`public/guide/blog/post.html` at
`http://127.0.0.1:1111/guide/blog/post.html`. `/guide` redirects to `/guide/`;
paths outside the configured base path return `404`.

For local browsing, a request ending in `/` serves that directory's
`index.html`; a directory request without the trailing slash redirects to the
slash form. These are development-server conveniences. They do not change
Heine's generated URL contract: `page()`, pagination, taxonomy links, and
sitemaps continue to name their literal output files, such as
`/blog/index.html`. A production host decides independently whether it offers
the same directory-index behavior.

It handles `GET` and `HEAD`, supplies ordinary content types, and supports one
`bytes` range for regular files. That allows local playback and seeking for
audio and video. Multiple or unsupported ranges receive a normal complete
response.

All development responses use `Cache-Control: no-store`, so a reload fetches
the current generated file. The server injects its reload client only into
HTML responses; it never writes that client into generated output.

## Rebuilds and reloads

The server watches `content/`, `data/`, `deploy/`, `LICENSES/`, `locales/`,
`templates/`, and `heine.toml`. It performs a full build for a detected change.
After a successful publication, it reloads connected HTML pages. A browser
that reconnects after a successful rebuild, including after a laptop wakes,
also receives a reload when its page was built from an older generation.

If a rebuild fails, Heine prints the diagnostic in the terminal, keeps the
previous successful output available, and does not reload the browser. A
watcher error identifies the watched project root and suggests saving again or
running a full build.

Full publication replaces the output tree with two renames. During the
interval in which the directory changes names, the server retries a missing
file lookup once. This keeps serving tied to the published filesystem, but it
does not promise uninterrupted responses during that replacement interval.

`--quick` does not apply to `serve`: a watched site always uses complete
rebuilds so template, configuration, relationship, rename, and deletion
changes cannot leave the preview stale.

## Output ownership

Like `heine`, `heine serve` refuses to replace a non-empty output directory
that Heine does not manage. To adopt one for the initial served build, first
confirm it may be replaced, then use:

```bash
/path/to/heine serve --root /path/to/site --clean-output
```

`--clean-output` applies only to that initial build. Read the
[build behavior](../README.md#build-behavior) section before using it.
