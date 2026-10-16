//! Fluent resource loading and deliberate locale fallback for templates.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use crate::{locale::Locale, source_path};
use fluent_bundle::{FluentArgs, FluentResource, FluentValue, concurrent::FluentBundle};
use fluent_syntax::ast::{
    CallArguments, Expression, InlineExpression, Pattern, PatternElement, Resource,
};
use intl_memoizer::{Memoizable, concurrent::IntlLangMemoizer};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

mod fluent;
pub(crate) mod number;
mod resources;
pub(crate) mod temporal;

use fluent::{
    fluent_pattern, fluent_pattern_targets, number_arguments_use_default_precision, resolve_binding,
};
use resources::{
    add_resources, ensure_kit_directory, ensure_root_directory, load_locale_resources,
    validate_kit_messages, validate_root_overrides,
};

type SelectorBindings = BTreeMap<String, Option<String>>;
type SelectorStack = BTreeSet<(Locale, String, Option<String>, SelectorBindings)>;
type SelectorPresentations = BTreeMap<String, FluentPresentation>;
type TemporalPresentations = BTreeMap<String, BTreeSet<temporal::Presentation>>;

/// Rendering needs only these checked facts from a Fluent message and everything
/// it references.
/// Loading computes them once so repeated template calls do not walk Fluent's
/// syntax tree again.
#[derive(Default)]
struct FormattingRequirements {
    selectors: BTreeSet<String>,
    temporal_presentations: TemporalPresentations,
}

#[derive(Default)]
struct SelectorAnalysis {
    stack: SelectorStack,
    selectors: BTreeSet<String>,
    presentations: SelectorPresentations,
    temporal_presentations: TemporalPresentations,
}

#[derive(Default)]
struct MessageFormattingPlans {
    value: Option<Result<FormattingRequirements, String>>,
    attributes: BTreeMap<String, Result<FormattingRequirements, String>>,
}

/// One page locale's resolved message supplier and its already checked
/// formatting requirements. Fallback messages share the supplier's plan.
struct ResolvedFormattingPlan {
    owner: Locale,
    requirements: Arc<MessageFormattingPlans>,
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

/// The closed Fluent function set that Heine registers and validates.
/// Adding a formatter extends this enum, rather than separate registration and
/// validation lists.
#[derive(Clone, Copy, Eq, PartialEq)]
enum FluentFunction {
    Number,
    DateTime,
}

impl FluentFunction {
    const SUPPORTED: [Self; 2] = [Self::Number, Self::DateTime];

