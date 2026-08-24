//! The sole owner of build-output filesystem writes and publication.
//!
//! A full build writes a complete replacement tree beside the current output
//! and publishes it only after every write succeeds. Quick builds update an
//! already-managed tree directly and deliberately never remove stale files.

mod publication;
mod target;
mod writer;

use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use miette::{NamedSource, SourceSpan};
use thiserror::Error;

pub(crate) use target::OutputTarget;
pub(crate) use writer::OutputWriter;

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
        for (index, claim) in claims.iter().enumerate() {
            if is_internal_output_path(claim.path.as_path()) {
                return Err(Error::Claim(Box::new(OutputClaimError::reserved(claim))));
            }
            for existing in &claims[..index] {
                if paths_conflict(existing.path.as_path(), claim.path.as_path()) {
                    return Err(Error::Claim(Box::new(OutputClaimError::collision(
                        existing, claim,
                    ))));
                }
            }
        }
        Ok(Self {
            paths: claims.iter().map(|claim| claim.path.clone()).collect(),
        })
    }

    pub(crate) fn planned_path(&self, path: &OutputPath) -> Option<PlannedOutputPath> {
        self.paths.get(path).cloned().map(PlannedOutputPath)
    }
}

fn paths_conflict(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
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
    #[error("output path {path} is reserved for Heine internal use")]
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
    #[error("copied asset {source_path} claims output {path}")]
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
            _ => unreachable!("asset discovery cannot claim duplicate output paths"),
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
    pub(crate) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub(crate) fn as_path(&self) -> &Path {
        &self.0
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
    #[error("could not copy asset {source_path} to output {output_path}")]
    AssetCopy {
        source_path: PathBuf,
        output_path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not inspect asset {path}")]
    AssetMetadata {
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
            .write(&assets, Vec::<GeneratedFile>::new(), BuildMode::Full, false)
            .expect_err("missing source asset prevents publication");
        match error {
            Error::AssetCopy {
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
