//! The sole owner of build-output filesystem writes and publication.
//!
//! A full build writes a complete replacement tree beside the current output
//! and publishes it only after every write succeeds. Quick builds update an
//! already-managed tree directly and deliberately never remove stale files.

mod publication;
mod target;
mod writer;

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
};

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use unicode_normalization::UnicodeNormalization;

use miette::Diagnostic;
use miette::{NamedSource, SourceSpan};
use thiserror::Error;

pub(crate) use target::OutputTarget;
pub(crate) use writer::{CopiedFiles, OutputWriter};

/// One producer's claim on a path in the published tree.
#[derive(Debug, Clone)]
pub(crate) struct OutputClaim {
    path: OutputPath,
    origin: OutputOrigin,
}

impl OutputClaim {
    pub(crate) fn authored(
        path: OutputPath,
        description: impl Into<String>,
        source_code: NamedSource<String>,
        span: SourceSpan,
    ) -> Self {
        Self {
            path,
            origin: OutputOrigin::Authored {
                description: description.into(),
                source_code,
                span,
            },
        }
    }
    pub(crate) fn file(path: OutputPath, source_path: PathBuf) -> Self {
        Self {
            path,
            origin: OutputOrigin::File { source_path },
        }
    }
}

#[derive(Debug, Clone)]
enum OutputOrigin {
    Authored {
        description: String,
        source_code: NamedSource<String>,
        span: SourceSpan,
    },
    File {
        source_path: PathBuf,
    },
}

/// Validates the complete set of paths claimed by build producers before any
/// rendering or output write begins.
#[derive(Debug)]
pub(crate) struct OutputPlan {
    paths: BTreeSet<OutputPath>,
}

/// A path that has been claimed by the validated output plan. Only this type
/// can construct a generated write artifact.
#[derive(Debug, Clone)]
pub(crate) struct PlannedOutputPath(OutputPath);

impl PlannedOutputPath {
    pub(crate) fn as_path(&self) -> &Path {
        self.0.as_path()
    }
}

impl OutputPlan {
    pub(crate) fn validate(claims: &[OutputClaim]) -> Result<Self, Error> {
        let mut ordered = BTreeMap::new();
        let mut portable = BTreeMap::new();
        for claim in claims {
            if is_internal_output_path(claim.path.as_path()) {
                return Err(Error::Claim(Box::new(OutputClaimError::reserved(claim))));
            }
            if let Some(existing) = portable.insert(claim.path.collision_key(), claim) {
                return Err(Error::Claim(Box::new(OutputClaimError::collision(
                    existing, claim,
                ))));
            }
            ordered.insert(claim.path.clone(), claim);
        }

        // Every non-identical conflict is an ancestor relationship. Looking
        // up each path's (normally few) ancestors keeps collision validation
        // proportional to the number of output components rather than to the
        // square of all generated files.
        for (path, claim) in &ordered {
            let mut ancestor = path.as_path().parent();
            while let Some(ancestor_path) = ancestor {
                if ancestor_path.as_os_str().is_empty() {
                    break;
                }
                let ancestor_output = OutputPath::try_new(ancestor_path.into())
                    .expect("ancestors of validated output paths remain validated");
                if let Some(existing) = portable.get(&ancestor_output.collision_key()) {
                    return Err(Error::Claim(Box::new(OutputClaimError::collision(
                        existing, claim,
                    ))));
                }
                ancestor = ancestor_path.parent();
            }
        }
        Ok(Self {
            paths: ordered.into_keys().collect(),
        })
    }

