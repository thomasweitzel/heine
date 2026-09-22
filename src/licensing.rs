//! REUSE-compatible licensing declarations for copied content assets.
//!
//! A project may place license texts in `LICENSES/`. They are published under
//! the same path. Adjacent `asset.ext.license` sidecars declare the copyright
//! notices and SPDX expression for the asset they accompany.

mod declaration;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{
    asset::{AssetCatalog, AssetLicensing},
    config::{LicensingConfig, LicensingCoverage},
    output::{CopiedFiles, OutputPath},
    source_diagnostic, source_path,
};

const LICENSE_DIRECTORY: &str = "LICENSES";

use declaration::{DeclarationFields, DeclarationProblem, TaggedValue};

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not inspect license directory {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("license directory {path} is not a directory")]
    NotDirectory { path: PathBuf },
    #[error("license path {path} is a symbolic link")]
    Symlink { path: PathBuf },
    #[error("could not walk license directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("license path {path} is not a regular file")]
    UnsupportedEntry { path: PathBuf },
    #[error(
        "license file {path} must be a direct `.txt` file named after an SPDX identifier or LicenseRef"
    )]
    InvalidLicenseFile { path: PathBuf },
    #[error("could not read licensing sidecar {path}")]
    ReadSidecar {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Detailed(#[from] Box<SidecarDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Group(#[from] Box<GroupDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Coverage(#[from] Box<CoverageDiagnostic>),
    #[error("could not inspect asset {path} declared by a licensing sidecar")]
    AssetMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Asset(#[from] Box<crate::asset::Error>),
    #[error("licensing configuration source was unavailable during resolution")]
    ConfigurationSourceUnavailable,
}

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum SidecarDiagnostic {
    #[error("licensing sidecar {path} has no copied asset sibling")]
    OrphanSidecar {
        path: PathBuf,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("this sidecar must be adjacent to a copied asset")]
        span: SourceSpan,
    },
    #[error("licensing sidecar {path} refers to missing license text {license_path}")]
    MissingLicenseText {
        path: PathBuf,
        license_path: PathBuf,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("this SPDX identifier needs a matching license text")]
        span: SourceSpan,
    },
    #[error("invalid licensing sidecar {path}: {message}")]
    InvalidSidecar {
        path: PathBuf,
        message: String,
        #[source_code]
        source_code: NamedSource<String>,
        #[label("{label}")]
        span: SourceSpan,
        label: String,
    },
}

#[derive(Debug, Diagnostic, Error)]
#[diagnostic(code(heine::licensing::group))]
#[error("{message}")]
pub(crate) struct GroupDiagnostic {
    message: String,
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
    #[help]
    help: String,
    #[related]
    related: Vec<DeclarationOrigin>,
}

#[derive(Debug, Diagnostic, Error)]
#[diagnostic(code(heine::licensing::coverage))]
#[error("copied assets are missing required licensing declarations:{missing}")]
pub(crate) struct CoverageDiagnostic {
    #[help]
    help: &'static str,
    missing: UncoveredAssets,
}

#[derive(Debug)]
struct UncoveredAssets(Vec<UncoveredAsset>);

impl fmt::Display for UncoveredAssets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for asset in &self.0 {
            writeln!(
                formatter,
                "\n  {}\n    publishes as {}",
                asset.source_path.display(),
                asset.output_path.display()
            )?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct UncoveredAsset {
    source_path: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug, Clone, Diagnostic, Error)]
#[error("first licensing declaration for this asset")]
struct DeclarationOrigin {
    #[source_code]
    source_code: Arc<NamedSource<String>>,
    #[label("first declaration here")]
    span: SourceSpan,
}

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    entries: BTreeMap<OutputPath, PathBuf>,
    paths: Vec<OutputPath>,
    texts: BTreeSet<String>,
    root: PathBuf,
}

impl Catalog {
    pub(crate) fn load(
        root: &Path,
        selected_kit: Option<&crate::kit::Selected>,
    ) -> Result<Self, Error> {
        let mut catalog = Self {
            root: root.into(),
            ..Self::default()
        };
        if let Some(selected_kit) = selected_kit {
            catalog.load_texts(selected_kit.paths().licenses())?;
        }
        catalog.load_texts(root)?;
        catalog.paths = catalog.entries.keys().cloned().collect();
        Ok(catalog)
    }

