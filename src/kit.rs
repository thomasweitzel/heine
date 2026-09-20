//! Checked kit manifests and kit-local source locations.
//!
//! A kit is not another site root. This module owns the small, separately
//! validated manifest contract that selects a presentation layer before other
//! domains decide how that layer participates in their inventories.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use semver::{Version, VersionReq};
use serde::Deserialize;
use thiserror::Error;

use crate::{source_diagnostic::DiagnosticPaths, source_path::LogicalPath};

mod extra;
mod manifest;

use extra::{ExtraSchema, merge, parse_schema, parse_selection};
pub(crate) use manifest::Manifest;
use manifest::parse_requirement;

const MANIFEST_FILE: &str = "kit.toml";
const TEMPLATES_DIRECTORY: &str = "templates";
const ASSETS_DIRECTORY: &str = "assets";
const LOCALES_DIRECTORY: &str = "locales";
const LICENSES_DIRECTORY: &str = "LICENSES";
const REFERENCE_SITE_DIRECTORY: &str = "site";
const CONFIG_FILE: &str = "heine.toml";

/// The checked identifier shared by a kit directory, manifest, template
/// namespace, and default component namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct KitName(String);

impl KitName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if crate::source_path::is_simple_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(format!("kit name {value:?} must be a simple identifier"))
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KitName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The root-owned request for one installed kit.
#[derive(Debug, Clone)]
pub(crate) struct Selection {
    name: KitName,
    version_requirement: Option<VersionReq>,
    extra: ExtraSchema,
    name_span: SourceSpan,
    version_span: Option<SourceSpan>,
    source: Option<NamedSource<String>>,
}

impl Selection {
    pub(crate) fn parse(
        name: toml::Spanned<String>,
        version_requirement: Option<toml::Spanned<String>>,
    ) -> Result<Self, ValueError> {
        let name_span = name.span().into();
        let name = KitName::parse(name.into_inner()).map_err(|message| ValueError {
            span: name_span,
            message,
        })?;
        let (version_requirement, version_span) = version_requirement
            .map(|value| {
                let span: SourceSpan = value.span().into();
                parse_requirement(value, "kit version requirement").map(|value| (value, span))
            })
            .transpose()?
            .map_or((None, None), |(value, span)| (Some(value), Some(span)));
        Ok(Self {
            name,
            version_requirement,
            extra: ExtraSchema::default(),
            name_span,
            version_span,
            source: None,
        })
    }

    pub(crate) fn name(&self) -> &KitName {
        &self.name
    }

    pub(crate) fn set_extra(&mut self, extra: ExtraSchema) {
        self.extra = extra;
    }

    pub(crate) fn attach_source(&mut self, source: NamedSource<String>) {
        self.source = Some(source);
    }
}

pub(crate) fn parse_selection_extra(source: &str) -> Result<ExtraSchema, ValueError> {
    parse_selection(source)
}

/// The root and fixed subdirectories belonging to one selected kit.
#[derive(Debug, Clone)]
pub(crate) struct Paths {
    root: PathBuf,
    manifest: PathBuf,
    templates: PathBuf,
    assets: PathBuf,
    locales: PathBuf,
    licenses: PathBuf,
    reference_site: PathBuf,
}

impl Paths {
    pub(crate) fn below_site_root(site_root: &Path, name: &KitName) -> Self {
        let root = site_root.join("kits").join(name.as_str());
        Self {
            manifest: root.join(MANIFEST_FILE),
            templates: root.join(TEMPLATES_DIRECTORY),
            assets: root.join(ASSETS_DIRECTORY),
            locales: root.join(LOCALES_DIRECTORY),
            licenses: root.join(LICENSES_DIRECTORY),
            reference_site: root.join(REFERENCE_SITE_DIRECTORY),
            root,
        }
    }

    pub(crate) fn manifest(&self) -> &Path {
        &self.manifest
    }

    fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn templates(&self) -> &Path {
        &self.templates
    }

    pub(crate) fn assets(&self) -> &Path {
        &self.assets
    }

    pub(crate) fn locales(&self) -> &Path {
        &self.locales
    }

    pub(crate) fn licenses(&self) -> &Path {
        &self.licenses
    }

