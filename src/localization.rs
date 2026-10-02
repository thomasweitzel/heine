//! Fluent resource loading and deliberate locale fallback for templates.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use fluent_bundle::{FluentArgs, FluentResource, FluentValue, concurrent::FluentBundle};
use fluent_syntax::{
    ast::{CallArguments, Entry, Expression, InlineExpression, Pattern, PatternElement},
    parser,
};
use intl_memoizer::{Memoizable, concurrent::IntlLangMemoizer};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{locale::Locale, source_path};

pub(crate) mod number;

type SelectorBindings = BTreeMap<String, Option<String>>;
type SelectorStack = BTreeSet<(Locale, String, Option<String>, SelectorBindings)>;

/// Fluent provides the resolved bundle language through its memoizer. The
/// marker lets Heine retain that language without tying locale formatting to
/// process-global state.
struct FluentLocale(unic_langid::LanguageIdentifier);

impl Memoizable for FluentLocale {
    type Args = ();
    type Error = Infallible;

    fn construct(
        language: unic_langid::LanguageIdentifier,
        (): Self::Args,
    ) -> Result<Self, Self::Error> {
        Ok(Self(language))
    }
}

fn format_fluent_number(value: &FluentValue<'_>, memoizer: &IntlLangMemoizer) -> Option<String> {
    let FluentValue::Number(value) = value else {
        return None;
    };
    memoizer
        .with_try_get::<FluentLocale, _, _>((), |locale| {
            let locale = locale.0.to_string().parse::<Locale>().ok()?;
            let number = number::LocalizedNumber::from_f64(value.value).ok()?;
            let minimum = value.options.minimum_fraction_digits.unwrap_or(0);
            let maximum = value.options.maximum_fraction_digits.unwrap_or(3);
            let minimum = u16::try_from(minimum).ok()?;
            let maximum = u16::try_from(maximum).ok()?;
            let options =
                number::Options::new(value.options.use_grouping, minimum, maximum).ok()?;
            number::format(&locale, &number, options).ok()
        })
        .ok()
        .flatten()
}

/// Template values remain checked until Heine has resolved the Fluent message
/// that will consume them. Fluent itself uses `f64` selectors, so selector and
/// display values deliberately cross that boundary at different points.
#[derive(Default)]
pub(crate) struct Arguments {
    values: BTreeMap<String, Argument>,
}

pub(crate) enum Argument {
    Text(String),
    Number(number::LocalizedNumber),
}

impl Arguments {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set<V>(&mut self, key: impl Into<String>, value: V)
    where
        V: Into<Argument>,
    {
        self.values.insert(key.into(), value.into());
    }

    pub(crate) fn set_number(&mut self, key: impl Into<String>, value: number::LocalizedNumber) {
        self.values.insert(key.into(), Argument::Number(value));
    }

    fn fluent(&self, selectors: &BTreeSet<String>) -> Result<FluentArgs<'static>, String> {
        let mut arguments = FluentArgs::with_capacity(self.values.len());
        for (key, value) in &self.values {
            match value {
                Argument::Text(value) => arguments.set(key.clone(), value.clone()),
                Argument::Number(value) => {
                    let value = if selectors.contains(key) {
                        value.normalized(number::Options::default())
                    } else {
                        value.clone()
                    };
                    arguments.set(
                        key.clone(),
                        value.fluent_value().map_err(|error| error.to_string())?,
                    );
                }
            }
        }
        Ok(arguments)
    }
}