    pub(crate) fn planned_path(&self, path: &OutputPath) -> Option<PlannedOutputPath> {
        self.paths.get(path).cloned().map(PlannedOutputPath)
    }
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum OutputClaimError {
    #[error("output path {path} is reserved for Heine internal use")]
    #[diagnostic(code(heine::output::reserved_path))]
    Reserved {
        path: PathBuf,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("output declared here")]
        span: SourceSpan,
    },
    #[error(
        "copied file {source_path} would claim output path {path}, which is reserved for Heine internal use"
    )]
    #[diagnostic(code(heine::output::reserved_path))]
    ReservedFile { path: PathBuf, source_path: PathBuf },
    #[error("output path {path} is claimed by {description} and another producer")]
    #[diagnostic(code(heine::output::collision))]
    Collision {
        path: PathBuf,
        description: String,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("this output is also declared here")]
        span: SourceSpan,
        #[related]
        related: Vec<ClaimOrigin>,
    },
    #[error("output path {path} would be produced by copied files {first} and {second}")]
    #[diagnostic(code(heine::output::collision))]
    CollisionFiles {
        path: PathBuf,
        first: PathBuf,
        second: PathBuf,
    },
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum ClaimOrigin {
    #[error("{description} first claims output {path}")]
    Authored {
        description: String,
        path: PathBuf,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("first output declaration")]
        span: SourceSpan,
    },
    #[error("copied file {source_path} claims output {path}")]
    File { source_path: PathBuf, path: PathBuf },
}

impl OutputClaimError {
    fn reserved(claim: &OutputClaim) -> Self {
        match &claim.origin {
            OutputOrigin::Authored {
                source_code, span, ..
            } => Self::Reserved {
                path: claim.path.as_path().into(),
                source_code: source_code.clone(),
                span: *span,
            },
            OutputOrigin::File { source_path } => Self::ReservedFile {
                path: claim.path.as_path().into(),
                source_path: source_path.clone(),
            },
        }
    }
    fn collision(left: &OutputClaim, right: &OutputClaim) -> Self {
        let (primary, related) = match (&left.origin, &right.origin) {
            (_, OutputOrigin::Authored { .. }) => (right, left),
            (OutputOrigin::Authored { .. }, _) => (left, right),
            (
                OutputOrigin::File { source_path: first },
                OutputOrigin::File {
                    source_path: second,
                },
            ) => {
                return Self::CollisionFiles {
                    path: right.path.as_path().into(),
                    first: first.clone(),
                    second: second.clone(),
                };
            }
        };
        let OutputOrigin::Authored {
            description,
            source_code,
            span,
        } = &primary.origin
        else {
            unreachable!()
        };
        Self::Collision {
            path: primary.path.as_path().into(),
            description: description.clone(),
            source_code: source_code.clone(),
            span: *span,
            related: vec![ClaimOrigin::from_claim(related)],
        }
    }
}

impl ClaimOrigin {
    fn from_claim(claim: &OutputClaim) -> Self {
        match &claim.origin {
            OutputOrigin::Authored {
                description,
                source_code,
                span,
            } => Self::Authored {
                description: description.clone(),
                path: claim.path.as_path().into(),
                source_code: source_code.clone(),
                span: *span,
            },
            OutputOrigin::File { source_path } => Self::File {
                source_path: source_path.clone(),
                path: claim.path.as_path().into(),
            },
        }
    }
}

/// A validated filesystem path relative to the generated site's root.
///
/// The type is shared by every producer of published files. It intentionally
/// belongs to the output domain rather than to pages: rendered pages, copied
/// assets, and generated build products all claim paths in the same tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OutputPath(PathBuf);