    pub(crate) fn reference_site(&self) -> &Path {
        &self.reference_site
    }

    fn reference_config(&self) -> PathBuf {
        self.reference_site.join(CONFIG_FILE)
    }

    /// Only these paths contribute runtime resources. The required reference
    /// snapshot stays outside this set so it cannot affect a consuming build.
    pub(crate) fn is_runtime_input(&self, path: &Path) -> bool {
        path == self.root
            || path == self.manifest
            || [&self.templates, &self.assets, &self.locales, &self.licenses]
                .iter()
                .any(|directory| path.starts_with(directory))
    }
}

/// One selected kit after its manifest and version promises have been checked.
#[derive(Debug, Clone)]
pub(crate) struct Selected {
    paths: Paths,
    manifest: Manifest,
    extra: serde_json::Value,
}

impl Selected {
    pub(crate) fn resolve(
        site_root: &Path,
        selection: &Selection,
        diagnostics: &DiagnosticPaths,
    ) -> Result<Self, ResolveError> {
        let paths = Paths::below_site_root(site_root, selection.name());
        let manifest = Manifest::load(&paths, diagnostics)?;
        let mut selected = Self {
            paths,
            manifest,
            extra: serde_json::Value::Null,
        };
        selected.check_selection(selection)?;
        selected.validate_reference_site(diagnostics)?;
        selected.extra = merge(selected.manifest.extra(), &selection.extra, selection)?;
        Ok(selected)
    }

    pub(crate) fn paths(&self) -> &Paths {
        &self.paths
    }

    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub(crate) fn extra(&self) -> &serde_json::Value {
        &self.extra
    }

    fn check_selection(&self, selection: &Selection) -> Result<(), ResolveError> {
        let site_source = selection
            .source
            .clone()
            .ok_or(ResolveError::MissingSelectionSource)?;
        let manifest_source = self
            .manifest
            .source
            .clone()
            .ok_or(ResolveError::MissingManifestSource)?;
        if self.manifest.name != selection.name {
            return Err(ResolveError::NameMismatch(Box::new(
                NameMismatchDiagnostic {
                    selected_name: selection.name.to_string(),
                    manifest_name: self.manifest.name.to_string(),
                    source_code: site_source,
                    span: selection.name_span,
                    manifest: vec![ManifestLocation {
                        source_code: manifest_source,
                        span: self.manifest.name_span,
                        label: "kit manifest name declared here".into(),
                    }],
                },
            )));
        }
        if let Some(requirement) = &selection.version_requirement
            && !requirement.matches(&self.manifest.version)
        {
            return Err(ResolveError::VersionMismatch(Box::new(
                VersionMismatchDiagnostic {
                    requirement: requirement.to_string(),
                    installed: self.manifest.version.to_string(),
                    source_code: site_source,
                    span: selection
                        .version_span
                        .ok_or(ResolveError::MissingVersionSpan)?,
                },
            )));
        }
        let running = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|source| {
            ResolveError::InvalidRunningVersion {
                version: env!("CARGO_PKG_VERSION"),
                source,
            }
        })?;
        if !self.manifest.heine_requirement.matches(&running) {
            return Err(ResolveError::HeineVersionMismatch(Box::new(
                HeineVersionMismatchDiagnostic {
                    requirement: self.manifest.heine_requirement.to_string(),
                    running: running.to_string(),
                    source_code: manifest_source,
                    span: self.manifest.heine_requirement_span,
                },
            )));
        }
        Ok(())
    }

    /// A selected kit must carry a copyable starting point, but that snapshot
    /// never joins the consuming site's runtime inputs. Checking its selection
    /// here catches a broken package before any kit runtime resource is loaded.
    fn validate_reference_site(&self, diagnostics: &DiagnosticPaths) -> Result<(), ResolveError> {
        let site = self.paths.reference_site();
        let metadata = match fs::symlink_metadata(site) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(ResolveError::MissingReferenceSite { path: site.into() });
            }
            Err(source) => {
                return Err(ResolveError::InspectReferenceSite {
                    path: site.into(),
                    source,
                });
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(ResolveError::ReferenceSiteSymlink { path: site.into() });
        }
        if !metadata.is_dir() {
            return Err(ResolveError::ReferenceSiteNotDirectory { path: site.into() });
        }

        let config = self.paths.reference_config();
        let metadata = match fs::symlink_metadata(&config) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(ResolveError::MissingReferenceConfig { path: config });
            }
            Err(source) => {
                return Err(ResolveError::InspectReferenceSite {
                    path: config,
                    source,
                });
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(ResolveError::ReferenceSiteSymlink { path: config });
        }
        if !metadata.is_file() {
            return Err(ResolveError::ReferenceConfigNotFile { path: config });
        }
        let source =
            fs::read_to_string(&config).map_err(|source| ResolveError::ReadReferenceConfig {
                path: config.clone(),
                source,
            })?;
        let reference: RawReferenceSite = toml::from_str(&source).map_err(|error| {
            ResolveError::InvalidReferenceConfig(Box::new(ReferenceConfigDiagnostic {
                message: error.message().to_owned(),
                source_code: diagnostics.named_source(&config, source.clone(), "TOML"),
                span: error.span().unwrap_or(0..0).into(),
                help: "write a TOML [kit] table that selects this kit".into(),
            }))
        })?;
        let source_code = diagnostics.named_source(&config, source, "TOML");
        let name = reference.kit.and_then(|kit| kit.name).ok_or_else(|| {
            ResolveError::InvalidReferenceConfig(Box::new(ReferenceConfigDiagnostic {
                message: "kit reference site must select its containing kit".into(),
                source_code: source_code.clone(),
                span: (0, 0).into(),
                help: format!(
                    "add [kit] name = {:?} to the reference site's heine.toml",
                    self.manifest.name.as_str()
                ),
            }))
        })?;
        let span: SourceSpan = name.span().into();
        let name = name.into_inner();
        if name != self.manifest.name.as_str() {
            return Err(ResolveError::InvalidReferenceConfig(Box::new(
                ReferenceConfigDiagnostic {
                    message: format!(
                        "kit reference site selects {name:?}, not its containing kit {:?}",
                        self.manifest.name.as_str()
                    ),
                    source_code,
                    span,
                    help: format!("set [kit].name to {:?}", self.manifest.name.as_str()),
                },
            )));
        }
        Ok(())
    }
}

