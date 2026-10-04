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
    ast::{CallArguments, Entry, Expression, InlineExpression, Pattern, PatternElement, Resource},
    parser,
};
use intl_memoizer::{Memoizable, concurrent::IntlLangMemoizer};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{locale::Locale, source_path};

pub(crate) mod number;
pub(crate) mod temporal;

type SelectorBindings = BTreeMap<String, Option<String>>;
type SelectorStack = BTreeSet<(Locale, String, Option<String>, SelectorBindings)>;
type SelectorPresentations = BTreeMap<String, FluentPresentation>;
type TemporalPresentations = BTreeMap<String, BTreeSet<temporal::Presentation>>;

#[derive(Default)]
struct SelectorAnalysis {
    stack: SelectorStack,
    selectors: BTreeSet<String>,
    presentations: SelectorPresentations,
    temporal_presentations: TemporalPresentations,
}

#[derive(Clone, Copy)]
struct SelectorTarget<'a> {
    id: &'a str,
    attribute: Option<&'a str>,
}

#[derive(Clone)]
struct FluentPresentation {
    message: String,
    path: PathBuf,
    source: Arc<String>,
    span: SourceSpan,
}

impl FluentPresentation {
    fn location(&self) -> String {
        let offset = self.span.offset();
        let before = &self.source[..offset];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before
            .rsplit_once('\n')
            .map_or(before.chars().count() + 1, |(_, line)| {
                line.chars().count() + 1
            });
        format!("{}:{line}:{column}", self.path.display())
    }
}

/// The closed Fluent function surface that Heine registers and validates.
/// Adding a formatter extends this enum, rather than separate registration and
/// validation lists.
#[derive(Clone, Copy, Eq, PartialEq)]
enum FluentFunction {
    Number,
    DateTime,
}

impl FluentFunction {
    const SUPPORTED: [Self; 2] = [Self::Number, Self::DateTime];

    fn parse(name: &str) -> Option<Self> {
        match name {
            "NUMBER" => Some(Self::Number),
            "DATETIME" => Some(Self::DateTime),
            _ => None,
        }
    }

    fn install(
        bundle: &mut FluentBundle<FluentResource>,
        locale: Locale,
        editorial_time_zone: Option<crate::config::EditorialTimeZone>,
    ) -> Result<(), fluent_bundle::FluentError> {
        for function in Self::SUPPORTED {
            function.register(bundle, locale.clone(), editorial_time_zone.clone())?;
        }
        Ok(())
    }

