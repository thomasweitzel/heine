//! Fluent resource loading and deliberate locale fallback for templates.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use fluent_syntax::{ast::Entry, parser};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{locale::Locale, source_path};

pub(crate) type Arguments<'a> = FluentArgs<'a>;

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not walk localization directory {path}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("localization path {path} is a symbolic link")]
    Symlink { path: PathBuf },
    #[error("localization file {path} must use the .ftl extension")]
    Format { path: PathBuf },
    #[error("localization path {path} is not a directory")]
    NotDirectory { path: PathBuf },
    #[error("selected kit localization directory {path} is missing or is not a directory")]
    MissingKitDirectory { path: PathBuf },
    #[error("localization inventory did not contain configured locale {locale}")]
    MissingResolvedLocale { locale: Locale },
    #[error("localization file {path} is outside localization directory {root}")]
    OutsideDirectory { root: PathBuf, path: PathBuf },
    #[error(transparent)]
    #[diagnostic(transparent)]
    NormalizationCollision(#[from] Box<source_path::NormalizationCollision>),
    #[error("could not read localization file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not inspect localization path {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] Box<FluentDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Duplicate(#[from] Box<DuplicateDiagnostic>),
    #[error(transparent)]
    #[diagnostic(transparent)]
    KitMessage(#[from] Box<KitMessageDiagnostic>),
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid Fluent resource {path}: {message}")]
#[diagnostic(code(heine::localization::invalid))]
pub(crate) struct FluentDiagnostic {
    path: PathBuf,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("invalid Fluent syntax here")]
    span: SourceSpan,
}

#[derive(Debug, Diagnostic, Error)]
#[error("message {id:?} is declared more than once for locale {locale}")]
#[diagnostic(code(heine::localization::duplicate_message))]
pub(crate) struct DuplicateDiagnostic {
    id: String,
    locale: Locale,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("duplicate declaration here")]
    span: SourceSpan,
    #[related]
    first: Vec<FirstMessage>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("first declaration of this message")]
struct FirstMessage {
    #[source_code]
    source_code: NamedSource<String>,
    #[label("first declaration here")]
    span: SourceSpan,
}

#[derive(Clone)]
struct MessageSource {
    path: PathBuf,
    source: Arc<String>,
    span: SourceSpan,
}

struct LoadedResource {
    path: PathBuf,
    source: Arc<String>,
}

#[derive(Default)]
struct LocaleResources {
    messages: BTreeMap<String, MessageSource>,
    resources: Vec<LoadedResource>,
    latest_input: Option<SystemTime>,
}

#[derive(Debug, Diagnostic, Error)]
#[error("{message}")]
#[diagnostic(code(heine::kit::message))]
pub(crate) struct KitMessageDiagnostic {
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("message declaration here")]
    span: SourceSpan,
    #[help]
    help: String,
}

/// One Fluent entry's authored and bundle identities. Fluent gives messages
/// and terms different source spellings, but stores both in one namespace.
struct FluentEntry {
    id: String,
    bundle_id: String,
    span: SourceSpan,
}

/// Loaded message bundles. Heine performs the exact fallback itself: current
/// locale first, then the configured default locale, and no implicit chain.
pub(crate) struct Localization {
    bundles: BTreeMap<Locale, FluentBundle<FluentResource>>,
    message_sources: BTreeMap<Locale, BTreeMap<String, MessageSource>>,
    latest_inputs: BTreeMap<Locale, SystemTime>,
    default_locale: Locale,
}

impl std::fmt::Debug for Localization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Localization")
            .field("locales", &self.bundles.keys())
            .field("default_locale", &self.default_locale)
            .finish()
    }
}