/// Only the reference site's own kit selection matters while the package is
/// validated. Its other root-owned configuration remains opaque until a user
/// copies the site and builds it as an ordinary project.
#[derive(Deserialize)]
struct RawReferenceSite {
    kit: Option<RawReferenceKit>,
}

#[derive(Deserialize)]
struct RawReferenceKit {
    name: Option<toml::Spanned<String>>,
}

/// Declared compatibility members. Their existence and visibility are checked
/// against their responsible runtime domains after those domains are loaded.
#[derive(Debug, Clone, Default)]
pub(crate) struct PublicSurface {
    pub(super) templates: Vec<Declaration<LogicalPath>>,
    pub(super) assets: Vec<Declaration<LogicalPath>>,
    pub(super) components: Vec<Declaration<String>>,
}

impl PublicSurface {
    pub(crate) fn templates(&self) -> &[Declaration<LogicalPath>] {
        &self.templates
    }

    pub(crate) fn assets(&self) -> &[Declaration<LogicalPath>] {
        &self.assets
    }

    pub(crate) fn components(&self) -> &[Declaration<String>] {
        &self.components
    }
}

/// One manifest member paired with the precise authored value that declared
/// it. Runtime inventories use the value, while diagnostics retain the span.
#[derive(Debug, Clone)]
pub(crate) struct Declaration<T> {
    pub(super) value: T,
    pub(super) span: SourceSpan,
}

impl<T> Declaration<T> {
    pub(crate) fn value(&self) -> &T {
        &self.value
    }

    pub(crate) fn span(&self) -> SourceSpan {
        self.span
    }
}

