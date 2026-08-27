# Deployment files

`deploy/` is an optional, output-root source domain for files that must travel
with a deployed site but are not site content. It keeps host configuration and
third-party verification files out of `content/`, where files are otherwise
public assets with locale and template semantics.

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

This is useful for an Apache `.htaccess` file that supplies a Content Security
Policy, or for the HTML file used by Google Search Console site verification.
Heine copies the bytes unchanged; it neither parses the file nor knows which
web server or external service will consume it.

## Deliberate boundary

Deployment files are not copied assets. They have no asset ID, template URL,
integrity value, image dimensions, locale override, or versioned filename.
Templates cannot refer to them through `asset()`. Use `content/` for files
that a page needs to link or embed; use `deploy/` only for output-root files
whose meaning belongs to the hosting environment.

## Validation and builds

The optional directory and its nested directories must be real directories;
its nested files must be regular files, and symbolic links are rejected. Paths
must be valid UTF-8, portable output paths;
they cannot escape the output root, use platform-reserved filename components,
or collide by case or Unicode normalization. Heine validates deployment-file
claims together with pages, assets, and generated resources before writing, so
a deployment file cannot silently overwrite another output producer. The
reserved `__heine/` namespace remains unavailable.

Full builds atomically publish a complete output tree. Consequently, removing
`deploy/google1234567890.html` removes its generated counterpart on the next
full build. Quick builds update changed, present deployment files, but—as with
all quick-build inputs—do not remove output for deleted or renamed files. Run
a normal build after such changes.