impl Localization {
    pub(crate) fn load<'a>(
        root: &Path,
        selected_kit: Option<&crate::kit::Selected>,
        locales: impl IntoIterator<Item = &'a Locale>,
        default_locale: Locale,
        diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    ) -> Result<Self, Error> {
        let locales = locales.into_iter().cloned().collect::<Vec<_>>();
        let kit_root = selected_kit.map(|kit| kit.paths().locales());
        ensure_root_directory(root)?;
        if let Some(kit_root) = kit_root {
            ensure_kit_directory(kit_root)?;
        }

        let mut kit_resources = BTreeMap::new();
        let mut root_resources = BTreeMap::new();
        for locale in &locales {
            let kit = kit_root
                .map(|root| {
                    load_locale_resources(&root.join(locale.to_string()), locale, diagnostics)
                })
                .transpose()?
                .unwrap_or_default();
            if let Some(selected_kit) = selected_kit {
                validate_kit_messages(&kit, selected_kit.manifest().message_prefix())?;
            }
            kit_resources.insert(locale.clone(), kit);
            root_resources.insert(
                locale.clone(),
                load_locale_resources(&root.join(locale.to_string()), locale, diagnostics)?,
            );
        }
        let kit_ids = kit_resources
            .values()
            .flat_map(|resources| resources.messages.keys().cloned())
            .collect::<BTreeSet<_>>();

        let mut bundles = BTreeMap::new();
        let mut message_sources = BTreeMap::new();
        let mut latest_inputs = BTreeMap::new();
        for locale in locales {
            let kit = kit_resources
                .get(&locale)
                .ok_or_else(|| Error::MissingResolvedLocale {
                    locale: locale.clone(),
                })?;
            let root_layer =
                root_resources
                    .get(&locale)
                    .ok_or_else(|| Error::MissingResolvedLocale {
                        locale: locale.clone(),
                    })?;
            if let Some(selected_kit) = selected_kit {
                validate_root_overrides(
                    root_layer,
                    selected_kit.manifest().message_prefix(),
                    &kit_ids,
                )?;
            }
            let language = locale.to_string().parse().map_err(|error| {
                Error::Parse(Box::new(FluentDiagnostic {
                    path: root.into(),
                    message: format!("configured locale {locale} is not a Fluent locale: {error}"),
                    source_code: NamedSource::new("heine.toml", String::new()),
                    span: (0, 0).into(),
                }))
            })?;
            let mut bundle = FluentBundle::new_concurrent(vec![language]);
            let default_kit =
                kit_resources
                    .get(&default_locale)
                    .ok_or_else(|| Error::MissingResolvedLocale {
                        locale: default_locale.clone(),
                    })?;
            let default_root = root_resources.get(&default_locale).ok_or_else(|| {
                Error::MissingResolvedLocale {
                    locale: default_locale.clone(),
                }
            })?;
            add_resources(&mut bundle, &default_kit.resources)?;
            add_resources(&mut bundle, &default_root.resources)?;
            if locale != default_locale {
                add_resources(&mut bundle, &kit.resources)?;
                add_resources(&mut bundle, &root_layer.resources)?;
            }
            let latest_input = [
                default_kit.latest_input,
                default_root.latest_input,
                kit.latest_input,
                root_layer.latest_input,
            ]
            .into_iter()
            .flatten()
            .max();
            if let Some(latest_input) = latest_input {
                latest_inputs.insert(locale.clone(), latest_input);
            }
            let mut messages = default_kit.messages.clone();
            messages.extend(default_root.messages.clone());
            if locale != default_locale {
                messages.extend(kit.messages.clone());
                messages.extend(root_layer.messages.clone());
            }
            bundles.insert(locale.clone(), bundle);
            message_sources.insert(locale, messages);
        }
        Ok(Self {
            bundles,
            message_sources,
            latest_inputs,
            default_locale,
        })
    }

    pub(crate) fn format(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
        args: &Arguments<'_>,
    ) -> Result<String, String> {
        match self.format_in(locale, id, attribute, args) {
            Ok(value) => Ok(value),
            Err(FormatError::Invalid(message)) => Err(message),
            Err(FormatError::Missing) if locale == &self.default_locale => {
                Err(format!("message {id:?} is not defined for locale {locale}"))
            }
            Err(FormatError::Missing) => self
                .format_in(&self.default_locale, id, attribute, args)
                .map_err(|error| match error {
                    FormatError::Missing => format!(
                        "message {id:?} is not defined for locale {locale} or fallback locale {}",
                        self.default_locale
                    ),
                    FormatError::Invalid(message) => message,
                }),
        }
    }

    /// A current-locale resource can affect that locale; default-locale
    /// resources can also affect every other locale through documented
    /// fallback.
    pub(crate) fn latest_input(&self, locale: &Locale) -> Option<SystemTime> {
        let current = self.latest_inputs.get(locale).copied();
        let fallback = self.latest_inputs.get(&self.default_locale).copied();
        match (current, fallback) {
            (Some(current), Some(fallback)) => Some(current.max(fallback)),
            (Some(input), None) | (None, Some(input)) => Some(input),
            (None, None) => None,
        }
    }

    /// Returns the authored resource selected by the same current-locale then
    /// default-locale lookup rule used for message formatting.
    pub(crate) fn message_origin(
        &self,
        locale: &Locale,
        id: &str,
    ) -> Option<(PathBuf, String, SourceSpan)> {
        self.message_sources
            .get(locale)
            .and_then(|messages| messages.get(id))
            .or_else(|| {
                self.message_sources
                    .get(&self.default_locale)
                    .and_then(|messages| messages.get(id))
            })
            .map(|source| {
                (
                    source.path.clone(),
                    source.source.as_ref().clone(),
                    source.span,
                )
            })
    }

    fn format_in(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
        args: &Arguments<'_>,
    ) -> Result<String, FormatError> {
        let bundle = &self.bundles[locale];
        let message = bundle.get_message(id).ok_or(FormatError::Missing)?;
        let pattern = match attribute {
            Some(attribute) => message
                .get_attribute(attribute)
                .map(|attribute| attribute.value()),
            None => message.value(),
        }
        .ok_or(FormatError::Missing)?;
        let mut errors = Vec::new();
        let value = bundle
            .format_pattern(pattern, Some(args), &mut errors)
            .into_owned();
        if errors.is_empty() {
            Ok(value)
        } else {
            Err(FormatError::Invalid(
                errors
                    .into_iter()
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            ))
        }
    }
}