impl From<String> for Argument {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Argument {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<i64> for Argument {
    fn from(value: i64) -> Self {
        Self::Number(number::LocalizedNumber::from_i64(value))
    }
}

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
    #[error("could not register Fluent built-in formatting functions: {message}")]
    FluentSetup { message: String },
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
    message_owners: BTreeMap<Locale, BTreeMap<String, Locale>>,
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
        let mut message_owners = BTreeMap::new();
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
            bundle.set_formatter(Some(format_fluent_number));
            bundle.add_builtins().map_err(|error| Error::FluentSetup {
                message: error.to_string(),
            })?;
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
            let mut owners = messages
                .keys()
                .cloned()
                .map(|id| (id, default_locale.clone()))
                .collect::<BTreeMap<_, _>>();
            if locale != default_locale {
                messages.extend(kit.messages.clone());
                messages.extend(root_layer.messages.clone());
                owners.extend(
                    kit.messages
                        .keys()
                        .chain(root_layer.messages.keys())
                        .cloned()
                        .map(|id| (id, locale.clone())),
                );
            }
            bundles.insert(locale.clone(), bundle);
            message_sources.insert(locale.clone(), messages);
            message_owners.insert(locale, owners);
        }
        Ok(Self {
            bundles,
            message_sources,
            message_owners,
            latest_inputs,
            default_locale,
        })
    }

    pub(crate) fn format(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
        args: &Arguments,
    ) -> Result<String, String> {
        let owner = self.format_owner(locale, id, attribute);
        let selectors = self.selector_variables(&owner, id, attribute);
        let args = args.fluent(&selectors).map_err(|error| error.to_string())?;
        match self.format_in(&owner, id, attribute, &args) {
            Ok(value) => Ok(value),
            Err(FormatError::Invalid(message)) => Err(message),
            Err(FormatError::Missing) if owner == self.default_locale => {
                Err(format!("message {id:?} is not defined for locale {locale}"))
            }
            Err(FormatError::Missing) => self
                .format_in(&self.default_locale, id, attribute, &args)
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
        args: &FluentArgs<'_>,
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

    fn selector_variables(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
    ) -> BTreeSet<String> {
        let mut selectors = BTreeSet::new();
        self.collect_selector_variables(
            locale,
            id,
            attribute,
            &BTreeMap::new(),
            &mut BTreeSet::new(),
            &mut selectors,
        );
        selectors
    }

    fn format_owner(&self, locale: &Locale, id: &str, attribute: Option<&str>) -> Locale {
        let owner = self
            .message_owners
            .get(locale)
            .and_then(|owners| owners.get(id))
            .unwrap_or(&self.default_locale);
        if attribute.is_none() || self.has_message_pattern(owner, id, attribute) {
            owner.clone()
        } else {
            self.default_locale.clone()
        }
    }

    fn has_message_pattern(&self, locale: &Locale, id: &str, attribute: Option<&str>) -> bool {
        let Some(source) = self
            .message_sources
            .get(locale)
            .and_then(|messages| messages.get(id))
        else {
            return false;
        };
        let Ok(resource) = parser::parse(source.source.as_ref().clone()) else {
            return false;
        };
        fluent_pattern(&resource.body, id, attribute).is_some()
    }

    fn collect_selector_variables(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
        bindings: &SelectorBindings,
        stack: &mut SelectorStack,
        selectors: &mut BTreeSet<String>,
    ) {
        let key = (
            locale.clone(),
            id.to_owned(),
            attribute.map(str::to_owned),
            bindings.clone(),
        );
        if !stack.insert(key.clone()) {
            return;
        }

        let Some(source) = self
            .message_sources
            .get(locale)
            .and_then(|messages| messages.get(id))
        else {
            return;
        };
        let Ok(resource) = parser::parse(source.source.as_ref().clone()) else {
            return;
        };
        if let Some(pattern) = fluent_pattern(&resource.body, id, attribute) {
            self.collect_selector_pattern(locale, pattern, bindings, stack, selectors);
        }
        stack.remove(&key);
    }

    fn collect_selector_pattern(
        &self,
        locale: &Locale,
        pattern: &Pattern<String>,
        bindings: &SelectorBindings,
        stack: &mut SelectorStack,
        selectors: &mut BTreeSet<String>,
    ) {
        for element in &pattern.elements {
            let PatternElement::Placeable { expression } = element else {
                continue;
            };
            self.collect_selector_expression(locale, expression, bindings, stack, selectors);
        }
    }

    fn collect_selector_expression(
        &self,
        locale: &Locale,
        expression: &Expression<String>,
        bindings: &SelectorBindings,
        stack: &mut SelectorStack,
        selectors: &mut BTreeSet<String>,
    ) {
        match expression {
            Expression::Inline(expression) => {
                self.collect_selector_inline(locale, expression, bindings, stack, selectors)
            }
            Expression::Select { selector, variants } => {
                if let InlineExpression::VariableReference { id } = selector {
                    if let Some(variable) = resolve_binding(bindings, id.name.as_ref()) {
                        selectors.insert(variable);
                    }
                }
                self.collect_selector_inline(locale, selector, bindings, stack, selectors);
                for variant in variants {
                    self.collect_selector_pattern(
                        locale,
                        &variant.value,
                        bindings,
                        stack,
                        selectors,
                    );
                }
            }
        }
    }

    fn collect_selector_inline(
        &self,
        locale: &Locale,
        expression: &InlineExpression<String>,
        bindings: &SelectorBindings,
        stack: &mut SelectorStack,
        selectors: &mut BTreeSet<String>,
    ) {
        match expression {
            InlineExpression::MessageReference { id, attribute } => self
                .collect_selector_variables(
                    locale,
                    id.name.as_ref(),
                    attribute.as_ref().map(|attribute| attribute.name.as_ref()),
                    bindings,
                    stack,
                    selectors,
                ),
            InlineExpression::TermReference {
                id,
                attribute,
                arguments,
            } => {
                let mut term_bindings = bindings.clone();
                if let Some(arguments) = arguments {
                    for argument in &arguments.named {
                        let binding = match &argument.value {
                            InlineExpression::VariableReference { id } => {
                                resolve_binding(bindings, id.name.as_ref())
                            }
                            _ => None,
                        };
                        term_bindings.insert(argument.name.name.to_string(), binding);
                    }
                    self.collect_selector_arguments(locale, arguments, bindings, stack, selectors);
                }
                self.collect_selector_variables(
                    locale,
                    &format!("-{}", id.name),
                    attribute.as_ref().map(|attribute| attribute.name.as_ref()),
                    &term_bindings,
                    stack,
                    selectors,
                );
            }
            InlineExpression::FunctionReference { arguments, .. } => {
                self.collect_selector_arguments(locale, arguments, bindings, stack, selectors)
            }
            InlineExpression::Placeable { expression } => {
                self.collect_selector_expression(locale, expression, bindings, stack, selectors)
            }
            InlineExpression::StringLiteral { .. }
            | InlineExpression::NumberLiteral { .. }
            | InlineExpression::VariableReference { .. } => {}
        }
    }

    fn collect_selector_arguments(
        &self,
        locale: &Locale,
        arguments: &CallArguments<String>,
        bindings: &SelectorBindings,
        stack: &mut SelectorStack,
        selectors: &mut BTreeSet<String>,
    ) {
        for argument in &arguments.positional {
            self.collect_selector_inline(locale, argument, bindings, stack, selectors);
        }
        for argument in &arguments.named {
            self.collect_selector_inline(locale, &argument.value, bindings, stack, selectors);
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
    validate_number_calls(&resource.body, path, source, diagnostics)?;
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

/// Heine deliberately accepts only the decimal `NUMBER()` options that its
/// shared formatter implements. Fluent otherwise ignores unsupported named
/// options, which would turn a misspelling into silently different output.
fn validate_number_calls(
    entries: &[Entry<&str>],
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    for entry in entries {
        match entry {
            Entry::Message(message) => {
                validate_number_pattern(message.value.as_ref(), path, source, diagnostics)?;
                for attribute in &message.attributes {
                    validate_number_pattern(Some(&attribute.value), path, source, diagnostics)?;
                }
            }
            Entry::Term(term) => {
                validate_number_pattern(Some(&term.value), path, source, diagnostics)?;
                for attribute in &term.attributes {
                    validate_number_pattern(Some(&attribute.value), path, source, diagnostics)?;
                }
            }
            Entry::Junk { .. }
            | Entry::Comment(_)
            | Entry::GroupComment(_)
            | Entry::ResourceComment(_) => {}
        }
    }
    Ok(())
}

fn fluent_pattern<'a>(
    entries: &'a [Entry<String>],
    id: &str,
    attribute: Option<&str>,
) -> Option<&'a Pattern<String>> {
    entries.iter().find_map(|entry| match entry {
        Entry::Message(message) if message.id.name == id => match attribute {
            Some(attribute) => message
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute)
                .map(|attribute| &attribute.value),
            None => message.value.as_ref(),
        },
        Entry::Term(term) if id.strip_prefix('-') == Some(term.id.name.as_ref()) => match attribute
        {
            Some(attribute) => term
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute)
                .map(|attribute| &attribute.value),
            None => Some(&term.value),
        },
        _ => None,
    })
}