impl OutputPath {
    /// Validates one filesystem-relative output location.
    ///
    /// This establishes the per-path invariant. `OutputPlan` separately owns
    /// relationships between several valid paths, such as collisions.
    pub(crate) fn try_new(path: PathBuf) -> Result<Self, String> {
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || !path.components().all(|component| {
                matches!(component, std::path::Component::Normal(component) if component
                    .to_str()
                    .is_some_and(is_safe_output_component))
            })
        {
            return Err(
                "output paths must be non-empty relative paths with portable filename components"
                    .into(),
            );
        }
        Ok(Self(path))
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns a slash-separated, unescaped spelling for template-facing
    /// identifiers. It is not a URL; use [`Self::url_path`] for URL syntax.
    pub(crate) fn logical_spelling(&self) -> String {
        self.0
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(component) => component.to_str(),
                std::path::Component::CurDir
                | std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// A deterministic NFC-normalized, case-independent output identity used
    /// only for collision checks. Heine deliberately rejects names that differ
    /// only by Unicode normalization or lowercase spelling, rather than
    /// depending on a deployment filesystem's naming policy.
    /// Returns the portable output identity used exclusively for collision
    /// checks between several producers.
    pub(crate) fn collision_key(&self) -> String {
        self.0
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(component) => component.to_str(),
                std::path::Component::CurDir
                | std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => None,
            })
            .map(|component| {
                component
                    .nfc()
                    .flat_map(char::to_lowercase)
                    .nfc()
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Converts this validated output location into a browser-facing URL
    /// path. Output names stay literal on disk; URL escaping happens exactly
    /// once at this boundary.
    pub(crate) fn url_path(&self) -> UrlPath {
        UrlPath::from_output(self)
    }
}

/// A canonical percent-encoded URL path relative to the generated site root.
///
/// It is distinct from [`OutputPath`]: the former is safe to place in HTML or
/// an HTTP URL, while the latter is a filesystem-relative location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UrlPath(String);

impl UrlPath {
    fn from_output(path: &OutputPath) -> Self {
        Self::from_components(path.as_path())
    }

    /// Converts a validated logical source path to URL syntax.
    pub(crate) fn from_logical(path: &crate::source_path::LogicalPath) -> Self {
        Self::from_components(path.as_path())
    }

    fn from_components(path: &Path) -> Self {
        Self(
            path.components()
                .filter_map(|component| match component {
                    std::path::Component::Normal(component) => component.to_str(),
                    std::path::Component::CurDir
                    | std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_) => None,
                })
                .map(|component| utf8_percent_encode(component, URL_PATH_SEGMENT).to_string())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

// RFC 3986 path segments allow `pchar`: unreserved characters, sub-delims,
// `:` and `@`. Every other byte, including a literal percent sign, is encoded
// so source filenames can never be mistaken for already-escaped URL syntax.
const URL_PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[cfg(test)]
mod url_path_tests {
    use super::*;

    #[test]
    fn output_urls_encode_literal_filename_bytes() {
        let path =
            OutputPath::try_new(PathBuf::from("über space#%/ab.html")).expect("valid output path");

        assert_eq!(path.url_path().as_str(), "%C3%BCber%20space%23%25/ab.html");
    }
}

/// Bytes produced by a renderer or a build feature and awaiting the output
/// stage. The writer treats these uniformly; it does not know whether they
/// came from a page, syntax highlighting, or a later generated feature.
#[derive(Debug)]
pub(crate) struct GeneratedFile {
    pub(crate) path: PlannedOutputPath,
    pub(crate) bytes: Vec<u8>,
}

impl GeneratedFile {
    pub(crate) fn text(path: PlannedOutputPath, contents: String) -> Self {
        Self {
            path,
            bytes: contents.into_bytes(),
        }
    }
}

/// Reserved for development-server endpoints and never published as site
/// output. Keeping this namespace out of the output tree prevents a source
/// asset or page from changing development-server behavior.
pub(crate) const INTERNAL_DIRECTORY: &str = "__heine";
pub(super) const MARKER: &str = ".heine-output";
pub(super) const MARKER_CONTENTS: &str = "heine-output-v1\n";

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Claim(#[from] Box<OutputClaimError>),
    #[error("could not write output {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("output path {path} is a symbolic link")]
    OutputSymlink { path: PathBuf },
    #[error("output path ancestor {path} is a symbolic link")]
    OutputAncestorSymlink { path: PathBuf },
    #[error("output target {path} is a symbolic link")]
    OutputTargetSymlink { path: PathBuf },
    #[error("output path {path} is not a directory")]
    OutputNotDirectory { path: PathBuf },
    #[error("refusing to replace unmanaged output directory {path}")]
    #[diagnostic(help("run a full build with --clean-output to adopt and replace this directory"))]
    UnmanagedOutput { path: PathBuf },
    #[error("output path {path} is reserved for Heine internal use")]
    ReservedPath { path: PathBuf },
    #[error("output directory {path} has an unrecognized ownership marker {marker}")]
    #[diagnostic(help(
        "run a full build with --clean-output to explicitly replace this directory"
    ))]
    InvalidMarker { path: PathBuf, marker: PathBuf },
    #[error("could not copy source file {source_path} to output {output_path}")]
    Copy {
        source_path: PathBuf,
        output_path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not inspect copied source or output file {path}")]
    CopyMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not restore previous output {path} after publication failed")]
    Restore {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("published output {output}, but could not remove its previous version at {backup}")]
    #[diagnostic(help(
        "the new site is already published; remove the leftover backup directory when it is safe to do so"
    ))]
    PublishedWithBackup {
        output: PathBuf,
        backup: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Whether an output path enters the namespace owned by the development
/// server rather than generated site files.
pub(crate) fn is_reserved_output_path(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_os_str() == INTERNAL_DIRECTORY)
}

/// Whether an output path is owned by Heine rather than published site data.
///
/// The marker is only internal at the output root; an asset may legitimately
/// use the same dotfile name in one of its own subdirectories.
pub(crate) fn is_internal_output_path(path: &Path) -> bool {
    path == Path::new(MARKER) || is_reserved_output_path(path)
}

/// Whether one user-authored path component can be materialized portably in
/// Heine's output tree. URL-facing generated names must satisfy this in
/// addition to their URL grammar: characters such as `:` and `*` are legal in
/// some URLs but not in Windows filenames.
pub(crate) fn is_safe_output_component(component: &str) -> bool {
    !component.is_empty()
        && !matches!(component, "." | "..")
        && !component.ends_with(['.', ' '])
        && !component.chars().any(char::is_control)
        && !component.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*'])
        && !is_windows_device_name(component)
}

fn is_windows_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetCatalog, site::BuildMode};
    use std::{
        collections::BTreeSet,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    use walkdir::WalkDir;

    #[test]
    fn output_components_use_portable_filename_syntax() {
        assert!(is_safe_output_component("page"));
        assert!(!is_safe_output_component("page:two"));
        assert!(!is_safe_output_component("page*two"));
        assert!(!is_safe_output_component("page. "));
        assert!(!is_safe_output_component("page\u{1}"));
        assert!(!is_safe_output_component("con"));
        assert!(!is_safe_output_component("LPT1.log"));
        assert!(!is_safe_output_component("COM¹.log"));
    }

    #[test]
    fn output_paths_admit_only_portable_relative_components() {
        assert!(OutputPath::try_new("blog/post.html".into()).is_ok());
        assert!(OutputPath::try_new("".into()).is_err());
        assert!(OutputPath::try_new("../post.html".into()).is_err());
        assert!(OutputPath::try_new("blog/post?.html".into()).is_err());
        assert!(OutputPath::try_new("CON.html".into()).is_err());
    }

    #[test]
    fn output_plan_detects_ancestor_claims_even_with_intervening_paths() {
        let claims = [
            OutputClaim::authored(
                OutputPath::try_new("articles".into()).expect("valid output path"),
                "first page",
                NamedSource::new("first.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
            OutputClaim::authored(
                OutputPath::try_new("articles.about".into()).expect("valid output path"),
                "intervening page",
                NamedSource::new("about.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
            OutputClaim::authored(
                OutputPath::try_new("articles/index.html".into()).expect("valid output path"),
                "second page",
                NamedSource::new("second.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
        ];

        assert!(OutputPlan::validate(&claims).is_err());
    }

    #[test]
    fn output_plan_rejects_case_independent_collisions() {
        let claims = [
            OutputClaim::authored(
                OutputPath::try_new("Blog/Index.html".into()).expect("valid output path"),
                "first page",
                NamedSource::new("first.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
            OutputClaim::authored(
                OutputPath::try_new("blog/index.html".into()).expect("valid output path"),
                "second page",
                NamedSource::new("second.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
        ];

        assert!(OutputPlan::validate(&claims).is_err());
    }

    #[test]
    fn output_plan_reports_case_independent_asset_collisions() {
        let claims = [
            OutputClaim::file(
                OutputPath::try_new("images/Logo.svg".into()).expect("valid output path"),
                "content/en/images/Logo.svg".into(),
            ),
            OutputClaim::file(
                OutputPath::try_new("images/logo.svg".into()).expect("valid output path"),
                "content/en/images/logo.svg".into(),
            ),
        ];

        let error = OutputPlan::validate(&claims).expect_err("case collision must fail");
        assert!(matches!(
            error,
            Error::Claim(error) if matches!(*error, OutputClaimError::CollisionFiles { .. })
        ));
    }

    #[test]
    fn output_plan_rejects_unicode_normalization_collisions() {
        let claims = [
            OutputClaim::authored(
                OutputPath::try_new("café.html".into()).expect("valid output path"),
                "first page",
                NamedSource::new("café.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
            OutputClaim::authored(
                OutputPath::try_new("cafe\u{301}.html".into()).expect("valid output path"),
                "second page",
                NamedSource::new("café.page", "template = 'page.tera'".to_owned()),
                (0, 1).into(),
            ),
        ];

        assert!(OutputPlan::validate(&claims).is_err());
    }

    #[test]
    fn a_staging_write_failure_preserves_output_and_removes_the_staging_tree() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("heine-output-test-{}-{nonce}", std::process::id()));
        let content = root.join("content");
        let output = root.join("public");
        fs::create_dir_all(&content).expect("content directory");
        fs::write(content.join("asset.txt"), "asset").expect("asset source");
        let entries = WalkDir::new(&content)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("content entries");
        let assets =
            AssetCatalog::discover(&content, &entries, &BTreeSet::new()).expect("asset catalog");
        fs::remove_file(content.join("asset.txt")).expect("remove source asset");
        fs::create_dir_all(&output).expect("output directory");
        target::write_marker(&output).expect("output marker");
        fs::write(output.join("index.html"), "previous site").expect("previous output");
        let error = OutputWriter::new(OutputTarget::new(&root, Path::new("public")))
            .write(
                &[&assets],
                Vec::<GeneratedFile>::new(),
                BuildMode::Full,
                false,
            )
            .expect_err("missing source asset prevents publication");
        match error {
            Error::Copy {
                source_path,
                output_path,
                ..
            } => {
                assert_eq!(source_path, content.join("asset.txt"));
                assert!(output_path.ends_with("asset.txt"));
            }
            error => panic!("expected asset copy error, got {error:?}"),
        }
        assert_eq!(
            fs::read_to_string(output.join("index.html")).expect("previous output"),
            "previous site"
        );
        assert!(fs::read_dir(&root).expect("root entries").all(|entry| {
            !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".public.heine-staging-")
        }));
        fs::remove_dir_all(root).expect("test cleanup");
    }

    #[test]
    fn published_backup_cleanup_errors_explain_the_published_state() {
        let error = Error::PublishedWithBackup {
            output: "public".into(),
            backup: ".public.heine-previous".into(),
            source: io::Error::other("permission denied"),
        };
        assert!(error.to_string().starts_with("published output public"));
        assert!(
            error
                .help()
                .expect("cleanup error has recovery guidance")
                .to_string()
                .contains("already published")
        );
    }

    #[test]
    fn distinguishes_internal_output_paths_from_nested_dotfile_assets() {
        assert!(is_internal_output_path(Path::new(MARKER)));
        assert!(is_internal_output_path(Path::new("__heine/reload.js")));
        assert!(!is_internal_output_path(Path::new("images/.heine-output")));
    }
}
