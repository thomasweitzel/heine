//! REUSE-compatible licensing declarations for copied content assets.
//!
//! A project may place license texts in `LICENSES/`. They are published under
//! the same path. Adjacent `asset.ext.license` sidecars declare the copyright
//! notices and SPDX expression for the asset they accompany.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{
    asset::{AssetCatalog, AssetLicensing, LicenseText},
    output::{CopiedFiles, OutputPath},
    source_diagnostic, source_path,
};

const LICENSE_DIRECTORY: &str = "LICENSES";

#[derive(Debug)]
struct TaggedValue {
    value: String,
    span: SourceSpan,
}

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
    #[error("could not inspect asset {path} declared by a licensing sidecar")]
    AssetMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Asset(#[from] Box<crate::asset::Error>),
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

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    entries: BTreeMap<OutputPath, PathBuf>,
    paths: Vec<OutputPath>,
    texts: BTreeSet<String>,
    root: PathBuf,
}

impl Catalog {
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        match fs::symlink_metadata(root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Symlink { path: root.into() });
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Error::NotDirectory { path: root.into() });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    root: root.into(),
                    ..Self::default()
                });
            }
            Err(source) => {
                return Err(Error::Inspect {
                    path: root.into(),
                    source,
                });
            }
        }
        let mut entries = BTreeMap::new();
        let mut texts = BTreeSet::new();
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
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("walk entries remain under their root");
            if relative.components().count() != 1
                || !source_path::has_safe_components(relative)
                || !valid_license_filename(relative)
            {
                return Err(Error::InvalidLicenseFile {
                    path: entry.path().into(),
                });
            }
            let name = relative
                .file_stem()
                .and_then(|name| name.to_str())
                .expect("validated UTF-8 file name")
                .to_owned();
            texts.insert(name);
            let output = license_output_path(entry.path(), relative)?;
            entries.insert(output, entry.path().into());
        }
        let paths = entries.keys().cloned().collect();
        Ok(Self {
            entries,
            paths,
            texts,
            root: root.into(),
        })
    }

    pub(crate) fn attach_to_assets(
        &self,
        assets: &mut AssetCatalog,
        content_root: &Path,
    ) -> Result<(), Error> {
        for entry in WalkDir::new(content_root).sort_by_file_name() {
            let entry = entry.map_err(|source| Error::Walk {
                path: content_root.into(),
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
                return Err(Box::new(SidecarDiagnostic::OrphanSidecar {
                    path: entry.path().into(),
                    source_code: source_diagnostic::named_source(
                        entry.path(),
                        source.clone(),
                        "text",
                    ),
                    span: first_line_span(&source),
                })
                .into());
            }
            let record = self.parse_sidecar(entry.path(), &source)?;
            let modified = fs::metadata(entry.path())
                .and_then(|metadata| metadata.modified())
                .map_err(|source_error| Error::AssetMetadata {
                    path: entry.path().into(),
                    source: source_error,
                })?;
            assets
                .attach_licensing(&sibling, record, modified)
                .map_err(Box::new)?;
        }
        Ok(())
    }

    fn parse_sidecar(&self, path: &Path, source: &str) -> Result<AssetLicensing, Error> {
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
                }),
                "SPDX-License-Identifier" => expressions.push(TaggedValue {
                    value: value.to_owned(),
                    span,
                }),
                "SPDX-FileComment" => comments.push(TaggedValue {
                    value: value.to_owned(),
                    span,
                }),
                _ => {}
            }
            line_offset += line.len();
        }
        let source_code = || source_diagnostic::named_source(path, source, "text");
        if copyrights.iter().any(|value| value.value.is_empty()) || copyrights.is_empty() {
            return Err(Box::new(SidecarDiagnostic::InvalidSidecar {
                path: path.into(),
                message: "at least one non-empty SPDX-FileCopyrightText is required".into(),
                source_code: source_code(),
                span: (0, source.len().min(1)).into(),
                label: "add an SPDX-FileCopyrightText declaration".into(),
            })
            .into());
        }
        if expressions.iter().any(|value| value.value.is_empty()) || expressions.is_empty() {
            return Err(Box::new(SidecarDiagnostic::InvalidSidecar {
                path: path.into(),
                message: "at least one non-empty SPDX-License-Identifier is required".into(),
                source_code: source_code(),
                span: (0, source.len().min(1)).into(),
                label: "add an SPDX-License-Identifier declaration".into(),
            })
            .into());
        }
        let mut license_ids = BTreeMap::new();
        for expression in &expressions {
            let parsed = spdx::Expression::parse(&expression.value).map_err(|error| {
                Box::new(SidecarDiagnostic::InvalidSidecar {
                    path: path.into(),
                    message: format!("{error}"),
                    source_code: source_code(),
                    span: expression_span(expression, error.span),
                    label: "invalid SPDX expression".into(),
                })
            })?;
            for requirement in parsed.requirements() {
                let span = expression_requirement_span(expression, requirement.span.clone());
                let license = match &requirement.req.license {
                    spdx::LicenseItem::Spdx { .. } => requirement.req.license.to_string(),
                    spdx::LicenseItem::Other(reference) if reference.doc_ref.is_none() => {
                        requirement.req.license.to_string()
                    }
                    spdx::LicenseItem::Other(_) => {
                        return Err(unsupported_spdx_reference(
                            path,
                            source,
                            span,
                            "external DocumentRef license references are not supported by Heine's local LICENSES/ model",
                            "use a local LicenseRef, or provide a recognized SPDX license identifier",
                        ));
                    }
                };
                license_ids.insert(license, span);
                if let Some(addition) = &requirement.req.addition {
                    let addition = match addition {
                        spdx::AdditionItem::Spdx(_) => addition.to_string(),
                        spdx::AdditionItem::Other(_) => {
                            return Err(unsupported_spdx_reference(
                                path,
                                source,
                                span,
                                "AdditionRef exceptions are not supported by Heine's local LICENSES/ model",
                                "use a recognized SPDX exception identifier",
                            ));
                        }
                    };
                    license_ids.insert(addition, span);
                }
            }
        }
        for (id, span) in &license_ids {
            if !self.texts.contains(id) {
                let filename = format!("{id}.txt");
                return Err(Box::new(SidecarDiagnostic::MissingLicenseText {
                    path: path.into(),
                    license_path: self.root.join(&filename),
                    source_code: source_code(),
                    span: *span,
                })
                .into());
            }
        }
        Ok(AssetLicensing {
            copyrights: copyrights.into_iter().map(|value| value.value).collect(),
            expressions: expressions.into_iter().map(|value| value.value).collect(),
            comments: comments.into_iter().map(|value| value.value).collect(),
            license_texts: license_ids
                .into_keys()
                .map(|id| LicenseText {
                    output: OutputPath::try_new(
                        Path::new(LICENSE_DIRECTORY).join(format!("{id}.txt")),
                    )
                    .expect("validated license output path"),
                    id,
                })
                .collect(),
        })
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

fn unsupported_spdx_reference(
    path: &Path,
    source: &str,
    span: SourceSpan,
    message: &str,
    label: &str,
) -> Error {
    Box::new(SidecarDiagnostic::InvalidSidecar {
        path: path.into(),
        message: message.into(),
        source_code: source_diagnostic::named_source(path, source, "text"),
        span,
        label: label.into(),
    })
    .into()
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
fn expression_span(expression: &TaggedValue, span: std::ops::Range<usize>) -> SourceSpan {
    let offset = expression.span.offset();
    (
        offset + span.start,
        span.end.saturating_sub(span.start).max(1),
    )
        .into()
}

fn expression_requirement_span(expression: &TaggedValue, span: std::ops::Range<u32>) -> SourceSpan {
    (
        expression.span.offset() + usize::try_from(span.start).expect("SPDX spans fit usize"),
        usize::try_from(span.end - span.start).expect("SPDX spans fit usize"),
    )
        .into()
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
