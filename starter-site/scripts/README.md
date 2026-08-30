# Local performance fixture

`performance_fixture.py` creates a deterministic, untracked set of English
blog posts under `content/en/performance-posts/`. They inherit normal post
membership, so a full build exercises page loading, Markdown rendering, the
`posts` collection, pagination, taxonomies, the Atom feed, and the sitemap.

Generate the default 10,000 posts, dated daily backwards from 31 December
2020:

```bash
python3 scripts/performance_fixture.py generate
```

The default body is 200 generated words per post. Change the count, content
size, or deterministic seed when profiling a particular workload:

```bash
python3 scripts/performance_fixture.py generate --count 1000 --words 2000 --seed 42
```

Build from the starter-site root in the usual way. For a release-mode timing,
use the already-built release binary rather than `cargo run`:

```bash
../target/release/heine
```

Remove only the generated fixture files when finished:

```bash
python3 scripts/performance_fixture.py clean
```

The script never removes `public/` or any committed starter-site file. A later
full Heine build refreshes `public/` without the fixture pages.
