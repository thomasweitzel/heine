//! Kit manifest loading and compatibility-surface parsing.

use std::{collections::BTreeSet, fs, io, path::Path};

use miette::{NamedSource, SourceSpan};
use semver::{Version, VersionReq};
use serde::Deserialize;

use super::{
    ASSETS_DIRECTORY, Declaration, Error, ExtraSchema, KitName, ManifestDiagnostic, Paths,
    PublicSurface, ValueError,
};
use crate::{source_diagnostic::DiagnosticPaths, source_path::LogicalPath};

/// A validated manifest. Resource existence remains a domain-specific check:
/// templates, assets, and Fluent resources each own their own discovery rules.
#[derive(Debug, Clone)]
pub(crate) struct Manifest {
    pub(super) name: KitName,
    pub(super) name_span: SourceSpan,
    pub(super) version: Version,
    pub(super) heine_requirement: VersionReq,
    pub(super) heine_requirement_span: SourceSpan,
    pub(super) message_prefix: String,
    pub(super) public: PublicSurface,
    pub(super) extra: ExtraSchema,
    pub(super) source: Option<NamedSource<String>>,
}

impl Manifest {
    pub(super) fn load(paths: &Paths, diagnostics: &DiagnosticPaths) -> Result<Self, Error> {
        ensure_package_root(paths.root())?;
        ensure_manifest_file(paths.manifest())?;
        let source = match fs::read_to_string(paths.manifest()) {
            Ok(source) => source,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(Error::Missing {
                    path: paths.manifest().into(),
                });
            }
            Err(source) => {
                return Err(Error::Read {
                    path: paths.manifest().into(),
                    source,
                });
            }
        };
        let raw: RawManifest = toml::from_str(&source).map_err(|error| {
            Error::Invalid(Box::new(ManifestDiagnostic {
                message: error.message().to_owned(),
                source_code: diagnostics.named_source(paths.manifest(), source.clone(), "TOML"),
                span: error.span().unwrap_or(0..0).into(),
            }))
        })?;
        let extra = super::parse_schema(&source).map_err(|error| {
            Error::Invalid(Box::new(ManifestDiagnostic {
                message: error.message,
                source_code: diagnostics.named_source(paths.manifest(), source.clone(), "TOML"),
                span: error.span,
            }))
        })?;
        let mut manifest = Self::from_raw(raw, extra).map_err(|error| {
            Error::Invalid(Box::new(ManifestDiagnostic {
                message: error.message,
                source_code: diagnostics.named_source(paths.manifest(), source.clone(), "TOML"),
                span: error.span,
            }))
        })?;
        manifest.source = Some(diagnostics.named_source(paths.manifest(), source, "TOML"));
        Ok(manifest)
    }

    pub(crate) fn name(&self) -> &KitName {
        &self.name
    }

    pub(crate) fn message_prefix(&self) -> &str {
        &self.message_prefix
    }

    pub(crate) fn source(&self) -> Option<&NamedSource<String>> {
        self.source.as_ref()
    }

    pub(crate) fn public(&self) -> &PublicSurface {
        &self.public
    }

    pub(crate) fn extra(&self) -> &ExtraSchema {
        &self.extra
    }

    fn from_raw(raw: RawManifest, extra: ExtraSchema) -> Result<Self, ValueError> {
        let name_span: SourceSpan = raw.name.span().into();
        let name = KitName::parse(raw.name.into_inner()).map_err(|message| ValueError {
            span: name_span,
            message,
        })?;
        let version = parse_version(raw.version, "kit version")?;
        let heine_requirement_span = raw.heine.span().into();
        let heine_requirement = parse_requirement(raw.heine, "kit heine requirement")?;
        let message_prefix_span: SourceSpan = raw.message_prefix.span().into();
        let message_prefix = raw.message_prefix.into_inner();
        if message_prefix.is_empty() {
            return Err(ValueError {
                span: message_prefix_span,
                message: "kit message_prefix must not be empty".into(),
            });
        }
        let public = PublicSurface {
            templates: parse_paths(raw.public.templates, "public template", None)?,
            assets: parse_paths(raw.public.assets, "public asset", Some(ASSETS_DIRECTORY))?,
            components: parse_components(raw.public.components, &name)?,
        };
        Ok(Self {
            name,
            name_span,
            version,
            heine_requirement,
            heine_requirement_span,
            message_prefix,
            public,
            extra,
            source: None,
        })
    }
}

/// The manifest is the package entry point. Check its containing directory and
/// file before reading either one so a selected kit cannot redirect runtime
/// ownership through a symbolic link.
fn ensure_package_root(root: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::PackageRootSymlink { path: root.into() })
        }
        Ok(metadata) if !metadata.is_dir() => {
            Err(Error::PackageRootNotDirectory { path: root.into() })
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            Err(Error::MissingPackageRoot { path: root.into() })
        }
        Err(source) => Err(Error::InspectPackageRoot {
            path: root.into(),
            source,
        }),
    }
}