fn ensure_root_directory(root: &Path) -> Result<(), Error> {
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

fn ensure_kit_directory(root: &Path) -> Result<(), Error> {
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

fn load_locale_resources(
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
        let entries = message_entries(path, source.as_ref(), diagnostics)?;
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

fn validate_kit_messages(resources: &LocaleResources, prefix: &str) -> Result<(), Error> {
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

fn validate_root_overrides(
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

fn add_resources(
    bundle: &mut FluentBundle<FluentResource>,
    resources: &[LoadedResource],
) -> Result<(), Error> {
    for resource in resources {
        let parsed = FluentResource::try_new(resource.source.as_ref().clone()).map_err(
            |(_resource, errors)| {
                Error::Parse(Box::new(FluentDiagnostic {
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

fn message_entries(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<Vec<FluentEntry>, Error> {
    let resource = parser::parse(source).map_err(|(_, errors)| {
        let error = errors.first();
        Error::Parse(Box::new(FluentDiagnostic {
            path: diagnostics.display_path(path),
            message: error.map_or_else(
                || "Fluent parser rejected this resource".to_owned(),
                ToString::to_string,
            ),
            source_code: diagnostics.named_source(path, source, "Fluent"),
            span: error.map_or((0, 0).into(), |error| error.pos.clone().into()),
        }))
    })?;
    let identifiers = resource
        .body
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Message(message) => {
                Some((message.id.name.to_string(), message.id.name.to_string()))
            }
            Entry::Term(term) => Some((format!("-{}", term.id.name), term.id.name.to_string())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut offset = 0;
    identifiers
        .into_iter()
        .map(|(id, bundle_id)| {
            let (span, next_offset) = message_span(source, offset, &id).ok_or_else(|| {
                Error::Parse(Box::new(FluentDiagnostic {
                    path: diagnostics.display_path(path),
                    message: format!("could not locate parsed Fluent message {id:?}"),
                    source_code: diagnostics.named_source(path, source, "Fluent"),
                    span: (0, 0).into(),
                }))
            })?;
            offset = next_offset;
            Ok(FluentEntry {
                id,
                bundle_id,
                span,
            })
        })
        .collect()
}

/// Finds each parsed top-level entry after its predecessor. `split_inclusive`
/// preserves CRLF byte offsets, which is essential for miette source spans.
fn message_span(source: &str, offset: usize, id: &str) -> Option<(SourceSpan, usize)> {
    let mut line_offset = offset;
    for line in source[offset..].split_inclusive('\n') {
        let leading = line.len() - line.trim_start().len();
        let candidate = &line[leading..];
        if leading == 0
            && candidate
                .strip_prefix(id)
                .is_some_and(|tail| tail.starts_with('=') || tail.starts_with(char::is_whitespace))
        {
            return Some((
                (line_offset + leading, id.len()).into(),
                line_offset + line.len(),
            ));
        }
        line_offset += line.len();
    }
    None
}

/// Only absence participates in the documented current-locale-to-default
/// fallback. Formatting failures belong to the resource that defined them and
/// must remain visible instead of being hidden by a default-language value.
enum FormatError {
    Missing,
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_spans_only_match_top_level_fluent_declarations() {
        let source =
            "first =\n    duplicate = text inside a multiline value\nduplicate = actual message\n";

        let (span, _) = message_span(source, 0, "duplicate").expect("message declaration");

        assert_eq!(span.offset(), source.find("duplicate = actual").unwrap());
        assert_eq!(span.len(), "duplicate".len());
    }
}
