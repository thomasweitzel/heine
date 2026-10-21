# Deployment files

Use `deploy/` for [deployment files](glossary.md#deployment-file) that must
travel with the site at a fixed output path, such as server configuration or a
third-party verification file. It keeps those host-owned files separate from
`content/`, whose regular files are copied assets for pages and templates.

Use this guide after the [tutorial's deployment step](tutorial.md#10-prepare-the-generated-site-for-deployment)
when you need a file at a fixed output-root path.

## Use cases

Place files below `deploy/` when their destination must be a particular path
in the generated output tree:

```text
my-site/
├── deploy/
│   ├── .htaccess
│   ├── google1234567890.html
│   └── server/
│       └── headers.conf
└── public/                 # generated
    ├── .htaccess
    ├── google1234567890.html
    └── server/
        └── headers.conf
```

Use this for an Apache `.htaccess` file that supplies a Content Security
Policy, or for the HTML file used by Google Search Console site verification.
Heine copies the bytes unchanged; it neither parses the file nor knows which
web server or external service will consume it.

## Boundary

Deployment files are not copied assets. They have no asset ID, template URL,
integrity value, image dimensions, locale override, or versioned filename.
Templates cannot refer to them through `asset()`. Use `content/` for files
that a page needs to link or embed; use `deploy/` only for output-root files
whose meaning belongs to the hosting environment.

## Checks and builds

The optional directory and its nested directories must be real directories;
its nested files must be regular files, and symbolic links are rejected. Paths
must be valid UTF-8, portable output paths;
they cannot escape the output root, use platform-reserved filename components,
or collide by case or Unicode normalization. Heine checks deployment-file
claims together with pages, assets, and generated resources before writing, so
a deployment file cannot silently overwrite another output producer. The
reserved [`__heine/`](glossary.md#heine) namespace remains unavailable.

Full builds atomically publish a complete output tree. Consequently, removing
`deploy/google1234567890.html` removes its generated counterpart on the next
full build. Quick builds update changed, present deployment files, but - as with
all quick-build inputs - do not remove output for deleted or renamed files. Run
a normal build after such changes.