#[derive(Debug)]
pub(crate) struct ValueError {
    pub(crate) span: SourceSpan,
    pub(crate) message: String,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum ResolveError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Manifest(#[from] Error),
    #[error(transparent)]
    #[diagnostic(transparent)]
    NameMismatch(#[from] Box<NameMismatchDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    VersionMismatch(#[from] Box<VersionMismatchDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    HeineVersionMismatch(#[from] Box<HeineVersionMismatchDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Extra(#[from] Box<ExtraDiagnostic>),
    #[error("kit reference site {path} is missing")]
    MissingReferenceSite { path: PathBuf },
    #[error("could not inspect kit reference site {path}")]
    InspectReferenceSite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("kit reference site {path} is a symbolic link")]
    ReferenceSiteSymlink { path: PathBuf },
    #[error("kit reference site {path} is not a directory")]
    ReferenceSiteNotDirectory { path: PathBuf },
    #[error("kit reference configuration {path} is missing")]
    MissingReferenceConfig { path: PathBuf },
    #[error("kit reference configuration {path} is not a regular file")]
    ReferenceConfigNotFile { path: PathBuf },
    #[error("could not read kit reference configuration {path}")]
    ReadReferenceConfig {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidReferenceConfig(#[from] Box<ReferenceConfigDiagnostic>),
    #[error("internal kit selection source was not attached")]
    MissingSelectionSource,
    #[error("internal kit manifest source was not attached")]
    MissingManifestSource,
    #[error("internal kit version requirement was not attached")]
    MissingVersionSpan,
    #[error("Heine package version {version:?} is not valid semantic versioning")]
    InvalidRunningVersion {
        version: &'static str,
        #[source]
        source: semver::Error,
    },
}

#[derive(Debug, Diagnostic, Error)]
#[error("{message}")]
#[diagnostic(code(heine::kit::extra))]
pub(crate) struct ExtraDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("kit setting here")]
    span: SourceSpan,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("{message}")]
#[diagnostic(code(heine::kit::reference_site))]
pub(crate) struct ReferenceConfigDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("kit reference configuration here")]
    span: SourceSpan,
    #[help]
    help: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("kit manifest declaration")]
struct ManifestLocation {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("selected kit {selected_name:?} does not match manifest kit {manifest_name:?}")]
#[diagnostic(code(heine::kit::name_mismatch))]
pub(crate) struct NameMismatchDiagnostic {
    selected_name: String,
    manifest_name: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("selected kit declared here")]
    span: SourceSpan,
    #[related]
    manifest: Vec<ManifestLocation>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("installed kit version {installed} does not satisfy site requirement {requirement:?}")]
#[diagnostic(
    code(heine::kit::version_mismatch),
    help(
        "install a kit version that satisfies the site requirement, or update the authored requirement"
    )
)]
pub(crate) struct VersionMismatchDiagnostic {
    requirement: String,
    installed: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("site kit version requirement declared here")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("running Heine version {running} does not satisfy kit requirement {requirement:?}")]
#[diagnostic(
    code(heine::kit::heine_version_mismatch),
    help(
        "install a kit release compatible with this Heine version, or run a Heine release compatible with the installed kit"
    )
)]
pub(crate) struct HeineVersionMismatchDiagnostic {
    requirement: String,
    running: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("kit Heine compatibility requirement declared here")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("selected kit directory {path} is missing")]
    MissingPackageRoot { path: PathBuf },
    #[error("could not inspect selected kit directory {path}")]
    InspectPackageRoot {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("selected kit directory {path} is a symbolic link")]
    PackageRootSymlink { path: PathBuf },
    #[error("selected kit path {path} is not a directory")]
    PackageRootNotDirectory { path: PathBuf },
    #[error("kit manifest {path} is missing")]
    #[diagnostic(code(heine::kit::missing_manifest))]
    Missing { path: PathBuf },
    #[error("could not inspect kit manifest {path}")]
    InspectManifest {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("kit manifest {path} is a symbolic link")]
    ManifestSymlink { path: PathBuf },
    #[error("kit manifest {path} is not a regular file")]
    ManifestNotFile { path: PathBuf },
    #[error("could not read kit manifest {path}")]
    #[diagnostic(code(heine::kit::read_manifest))]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid kit manifest")]
    #[diagnostic(transparent)]
    Invalid(Box<ManifestDiagnostic>),
}

#[derive(Debug, Diagnostic, Error)]
#[error("{message}")]
#[diagnostic(code(heine::kit::invalid_manifest))]
pub(crate) struct ManifestDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("invalid kit manifest value")]
    span: SourceSpan,
}