fn ensure_manifest_file(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::ManifestSymlink { path: path.into() })
        }
        Ok(metadata) if !metadata.is_file() => Err(Error::ManifestNotFile { path: path.into() }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            Err(Error::Missing { path: path.into() })
        }
        Err(source) => Err(Error::InspectManifest {
            path: path.into(),
            source,
        }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    name: toml::Spanned<String>,
    version: toml::Spanned<String>,
    heine: toml::Spanned<String>,
    message_prefix: toml::Spanned<String>,
    #[serde(default)]
    public: RawPublicSurface,
    #[serde(default, rename = "extra")]
    _extra: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPublicSurface {
    #[serde(default)]
    templates: Vec<toml::Spanned<String>>,
    #[serde(default)]
    assets: Vec<toml::Spanned<String>>,
    #[serde(default)]
    components: Vec<toml::Spanned<String>>,
}

pub(super) fn parse_requirement(
    value: toml::Spanned<String>,
    field: &str,
) -> Result<VersionReq, ValueError> {
    let span = value.span().into();
    let value = value.into_inner();
    VersionReq::parse(&value).map_err(|error| ValueError {
        span,
        message: format!("{field} {value:?} must be a semantic-version requirement: {error}"),
    })
}

fn parse_version(value: toml::Spanned<String>, field: &str) -> Result<Version, ValueError> {
    let span = value.span().into();
    let value = value.into_inner();
    Version::parse(&value).map_err(|error| ValueError {
        span,
        message: format!("{field} {value:?} must be a semantic version: {error}"),
    })
}

fn parse_paths(
    values: Vec<toml::Spanned<String>>,
    field: &str,
    required_prefix: Option<&str>,
) -> Result<Vec<Declaration<LogicalPath>>, ValueError> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| {
            let span = value.span().into();
            let value = value.into_inner();
            let path = LogicalPath::parse(&value).ok_or_else(|| ValueError {
                span,
                message: format!(
                    "{field} path {value:?} must be a non-empty portable relative path"
                ),
            })?;
            if let Some(prefix) = required_prefix {
                let has_member_below_prefix = path
                    .as_path()
                    .strip_prefix(prefix)
                    .is_ok_and(|relative| !relative.as_os_str().is_empty());
                if !has_member_below_prefix {
                    return Err(ValueError {
                        span,
                        message: format!("{field} path {value:?} must begin with {prefix:?}/"),
                    });
                }
            }
            if !seen.insert(path.clone()) {
                return Err(ValueError {
                    span,
                    message: format!("{field} path {value:?} is declared more than once"),
                });
            }
            Ok(Declaration { value: path, span })
        })
        .collect()
}

fn parse_components(
    values: Vec<toml::Spanned<String>>,
    name: &KitName,
) -> Result<Vec<Declaration<String>>, ValueError> {
    let prefix = format!("{}.", name.as_str());
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| {
            let span = value.span().into();
            let value = value.into_inner();
            let suffix = value.strip_prefix(&prefix).filter(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .split('.')
                        .all(crate::source_path::is_simple_identifier)
            });
            if suffix.is_none() {
                return Err(ValueError {
                    span,
                    message: format!(
                        "public component {value:?} must use the kit component namespace {prefix:?}"
                    ),
                });
            }
            if !seen.insert(value.clone()) {
                return Err(ValueError {
                    span,
                    message: format!("public component {value:?} is declared more than once"),
                });
            }
            Ok(Declaration { value, span })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(source: &str) -> Manifest {
        let raw: RawManifest = toml::from_str(source).expect("manifest TOML");
        let extra = crate::kit::parse_schema(source).expect("valid extra schema");
        Manifest::from_raw(raw, extra).expect("valid manifest")
    }

    #[test]
    fn parses_a_manifest_with_checked_public_surface_and_extra_schema() {
        let manifest = manifest(
            r#"
name = "paper"
version = "1.2.3"
heine = ">=0.8.0, <0.9.0"
message_prefix = "paper-"

[public]
templates = ["base.tera"]
assets = ["assets/site.css"]
components = ["paper.card"]

[extra]
accent = "violet"
show_search = true
spacing = [4, 8]
[extra.navigation]
compact = false
"#,
        );

        assert_eq!(manifest.name().as_str(), "paper");
        assert_eq!(manifest.version.to_string(), "1.2.3");
        assert!(manifest.heine_requirement.matches(&Version::new(0, 8, 0)));
        assert_eq!(
            manifest.public().templates()[0].value().as_str(),
            "base.tera"
        );
        assert_eq!(
            manifest.public().assets()[0].value().as_str(),
            "assets/site.css"
        );
        assert_eq!(manifest.public().components()[0].value(), "paper.card");
        assert_eq!(manifest.extra().len(), 4);
    }

    #[test]
    fn rejects_public_members_outside_their_declared_namespace() {
        let raw: RawManifest = toml::from_str(
            "name='paper'\nversion='1.0.0'\nheine='^0.8'\nmessage_prefix='paper-'\n[public]\nassets=['site.css']\n",
        )
        .expect("manifest TOML");
        let error = Manifest::from_raw(raw, ExtraSchema::default())
            .expect_err("asset must use assets namespace");
        assert!(error.message.contains("must begin with \"assets\""));
    }
}