    pub(super) fn parse(name: &str) -> Option<Self> {
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
struct FluentLocale(Locale);

impl Memoizable for FluentLocale {
    type Args = ();
    type Error = unic_langid::LanguageIdentifierError;

    fn construct(
        language: unic_langid::LanguageIdentifier,
        (): Self::Args,
    ) -> Result<Self, Self::Error> {
        language.to_string().parse().map(Self)
    }
}

fn format_fluent_number(value: &FluentValue<'_>, memoizer: &IntlLangMemoizer) -> Option<String> {
    let FluentValue::Number(value) = value else {
        return None;
    };
    memoizer
        .with_try_get::<FluentLocale, _, _>((), |locale| {
            let number = number::LocalizedNumber::from_f64(value.value).ok()?;
            let minimum = value.options.minimum_fraction_digits.unwrap_or(0);
            let maximum = value.options.maximum_fraction_digits;
            let minimum = u16::try_from(minimum).ok()?;
            let maximum = maximum.map(u16::try_from).transpose().ok()?;
            let options = number::Options::with_optional_maximum(
                value.options.use_grouping,
                minimum,
                maximum,
            )
            .ok()?;
            number::format(&locale.0, &number, options).ok()
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
    formatting_plans: BTreeMap<Locale, BTreeMap<String, ResolvedFormattingPlan>>,
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
        let mut localization = Self {
            bundles,
            message_sources,
            message_owners,
            formatting_plans: BTreeMap::new(),
            latest_inputs,
            default_locale,
            editorial_time_zone,
        };
        localization.build_formatting_plans();
        Ok(localization)
    }

    pub(crate) fn format(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
        args: &Arguments,
    ) -> Result<String, String> {
        let plan = self.formatting_plan(locale, id, attribute)?;
        let empty_requirements = FormattingRequirements::default();
        let requirements =
            Self::formatting_requirements(plan, attribute)?.unwrap_or(&empty_requirements);
        let args = args.fluent(
            &requirements.selectors,
            &requirements.temporal_presentations,
            self.editorial_time_zone.as_ref(),
        )?;
        match self.format_in(&plan.owner, id, attribute, &args) {
            Ok(value) => Ok(value),
            Err(FormatError::Invalid(message)) => Err(message),
            Err(FormatError::Missing) if plan.owner == self.default_locale => {
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

    fn build_formatting_plans(&mut self) {
        let mut owner_plans = BTreeMap::new();
        for (lookup_locale, messages) in &self.message_sources {
            for (id, source) in messages {
                let owner = &self.message_owners[lookup_locale][id];
                let locale_plans = owner_plans
                    .entry(owner.clone())
                    .or_insert_with(BTreeMap::new);
                if locale_plans.contains_key(id) {
                    continue;
                }
                let mut message_plans = MessageFormattingPlans::default();
                for attribute in fluent_pattern_targets(&source.parsed.body, id) {
                    let plan = self.formatting_requirements_for(owner, id, attribute.as_deref());
                    match attribute {
                        Some(attribute) => {
                            message_plans.attributes.insert(attribute, plan);
                        }
                        None => message_plans.value = Some(plan),
                    }
                }
                locale_plans.insert(id.clone(), Arc::new(message_plans));
            }
        }
        self.formatting_plans = self
            .message_sources
            .iter()
            .map(|(locale, messages)| {
                let plans = messages
                    .keys()
                    .map(|id| {
                        let owner = self.message_owners[locale][id].clone();
                        let requirements = owner_plans[&owner][id].clone();
                        (
                            id.clone(),
                            ResolvedFormattingPlan {
                                owner,
                                requirements,
                            },
                        )
                    })
                    .collect();
                (locale.clone(), plans)
            })
            .collect();
    }

    fn formatting_plan(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
    ) -> Result<&ResolvedFormattingPlan, String> {
        let plan = self
            .formatting_plans
            .get(locale)
            .and_then(|messages| messages.get(id))
            .ok_or_else(|| format!("message {id:?} is not defined for locale {locale}"))?;
        let pattern_exists = match attribute {
            Some(attribute) => plan.requirements.attributes.contains_key(attribute),
            None => plan.requirements.value.is_some(),
        };
        if pattern_exists || locale == &self.default_locale {
            return Ok(plan);
        }
        self.formatting_plans
            .get(&self.default_locale)
            .and_then(|messages| messages.get(id))
            .ok_or_else(|| format!("message {id:?} is not defined for locale {locale}"))
    }

    fn formatting_requirements<'a>(
        plan: &'a ResolvedFormattingPlan,
        attribute: Option<&str>,
    ) -> Result<Option<&'a FormattingRequirements>, String> {
        let requirements = match attribute {
            Some(attribute) => plan.requirements.attributes.get(attribute),
            None => plan.requirements.value.as_ref(),
        };
        requirements
            .map(|requirements| requirements.as_ref().map_err(Clone::clone))
            .transpose()
    }

    fn formatting_requirements_for(
        &self,
        locale: &Locale,
        id: &str,
        attribute: Option<&str>,
    ) -> Result<FormattingRequirements, String> {
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
        Ok(FormattingRequirements {
            selectors: analysis.selectors,
            temporal_presentations: analysis.temporal_presentations,
        })
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

/// Only absence participates in the documented current-locale-to-default
/// fallback. Formatting failures belong to the resource that defined them and
/// must remain visible instead of being hidden by a default-language value.
enum FormatError {
    Missing,
    Invalid(String),
}