    /// Each layer uses the same logical LICENSES namespace. Loading the root
    /// after the selected kit gives a site owner the documented correction
    /// path without inventing a separate legal-output domain.
    fn load_texts(&mut self, root: &Path) -> Result<(), Error> {
        match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Symlink { path: root.into() });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::NotDirectory { path: root.into() });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(source) => {
                return Err(Error::Inspect {
                    path: root.into(),
                    source,
                });
            }
        }
        for entry in WalkDir::new(root).sort_by_file_name() {
            let entry = entry.map_err(|source| Error::Walk {
                path: root.into(),
                source,
            })?;
            if entry.path() == root {
                continue;
            }
            if entry.file_type().is_symlink() {
                return Err(Error::Symlink {
                    path: entry.path().into(),
                });
            }
            if entry.file_type().is_dir() {
                return Err(Error::InvalidLicenseFile {
                    path: entry.path().into(),
                });
            }
            if !entry.file_type().is_file() {
                return Err(Error::UnsupportedEntry {
                    path: entry.path().into(),
                });
            }
            let relative =
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| Error::InvalidLicenseFile {
                        path: entry.path().into(),
                    })?;
            if relative.components().count() != 1
                || !source_path::has_safe_components(relative)
                || !valid_license_filename(relative)
            {
                return Err(Error::InvalidLicenseFile {
                    path: entry.path().into(),
                });
            }
            let Some(name) = relative.file_stem().and_then(|name| name.to_str()) else {
                return Err(Error::InvalidLicenseFile {
                    path: entry.path().into(),
                });
            };
            let name = name.to_owned();
            self.texts.insert(name);
            let output = license_output_path(entry.path(), relative)?;
            self.entries.insert(output, entry.path().into());
        }
        Ok(())
    }

    pub(crate) fn attach_to_assets(
        &self,
        assets: &mut AssetCatalog,
        content_root: &Path,
        kit_assets: Option<&crate::asset::KitAssets>,
        config: &LicensingConfig,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Result<(), Error> {
        let mut declarations = BTreeMap::new();
        self.attach_sidecars(assets, content_root, false, &mut declarations, diagnostics)?;
        if let Some(kit_assets) = kit_assets {
            self.attach_sidecars(
                assets,
                kit_assets.root(),
                true,
                &mut declarations,
                diagnostics,
            )?;
        }
        self.attach_groups(assets, config, &mut declarations, diagnostics)?;
        if config.coverage() == LicensingCoverage::Required {
            self.require_coverage(assets, diagnostics)?;
        }
        Ok(())
    }

    fn attach_sidecars(
        &self,
        assets: &mut AssetCatalog,
        root: &Path,
        ignore_masked_assets: bool,
        declarations: &mut BTreeMap<OutputPath, DeclarationOrigin>,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Result<(), Error> {
        for entry in WalkDir::new(root).sort_by_file_name() {
            let entry = entry.map_err(|source| Error::Walk {
                path: root.into(),
                source,
            })?;
            if entry.file_type().is_symlink() {
                return Err(Error::Symlink {
                    path: entry.path().into(),
                });
            }
            if !entry.file_type().is_file()
                || entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "license")
            {
                continue;
            }
            let source = read_sidecar(entry.path())?;
            let sibling = entry.path().with_extension("");
            if !assets.contains_source(&sibling) {
                if ignore_masked_assets && sibling.is_file() {
                    continue;
                }
                return Err(Box::new(SidecarDiagnostic::OrphanSidecar {
                    path: diagnostics.display_path(entry.path()),
                    source_code: diagnostics.named_source(entry.path(), source.clone(), "text"),
                    span: first_line_span(&source),
                })
                .into());
            }
            let record = self.parse_sidecar(entry.path(), &source, diagnostics)?;
            let declaration_span = first_line_span(&source);
            let modified = fs::metadata(entry.path())
                .and_then(|metadata| metadata.modified())
                .map_err(|source_error| Error::AssetMetadata {
                    path: entry.path().into(),
                    source: source_error,
                })?;
            let output = assets
                .attach_licensing(&sibling, record, modified)
                .map_err(Box::new)?;
            declarations.insert(
                output,
                DeclarationOrigin {
                    source_code: Arc::new(diagnostics.named_source(entry.path(), source, "text")),
                    span: declaration_span,
                },
            );
        }
        Ok(())
    }

    fn parse_sidecar(
        &self,
        path: &Path,
        source: &str,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Result<AssetLicensing, Error> {
        let mut copyrights = Vec::new();
        let mut expressions = Vec::new();
        let mut comments = Vec::new();
        let mut line_offset = 0;
        for line in source.split_inclusive('\n') {
            let Some((tag, value)) = line.split_once(':') else {
                line_offset += line.len();
                continue;
            };
            let leading = value.len() - value.trim_start().len();
            let value = value.trim();
            let span = (line_offset + tag.len() + 1 + leading, value.len()).into();
            match tag.trim() {
                "SPDX-FileCopyrightText" => copyrights.push(TaggedValue {
                    value: value.to_owned(),
                    span,
                    span_is_exact: true,
                }),
                "SPDX-License-Identifier" => expressions.push(TaggedValue {
                    value: value.to_owned(),
                    span,
                    span_is_exact: true,
                }),
                "SPDX-FileComment" => comments.push(TaggedValue {
                    value: value.to_owned(),
                    span,
                    span_is_exact: true,
                }),
                _ => {}
            }
            line_offset += line.len();
        }
        let source_code = || diagnostics.named_source(path, source, "text");
        if copyrights.iter().any(|value| value.value.is_empty()) || copyrights.is_empty() {
            return Err(Box::new(SidecarDiagnostic::InvalidSidecar {
                path: diagnostics.display_path(path),
                message: "at least one non-empty SPDX-FileCopyrightText is required".into(),
                source_code: source_code(),
                span: (0, source.len().min(1)).into(),
                label: "add an SPDX-FileCopyrightText declaration".into(),
            })
            .into());
        }
        if expressions.iter().any(|value| value.value.is_empty()) || expressions.is_empty() {
            return Err(Box::new(SidecarDiagnostic::InvalidSidecar {
                path: diagnostics.display_path(path),
                message: "at least one non-empty SPDX-License-Identifier is required".into(),
                source_code: source_code(),
                span: (0, source.len().min(1)).into(),
                label: "add an SPDX-License-Identifier declaration".into(),
            })
            .into());
        }
        self.resolve_declaration(DeclarationFields {
            copyrights,
            expressions,
            comments,
        })
        .map_err(|problem| self.sidecar_problem(path, source, problem, diagnostics))
    }

    fn resolve_declaration(
        &self,
        fields: DeclarationFields,
    ) -> Result<AssetLicensing, DeclarationProblem> {
        declaration::resolve(&self.texts, fields)
    }

    fn sidecar_problem(
        &self,
        path: &Path,
        source: &str,
        problem: DeclarationProblem,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Error {
        let source_code = || diagnostics.named_source(path, source, "text");
        match problem {
            DeclarationProblem::InvalidExpression { span, message } => {
                Box::new(SidecarDiagnostic::InvalidSidecar {
                    path: diagnostics.display_path(path),
                    message,
                    source_code: source_code(),
                    span,
                    label: "invalid SPDX expression".into(),
                })
                .into()
            }
            DeclarationProblem::UnsupportedReference {
                span,
                message,
                help,
            } => Box::new(SidecarDiagnostic::InvalidSidecar {
                path: diagnostics.display_path(path),
                message: message.into(),
                source_code: source_code(),
                span,
                label: help.into(),
            })
            .into(),
            DeclarationProblem::MissingLicenseText { span, id } => {
                Box::new(SidecarDiagnostic::MissingLicenseText {
                    path: diagnostics.display_path(path),
                    license_path: diagnostics.display_path(&self.root.join(format!("{id}.txt"))),
                    source_code: source_code(),
                    span,
                })
                .into()
            }
        }
    }

    fn attach_groups(
        &self,
        assets: &mut AssetCatalog,
        config: &LicensingConfig,
        declarations: &mut BTreeMap<OutputPath, DeclarationOrigin>,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Result<(), Error> {
        for group in config.groups() {
            let source_code = group
                .source()
                .cloned()
                .ok_or(Error::ConfigurationSourceUnavailable)?;
            let record = self
                .resolve_declaration(declaration::from_config(group.declaration()))
                .map_err(|problem| self.group_problem(source_code.clone(), problem, diagnostics))?;
            for member in group.assets() {
                let output = member.output();
                if !assets.contains_output(output) {
                    return Err(Box::new(GroupDiagnostic {
                        message: format!(
                            "licensing group names {path}, which is not a copied asset",
                            path = output.as_path().display()
                        ),
                        source_code,
                        span: member.span(),
                        label: "this group member does not resolve to a copied asset".into(),
                        help: "use the literal published output_path of a current copied asset"
                            .into(),
                        related: Vec::new(),
                    })
                    .into());
                }
                if let Some(first) = declarations.get(output) {
                    return Err(Box::new(GroupDiagnostic {
                        message: format!(
                            "copied asset {path} has more than one licensing declaration",
                            path = output.as_path().display()
                        ),
                        source_code,
                        span: member.span(),
                        label: "another declaration already applies to this copied asset".into(),
                        help: "remove this member from the group, or remove the other declaration"
                            .into(),
                        related: vec![first.clone()],
                    })
                    .into());
                }
                assets
                    .attach_output_licensing(output, record.clone())
                    .map_err(Box::new)?;
                declarations.insert(
                    output.clone(),
                    DeclarationOrigin {
                        source_code: source_code.clone(),
                        span: member.span(),
                    },
                );
            }
        }
        Ok(())
    }

    fn require_coverage(
        &self,
        assets: &AssetCatalog,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Result<(), Error> {
        let missing = assets
            .unlicensed_assets()
            .into_iter()
            .map(|(output, source)| UncoveredAsset {
                source_path: diagnostics.display_path(&source),
                output_path: output.as_path().into(),
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(Box::new(CoverageDiagnostic {
            help: "add an adjacent .license sidecar or include each asset in one licensing group",
            missing: UncoveredAssets(missing),
        })
        .into())
    }

    fn group_problem(
        &self,
        source_code: Arc<NamedSource<String>>,
        problem: DeclarationProblem,
        diagnostics: &source_diagnostic::DiagnosticPaths,
    ) -> Error {
        let (span, message, label, help) = match problem {
            DeclarationProblem::InvalidExpression { span, message } => (
                span,
                format!("invalid licensing group declaration: {message}"),
                "invalid SPDX expression".into(),
                "use a valid SPDX license expression".into(),
            ),
            DeclarationProblem::UnsupportedReference {
                span,
                message,
                help,
            } => (
                span,
                format!("invalid licensing group declaration: {message}"),
                "this reference cannot be resolved from local LICENSES/".into(),
                help.into(),
            ),
            DeclarationProblem::MissingLicenseText { span, id } => (
                span,
                format!(
                    "licensing group refers to missing license text {}",
                    diagnostics
                        .display_path(&self.root.join(format!("{id}.txt")))
                        .display()
                ),
                "this SPDX identifier needs a matching license text".into(),
                "add the matching text below LICENSES/".into(),
            ),
        };
        Box::new(GroupDiagnostic {
            message,
            source_code,
            span,
            label,
            help,
            related: Vec::new(),
        })
        .into()
    }

    pub(crate) fn paths(&self) -> &[OutputPath] {
        &self.paths
    }
    pub(crate) fn source_path(&self, output: &OutputPath) -> &Path {
        &self.entries[output]
    }
}

impl CopiedFiles for Catalog {
    fn paths(&self) -> &[OutputPath] {
        self.paths()
    }
    fn source_path(&self, output: &OutputPath) -> &Path {
        self.source_path(output)
    }
}

fn read_sidecar(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|source| Error::ReadSidecar {
        path: path.into(),
        source,
    })
}

fn license_output_path(source: &Path, relative: &Path) -> Result<OutputPath, Error> {
    OutputPath::try_new(Path::new(LICENSE_DIRECTORY).join(relative)).map_err(|_| {
        Error::InvalidLicenseFile {
            path: source.into(),
        }
    })
}

fn first_line_span(source: &str) -> SourceSpan {
    (0, source.find('\n').unwrap_or(source.len()).max(1)).into()
}

fn valid_license_filename(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    path.extension().is_some_and(|extension| extension == "txt")
        && (spdx::license_id(stem).is_some()
            || spdx::exception_id(stem).is_some()
            || stem
                .strip_prefix("LicenseRef-")
                .is_some_and(|name| !name.is_empty()))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_license_output_paths_name_the_source_file() {
        let source = Path::new("/site/LICENSES/LicenseRef-local:.txt");
        let error = license_output_path(source, Path::new("LicenseRef-local:.txt"))
            .expect_err("colon is not portable in an output filename");
        assert!(matches!(error, Error::InvalidLicenseFile { path } if path == source));
    }
}
