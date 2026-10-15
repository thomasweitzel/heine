//! Filesystem-backed Fluent resource layers and kit ownership checks.

use std::{collections::BTreeSet, fs, io, path::Path, sync::Arc};

use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use miette::NamedSource;
use walkdir::WalkDir;

use crate::{locale::Locale, source_path};

use super::{
    DuplicateDiagnostic, Error, FirstMessage, KitMessageDiagnostic, LoadedResource,
    LocaleResources, MessageSource, message_entries,
};

pub(super) fn ensure_root_directory(root: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::Symlink { path: root.into() })
        }
        Ok(metadata) if !metadata.is_dir() => Err(Error::NotDirectory { path: root.into() }),
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Inspect {
            path: root.into(),
            source,
        }),
    }
}

pub(super) fn ensure_kit_directory(root: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::Symlink { path: root.into() })
        }
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Error::MissingKitDirectory { path: root.into() }),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            Err(Error::MissingKitDirectory { path: root.into() })
        }
        Err(source) => Err(Error::Inspect {
            path: root.into(),
            source,
        }),
    }
}

pub(super) fn load_locale_resources(
    directory: &Path,
    locale: &Locale,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<LocaleResources, Error> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Error::Symlink {
                path: directory.into(),
            });
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(Error::NotDirectory {
                path: directory.into(),
            });
        }
        Ok(_) => true,
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(Error::Inspect {
                path: directory.into(),
                source,
            });
        }
    };
    if !metadata {
        return Ok(LocaleResources::default());
    }
    let entries = WalkDir::new(directory)
        .sort_by_file_name()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Walk {
            path: directory.into(),
            source,
        })?;
    for entry in &entries {
        if entry.file_type().is_symlink() {
            return Err(Error::Symlink {
                path: entry.path().into(),
            });
        }
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "ftl")
        {
            return Err(Error::Format {
                path: entry.path().into(),
            });
        }
    }
    let paths = entries
        .iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let path = entry.path().to_path_buf();
            let relative = path
                .strip_prefix(directory)
                .map(Path::to_path_buf)
                .map_err(|_| Error::OutsideDirectory {
                    root: directory.into(),
                    path: path.clone(),
                })?;
            Ok((relative, path))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if let Some(collision) = source_path::normalization_collision(paths) {
        return Err(Error::NormalizationCollision(Box::new(collision)));
    }

    let mut loaded = LocaleResources::default();
    for entry in entries.iter().filter(|entry| entry.file_type().is_file()) {
        let path = entry.path();
        let source = Arc::new(fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.into(),
            source,
        })?);
        let modified = fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .map_err(|source| Error::Inspect {
                path: path.into(),
                source,
            })?;
        loaded.latest_input = Some(
            loaded
                .latest_input
                .map_or(modified, |latest| latest.max(modified)),
        );
        let (entries, parsed) = message_entries(path, source.as_ref(), diagnostics)?;
        for entry in &entries {
            if let Some(first) = loaded.messages.get(&entry.bundle_id) {
                return Err(Error::Duplicate(Box::new(DuplicateDiagnostic {
                    id: entry.id.clone(),
                    locale: locale.clone(),
                    source_code: diagnostics.named_source(path, source.as_ref(), "Fluent"),
                    span: entry.span,
                    first: vec![FirstMessage {
                        source_code: NamedSource::new(
                            first.path.display().to_string(),
                            first.source.as_ref().clone(),
                        ),
                        span: first.span,
                    }],
                })));
            }
            loaded.messages.insert(
                entry.bundle_id.clone(),
                MessageSource {
                    path: diagnostics.display_path(path),
                    source: source.clone(),
                    parsed: parsed.clone(),
                    span: entry.span,
                },
            );
        }
        loaded.resources.push(LoadedResource {
            path: diagnostics.display_path(path),
            source,
        });
    }
    Ok(loaded)
}

pub(super) fn validate_kit_messages(
    resources: &LocaleResources,
    prefix: &str,
) -> Result<(), Error> {
    for (id, source) in &resources.messages {
        let name = id.strip_prefix('-').unwrap_or(id);
        if !name.starts_with(prefix) {
            return Err(Box::new(KitMessageDiagnostic {
                message: format!("kit Fluent message or term {id:?} must begin with {prefix:?}"),
                source_code: NamedSource::new(
                    source.path.display().to_string(),
                    source.source.as_ref().clone(),
                ),
                span: source.span,
                help:
                    "rename this kit interface message or term to use the manifest message_prefix"
                        .into(),
            })
            .into());
        }
    }
    Ok(())
}

pub(super) fn validate_root_overrides(
    resources: &LocaleResources,
    prefix: &str,
    kit_ids: &BTreeSet<String>,
) -> Result<(), Error> {
    for (id, source) in &resources.messages {
        if id.strip_prefix('-').unwrap_or(id).starts_with(prefix) && !kit_ids.contains(id) {
            return Err(Box::new(KitMessageDiagnostic {
                message: format!("root Fluent message or term {id:?} does not override a kit declaration"),
                source_code: NamedSource::new(
                    source.path.display().to_string(),
                    source.source.as_ref().clone(),
                ),
                span: source.span,
                help: "use an existing prefixed kit message or term, or choose a root-owned message ID".into(),
            }).into());
        }
    }
    Ok(())
}

pub(super) fn add_resources(
    bundle: &mut FluentBundle<FluentResource>,
    resources: &[LoadedResource],
) -> Result<(), Error> {
    for resource in resources {
        let parsed = FluentResource::try_new(resource.source.as_ref().clone()).map_err(
            |(_resource, errors)| {
                Error::Parse(Box::new(super::FluentDiagnostic {
                    path: resource.path.clone(),
                    message: errors.first().map_or_else(
                        || "Fluent parser rejected this resource".to_owned(),
                        ToString::to_string,
                    ),
                    source_code: NamedSource::new(
                        resource.path.display().to_string(),
                        resource.source.as_ref().clone(),
                    ),
                    span: errors
                        .first()
                        .map_or((0, 0).into(), |error| error.pos.clone().into()),
                }))
            },
        )?;
        // Each layer has already rejected its own duplicates. Between layers,
        // this order is the documented ownership and fallback contract.
        bundle.add_resource_overriding(parsed);
    }
    Ok(())
}