    fn register(
        self,
        bundle: &mut FluentBundle<FluentResource>,
        locale: Locale,
        editorial_time_zone: Option<crate::config::EditorialTimeZone>,
    ) -> Result<(), fluent_bundle::FluentError> {
        match self {
            // fluent-bundle 0.16 registers only NUMBER through this API.
            Self::Number => bundle.add_builtins(),
            Self::DateTime => bundle.add_function("DATETIME", move |positional, named| {
                temporal::format_fluent(&locale, editorial_time_zone.as_ref(), positional, named)
                    .map(Into::into)
                    .unwrap_or(fluent_bundle::FluentValue::Error)
            }),
        }
    }
}

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

    fn fluent(
        &self,
        selectors: &BTreeSet<String>,
        temporal_presentations: &TemporalPresentations,
        editorial_time_zone: Option<&crate::config::EditorialTimeZone>,
    ) -> Result<FluentArgs<'static>, String> {
        self.validate_temporal(temporal_presentations, editorial_time_zone)?;
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

    fn validate_temporal(
        &self,
        presentations: &TemporalPresentations,
        editorial_time_zone: Option<&crate::config::EditorialTimeZone>,
    ) -> Result<(), String> {
        for (name, presentations) in presentations {
            let Some(value) = self.values.get(name) else {
                continue;
            };
            let Argument::Text(value) = value else {
                return Err(format!(
                    "DATETIME() argument {name:?} must be an RFC 3339 instant or ISO calendar date string"
                ));
            };
            for presentation in presentations {
                presentation
                    .validate(editorial_time_zone, value)
                    .map_err(|error| format!("DATETIME() argument {name:?}: {error}"))?;
            }
        }
        Ok(())
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
    #[label("problem in this Fluent source")]
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
    parsed: Arc<Resource<String>>,
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
    editorial_time_zone: Option<crate::config::EditorialTimeZone>,
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
        editorial_time_zone: Option<crate::config::EditorialTimeZone>,
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
            FluentFunction::install(&mut bundle, locale.clone(), editorial_time_zone.clone())
                .map_err(|error| Error::FluentSetup {
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
            editorial_time_zone,
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
        let analysis = self.formatting_analysis(&owner, id, attribute)?;
        let args = args
            .fluent(
                &analysis.selectors,
                &analysis.temporal_presentations,
                self.editorial_time_zone.as_ref(),
            )
            .map_err(|error| error.to_string())?;
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

    fn formatting_analysis(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
    ) -> Result<SelectorAnalysis, String> {
        let mut analysis = SelectorAnalysis::default();
        self.collect_selector_variables(
            locale,
            SelectorTarget { id, attribute },
            &BTreeMap::new(),
            &mut analysis,
        );
        if let Some(presentation) =
            analysis
                .presentations
                .iter()
                .find_map(|(variable, presentation)| {
                    analysis
                        .selectors
                        .contains(variable)
                        .then_some(presentation)
                })
        {
            return Err(format!(
                "NUMBER() for a plural selector must use the default fraction-digit policy; the referenced Fluent message {:?} declares a conflicting presentation at {}",
                presentation.message,
                presentation.location(),
            ));
        }
        Ok(analysis)
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
        fluent_pattern(&source.parsed.body, id, attribute).is_some()
    }

    fn collect_selector_variables(
        &self,
        locale: &Locale,
        target: SelectorTarget<'_>,
        bindings: &SelectorBindings,
        analysis: &mut SelectorAnalysis,
    ) {
        let key = (
            locale.clone(),
            target.id.to_owned(),
            target.attribute.map(str::to_owned),
            bindings.clone(),
        );
        if !analysis.stack.insert(key.clone()) {
            return;
        }

        let Some(source) = self
            .message_sources
            .get(locale)
            .and_then(|messages| messages.get(target.id))
        else {
            return;
        };
        let presentation = FluentPresentation {
            message: target.id.to_owned(),
            path: source.path.clone(),
            source: source.source.clone(),
            span: source.span,
        };
        if let Some(pattern) = fluent_pattern(&source.parsed.body, target.id, target.attribute) {
            self.collect_selector_pattern(locale, pattern, bindings, analysis, &presentation);
        }
        analysis.stack.remove(&key);
    }

    fn collect_selector_pattern(
        &self,
        locale: &Locale,
        pattern: &Pattern<String>,
        bindings: &SelectorBindings,
        analysis: &mut SelectorAnalysis,
        presentation: &FluentPresentation,
    ) {
        for element in &pattern.elements {
            let PatternElement::Placeable { expression } = element else {
                continue;
            };
            self.collect_selector_expression(locale, expression, bindings, analysis, presentation);
        }
    }

    fn collect_selector_expression(
        &self,
        locale: &Locale,
        expression: &Expression<String>,
        bindings: &SelectorBindings,
        analysis: &mut SelectorAnalysis,
        presentation: &FluentPresentation,
    ) {
        match expression {
            Expression::Inline(expression) => {
                self.collect_selector_inline(locale, expression, bindings, analysis, presentation)
            }
            Expression::Select { selector, variants } => {
                if let InlineExpression::VariableReference { id } = selector {
                    if let Some(variable) = resolve_binding(bindings, id.name.as_ref()) {
                        analysis.selectors.insert(variable);
                    }
                }
                self.collect_selector_inline(locale, selector, bindings, analysis, presentation);
                for variant in variants {
                    self.collect_selector_pattern(
                        locale,
                        &variant.value,
                        bindings,
                        analysis,
                        presentation,
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
        analysis: &mut SelectorAnalysis,
        presentation: &FluentPresentation,
    ) {
        match expression {
            InlineExpression::MessageReference { id, attribute } => self
                .collect_selector_variables(
                    locale,
                    SelectorTarget {
                        id: id.name.as_ref(),
                        attribute: attribute.as_ref().map(|attribute| attribute.name.as_ref()),
                    },
                    bindings,
                    analysis,
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
                    self.collect_selector_arguments(
                        locale,
                        arguments,
                        bindings,
                        analysis,
                        presentation,
                    );
                }
                self.collect_selector_variables(
                    locale,
                    SelectorTarget {
                        id: &format!("-{}", id.name),
                        attribute: attribute.as_ref().map(|attribute| attribute.name.as_ref()),
                    },
                    &term_bindings,
                    analysis,
                );
            }
            InlineExpression::FunctionReference { id, arguments } => {
                if id.name == "NUMBER"
                    && let [InlineExpression::VariableReference { id }] =
                        arguments.positional.as_slice()
                    && let Some(variable) = resolve_binding(bindings, id.name.as_ref())
                    && !number_arguments_use_default_precision(arguments)
                {
                    analysis
                        .presentations
                        .entry(variable)
                        .or_insert_with(|| presentation.clone());
                }
                if id.name == "DATETIME"
                    && let [InlineExpression::VariableReference { id }] =
                        arguments.positional.as_slice()
                    && let Some(variable) = resolve_binding(bindings, id.name.as_ref())
                    && let Some(presentation) = datetime_presentation(arguments)
                {
                    analysis
                        .temporal_presentations
                        .entry(variable)
                        .or_default()
                        .insert(presentation);
                }
                self.collect_selector_arguments(locale, arguments, bindings, analysis, presentation)
            }
            InlineExpression::Placeable { expression } => self.collect_selector_expression(
                locale,
                expression,
                bindings,
                analysis,
                presentation,
            ),
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
        analysis: &mut SelectorAnalysis,
        presentation: &FluentPresentation,
    ) {
        for argument in &arguments.positional {
            self.collect_selector_inline(locale, argument, bindings, analysis, presentation);
        }
        for argument in &arguments.named {
            self.collect_selector_inline(locale, &argument.value, bindings, analysis, presentation);
        }
    }
}

fn datetime_presentation<S: AsRef<str>>(
    arguments: &CallArguments<S>,
) -> Option<temporal::Presentation> {
    let mut date_style = None;
    let mut time_style = None;
    for argument in &arguments.named {
        let InlineExpression::StringLiteral { value } = &argument.value else {
            return None;
        };
        let style = temporal::Style::parse(value.as_ref(), argument.name.name.as_ref()).ok()?;
        match argument.name.name.as_ref() {
            "dateStyle" => date_style = Some(style),
            "timeStyle" => time_style = Some(style),
            _ => return None,
        }
    }
    temporal::fluent_presentation(date_style, time_style).ok()
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
) -> Result<(Vec<FluentEntry>, Arc<Resource<String>>), Error> {
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
    validate_fluent_function_calls(&resource.body, path, source, diagnostics)?;
    let parsed = Arc::new(parser::parse(source.to_owned()).map_err(|(_, errors)| {
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
    })?);
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
    let entries = identifiers
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
        .collect::<Result<Vec<_>, Error>>()?;
    Ok((entries, parsed))
}

/// Heine deliberately accepts only the decimal `NUMBER()` options that its
/// shared formatter implements. Fluent otherwise ignores unsupported named
/// options, which would turn a misspelling into silently different output.
fn validate_fluent_function_calls(
    entries: &[Entry<&str>],
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<(), Error> {
    let mut calls = FluentCallCursor::new(source);
    for entry in entries {
        match entry {
            Entry::Message(message) => {
                validate_fluent_function_pattern(
                    message.value.as_ref(),
                    path,
                    source,
                    diagnostics,
                    &mut calls,
                    None,
                )?;
                for attribute in &message.attributes {
                    validate_fluent_function_pattern(
                        Some(&attribute.value),
                        path,
                        source,
                        diagnostics,
                        &mut calls,
                        None,
                    )?;
                }
            }
            Entry::Term(term) => {
                validate_fluent_function_pattern(
                    Some(&term.value),
                    path,
                    source,
                    diagnostics,
                    &mut calls,
                    None,
                )?;
                for attribute in &term.attributes {
                    validate_fluent_function_pattern(
                        Some(&attribute.value),
                        path,
                        source,
                        diagnostics,
                        &mut calls,
                        None,
                    )?;
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

fn validate_fluent_function_pattern(
    pattern: Option<&Pattern<&str>>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    let Some(pattern) = pattern else {
        return Ok(());
    };
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            validate_fluent_function_expression(
                expression,
                path,
                source,
                diagnostics,
                calls,
                selector,
            )?;
        }
    }
    Ok(())
}

fn validate_fluent_function_expression(
    expression: &Expression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    match expression {
        Expression::Inline(expression) => {
            validate_fluent_function_inline(expression, path, source, diagnostics, calls, selector)
        }
        Expression::Select { selector, variants } => {
            validate_fluent_function_inline(selector, path, source, diagnostics, calls, None)?;
            let nested_selector = match selector {
                InlineExpression::VariableReference { id } => Some(id.name),
                _ => None,
            };
            for variant in variants {
                validate_fluent_function_pattern(
                    Some(&variant.value),
                    path,
                    source,
                    diagnostics,
                    calls,
                    nested_selector,
                )?;
            }
            Ok(())
        }
    }
}

fn number_arguments_use_default_precision<S: AsRef<str>>(arguments: &CallArguments<S>) -> bool {
    arguments.named.iter().all(|argument| match argument.name.name.as_ref() {
        "minimumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if value.as_ref() == "0"),
        "maximumFractionDigits" => matches!(&argument.value, InlineExpression::NumberLiteral { value } if value.as_ref() == "3"),
        _ => true,
    })
}

fn validate_fluent_function_inline(
    expression: &InlineExpression<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    match expression {
        InlineExpression::FunctionReference { id, arguments } => {
            let span = calls.next(id.name);
            match FluentFunction::parse(id.name) {
                Some(FluentFunction::Number) => {
                    validate_number_arguments(arguments, path, source, diagnostics, calls, span)?;
                    let selected = matches!(
                        arguments.positional.as_slice(),
                        [InlineExpression::VariableReference { id }] if Some(id.name) == selector
                    );
                    if selected && !number_arguments_use_default_precision(arguments) {
                        return Err(invalid_number_call(
                            path,
                            source,
                            diagnostics,
                            "NUMBER() for a plural selector must use the default fraction-digit policy",
                            span,
                        ));
                    }
                    Ok(())
                }
                Some(FluentFunction::DateTime) => {
                    validate_datetime_arguments(arguments, path, source, diagnostics, calls, span)
                }
                None => Err(unknown_fluent_function(
                    path,
                    source,
                    diagnostics,
                    id.name,
                    span,
                )),
            }
        }
        InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => validate_fluent_function_arguments(
            arguments,
            path,
            source,
            diagnostics,
            calls,
            selector,
        ),
        InlineExpression::Placeable { expression } => validate_fluent_function_expression(
            expression,
            path,
            source,
            diagnostics,
            calls,
            selector,
        ),
        _ => Ok(()),
    }
}

fn validate_fluent_function_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    selector: Option<&str>,
) -> Result<(), Error> {
    for argument in &arguments.positional {
        validate_fluent_function_inline(argument, path, source, diagnostics, calls, selector)?;
    }
    for argument in &arguments.named {
        validate_fluent_function_inline(
            &argument.value,
            path,
            source,
            diagnostics,
            calls,
            selector,
        )?;
    }
    Ok(())
}

fn validate_number_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    span: SourceSpan,
) -> Result<(), Error> {
    let invalid = || invalid_number_call(path, source, diagnostics, "invalid NUMBER() call", span);
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
                        span,
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
                        span,
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
                        span,
                    )
                })?;
            }
            _ => {
                return Err(invalid_number_call(
                    path,
                    source,
                    diagnostics,
                    "NUMBER() supports only useGrouping, minimumFractionDigits, and maximumFractionDigits",
                    span,
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
            span,
        )
    })?;
    validate_fluent_function_arguments(arguments, path, source, diagnostics, calls, None)
}

/// `DATETIME()` deliberately exposes only date and time field-set lengths.
/// Fluent otherwise accepts arbitrary named options and silently ignores them.
fn validate_datetime_arguments(
    arguments: &CallArguments<&str>,
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    calls: &mut FluentCallCursor,
    span: SourceSpan,
) -> Result<(), Error> {
    let invalid = |message| invalid_datetime_call(path, source, diagnostics, message, span);
    if arguments.positional.len() != 1 {
        return Err(invalid("DATETIME() requires exactly one value argument"));
    }
    if !matches!(
        arguments.positional.as_slice(),
        [InlineExpression::VariableReference { .. }]
    ) {
        return Err(invalid("DATETIME() requires a variable value argument"));
    }
    let mut styles = BTreeSet::new();
    for argument in &arguments.named {
        let name = argument.name.name;
        if !matches!(name, "dateStyle" | "timeStyle") {
            return Err(invalid(
                "DATETIME() supports only dateStyle and timeStyle options",
            ));
        }
        if !styles.insert(name) {
            return Err(invalid("DATETIME() cannot repeat a style option"));
        }
        match &argument.value {
            InlineExpression::StringLiteral { value }
                if matches!(*value, "short" | "medium" | "long") => {}
            _ => {
                return Err(invalid(
                    "DATETIME() styles must be the string literal \"short\", \"medium\", or \"long\"",
                ));
            }
        }
    }
    if styles.is_empty() {
        return Err(invalid("DATETIME() requires dateStyle or timeStyle"));
    }
    validate_fluent_function_arguments(arguments, path, source, diagnostics, calls, None)
}

fn invalid_number_call(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    message: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

fn invalid_datetime_call(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    message: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: message.into(),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

/// Fluent-bundle registers one function for Heine. Checking every function
/// reference while loading a resource keeps a typo in a translation from
/// becoming a later, unrelated template-rendering error.
fn unknown_fluent_function(
    path: &Path,
    source: &str,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
    function: &str,
    span: SourceSpan,
) -> Error {
    Error::Parse(Box::new(FluentDiagnostic {
        path: diagnostics.display_path(path),
        message: format!(
            "unknown Fluent function {function:?}; Heine currently provides NUMBER() and DATETIME()"
        ),
        source_code: diagnostics.named_source(path, source, "Fluent"),
        span,
    }))
}

/// Fluent's AST intentionally does not retain source spans. This narrow lexer
/// recovers function-name spans without mistaking ordinary resource text or a
/// string literal for a call. Validation visits expressions in source order,
/// so the cursor connects each validated call to its own source span.
struct FluentCallCursor {
    calls: Vec<(String, SourceSpan)>,
    next: usize,
}

impl FluentCallCursor {
    fn new(source: &str) -> Self {
        let mut calls = Vec::new();
        let mut offset = 0;
        let mut placeables = 0usize;
        let mut quoted = false;
        let mut line_start = true;

        while offset < source.len() {
            let Some(character) = source[offset..].chars().next() else {
                break;
            };
            if quoted {
                if character == '\\' {
                    offset += 1;
                    if let Some(next) = source[offset..].chars().next() {
                        offset += next.len_utf8();
                    }
                    continue;
                }
                if character == '"' {
                    quoted = false;
                }
                offset += character.len_utf8();
                continue;
            }

            if line_start && character.is_ascii_whitespace() && character != '\n' {
                offset += character.len_utf8();
                continue;
            }
            if line_start && character == '#' {
                let line_end = source[offset..]
                    .find('\n')
                    .map_or(source.len(), |length| offset + length);
                offset = line_end;
                continue;
            }

            line_start = character == '\n';
            match character {
                '"' if placeables > 0 => quoted = true,
                '{' => placeables += 1,
                '}' => placeables = placeables.saturating_sub(1),
                _ if placeables > 0 && fluent_identifier_character(character) => {
                    let start = offset;
                    offset += character.len_utf8();
                    while source[offset..]
                        .chars()
                        .next()
                        .is_some_and(fluent_identifier_character)
                    {
                        offset += source[offset..].chars().next().map_or(0, char::len_utf8);
                    }
                    let name = &source[start..offset];
                    let mut next = offset;
                    while source[next..]
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_whitespace())
                    {
                        next += source[next..].chars().next().map_or(0, char::len_utf8);
                    }
                    if source[next..].starts_with('(') {
                        calls.push((name.to_owned(), (start, name.len()).into()));
                    }
                    continue;
                }
                _ => {}
            }
            offset += character.len_utf8();
        }

        Self { calls, next: 0 }
    }

    fn next(&mut self, name: &str) -> SourceSpan {
        let Some((position, (_, span))) = self
            .calls
            .iter()
            .enumerate()
            .skip(self.next)
            .find(|(_, (candidate, _))| candidate == name)
        else {
            return (0, name.len()).into();
        };
        self.next = position + 1;
        *span
    }
}

fn fluent_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
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