fn resolve_binding(bindings: &BTreeMap<String, Option<String>>, variable: &str) -> Option<String> {
    bindings
        .get(variable)
        .cloned()
        .unwrap_or_else(|| Some(variable.to_owned()))
}

fn validate_number_pattern(
    pattern: Option<&Pattern<&str>>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    let Some(pattern) = pattern else {
        return Ok(());
    };
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            validate_number_expression(expression, path, source, diagnostics)?;
        }
    }
    Ok(())
}

fn validate_number_expression(
    expression: &Expression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    match expression {
        Expression::Inline(expression) => {
            validate_number_inline(expression, path, source, diagnostics)
        }
        Expression::Select { selector, variants } => {
            validate_number_inline(selector, path, source, diagnostics)?;
            let selector = match selector {
                InlineExpression::VariableReference { id } => Some(id.name),
                _ => None,
            };
            for variant in variants {
                validate_number_pattern(Some(&variant.value), path, source, diagnostics)?;
                if let Some(selector) = selector {
                    validate_selector_presentations(
                        &variant.value,
                        selector,
                        path,
                        source,
                        diagnostics,
                    )?;
                }
            }
            Ok(())
        }
    }
}

fn validate_selector_presentations(
    pattern: &Pattern<&str>,
    selector: &str,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    for element in &pattern.elements {
        let PatternElement::Placeable { expression } = element else {
            continue;
        };
        match expression {
            Expression::Inline(InlineExpression::FunctionReference { id, arguments })
                if id.name == "NUMBER" =>
            {
                let selected = matches!(arguments.positional.as_slice(), [InlineExpression::VariableReference { id }] if id.name == selector);
                if selected && !number_arguments_use_default_precision(arguments) {
                    return Err(invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "NUMBER() for a plural selector must use the default fraction-digit policy",
                        selector_precision_option(arguments),
                    ));
                }
            }
            Expression::Select { variants, .. } => {
                for variant in variants {
                    validate_selector_presentations(
                        &variant.value,
                        selector,
                        path,
                        source,
                        diagnostics,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn number_arguments_use_default_precision(arguments: &CallArguments<&str>) -> bool {
    arguments.named.iter().all(|argument| match argument.name.name {
        "minimumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if *value == "0"),
        "maximumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if *value == "3"),
        _ => true,
    })
}

fn selector_precision_option(arguments: &CallArguments<&str>) -> Option<&'static str> {
    arguments.named.iter().find_map(|argument| match argument.name.name {
        "minimumFractionDigits"
            if !matches!(&argument.value, InlineExpression::NumberLiteral { value } if *value == "0") =>
        {
            Some("minimumFractionDigits")
        }
        "maximumFractionDigits"
            if !matches!(&argument.value, InlineExpression::NumberLiteral { value } if *value == "3") =>
        {
            Some("maximumFractionDigits")
        }
        _ => None,
    })
}

fn validate_number_inline(
    expression: &InlineExpression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    match expression {
        InlineExpression::FunctionReference { id, arguments } if id.name == "NUMBER" => {
            validate_number_arguments(arguments, path, source, diagnostics)
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            validate_number_arguments_inside(arguments, path, source, diagnostics)
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => validate_number_arguments_inside(arguments, path, source, diagnostics),
        InlineExpression::Placeable { expression } => {
            validate_number_expression(expression, path, source, diagnostics)
        }
        _ => Ok(()),
    }
}

fn validate_number_arguments_inside(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    for argument in &arguments.positional {
        validate_number_inline(argument, path, source, diagnostics)?;
    }
    for argument in &arguments.named {
        validate_number_inline(&argument.value, path, source, diagnostics)?;
    }
    Ok(())
}

fn validate_number_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    let invalid = || invalid_number_call(path, source, diagnostics, "invalid NUMBER() call", None);
    if arguments.positional.len() != 1 {
        return Err(invalid());
    }
    let mut minimum = 0;
    let mut maximum = 3;
    for argument in &arguments.named {
        match argument.name.name {
            "useGrouping" => match &argument.value {
                InlineExpression::StringLiteral { value } if matches!(*value, "true" | "false") => {
                }
                _ => {
                    return Err(invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "useGrouping must be the string literal \"true\" or \"false\"",
                        Some("useGrouping"),
                    ));
                }
            },
            "minimumFractionDigits" => {
                minimum = number_option(&argument.value).ok_or_else(|| {
                    invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "minimumFractionDigits must be a supported non-negative integer",
                        Some("minimumFractionDigits"),
                    )
                })?;
            }
            "maximumFractionDigits" => {
                maximum = number_option(&argument.value).ok_or_else(|| {
                    invalid_number_call(
                        path,
                        source,
                        diagnostics,
                        "maximumFractionDigits must be a supported non-negative integer",
                        Some("maximumFractionDigits"),
                    )
                })?;
            }
            _ => {
                return Err(invalid_number_call(
                    path,
                    source,
                    diagnostics,
                    "NUMBER() supports only useGrouping, minimumFractionDigits, and maximumFractionDigits",
                    Some(argument.name.name),
                ));
            }
        }
    }
    number::Options::new(true, minimum, maximum).map_err(|_| {
        invalid_number_call(
            path,
            source,
            diagnostics,
            "minimumFractionDigits cannot exceed maximumFractionDigits",
            Some("maximumFractionDigits"),
        )
    })?;
    validate_number_arguments_inside(arguments, path, source, diagnostics)
}

fn invalid_number_call(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    message: &str,
    option: Option<&str>,
) -> Error {
    let offset = option
        .and_then(|option| source.rfind(option))
        .and_then(|option_offset| source[..option_offset].rfind("NUMBER("))
        .or_else(|| source.rfind("NUMBER("))
        .unwrap_or(0);
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span: (offset, "NUMBER".len()).into(),
    }))
}

fn number_option(value: &InlineExpression<&str>) -> Option<u16> {
    let InlineExpression::NumberLiteral { value } = value else {
        return None;
    };
    value
        .parse::<u16>()
        .ok()
        .filter(|value| *value <= number::MAX_FRACTION_DIGITS)
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
