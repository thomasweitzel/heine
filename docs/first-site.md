# First site

Install Heine from the [download page](https://heine.rs/download/) or with
Cargo. The commands below assume `heine` is on your `PATH`. If you extracted
an executable without adding it to `PATH`, use its full path in place of
`heine` in each command.

Create a ready-to-edit site with a home page, an About page, a blog, and
search:

```sh
heine new my-site
cd my-site
heine serve
```

Choose a destination that does not already exist; its parent directory must
exist. Open the local address printed by `heine serve` to preview the site.
The server rebuilds and reloads it as you edit its inputs.

Start with the files under `content/en/`. The home page is `home.md`, the
About page is `about.md`, and the example posts are under `blog/`. Each
`.page` file selects a template and holds its page's settings. Change the site
name and navigation labels in `locales/en/site.ftl`. The generated README
explains how to add a post and where to find the templates and assets.

The generated site belongs to you: its files can be edited directly, and
Heine does not update them when you upgrade the executable. See the
[reference](reference.md#site-root-and-commands) for the command's destination
rules and the [development-server guide](development-server.md) for preview
options.

To learn how Heine's files fit together, follow the [tutorial](tutorial.md).
It builds a separate minimal site by hand and grows it into a multilingual
blog. For a larger working example, explore the [starter site](../starter-site/README.md).

## Build behavior

Run `heine` in the site directory for a full build. It checks the site before
publishing the output; `--quick` has different limits. The
[reference](reference.md#site-root-and-commands) describes both modes.
