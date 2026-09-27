//! Validation and execution boundary for site-owned renderer profiles.

mod cache;
mod result;
mod runner;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use miette::SourceSpan;
use serde::Deserialize;

use crate::source_path;

pub(super) use cache::RendererCache;
pub(super) use result::validate_renderer_output;
pub(crate) use result::{Appearance, Output, Placement, RendererOutput, ResultCapabilities};
pub(crate) use result::{add_embedded_svg_class, image_dimensions};
pub(crate) use runner::{
    RendererError, RendererRuntime, StreamRequest, WorkspaceInput, WorkspaceRequest, run_stream,
    run_workspace,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Profiles {
    profiles: BTreeMap<String, Profile>,
}

impl Profiles {
    pub(crate) fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub(crate) fn uses_workspace(&self) -> bool {
        self.profiles.values().any(Profile::uses_workspace)
    }
}

pub(crate) fn parse(
    profiles: BTreeMap<toml::Spanned<String>, RawProfile>,
) -> Result<Profiles, ConfigError> {
    let mut parsed = BTreeMap::new();
    for (raw_name, profile) in profiles {
        let span = raw_name.span().into();
        let name = raw_name.into_inner();
        if !source_path::is_simple_identifier(&name) {
            return Err(ConfigError {
                span,
                message: format!("renderer profile name {name:?} must be a simple identifier"),
            });
        }
        parsed.insert(name.clone(), validate_profile(&name, profile)?);
    }
    Ok(Profiles { profiles: parsed })
}

fn validate_profile(name: &str, profile: RawProfile) -> Result<Profile, ConfigError> {
    let RawProfile {
        protocol,
        program,
        timeout_seconds,
        cache_identity,
        inputs,
        result,
        input_extension,
        media_type,
        extension,
        materializations,
        default_output,
        inline,
        block,
    } = profile;
    let program = validate_nonempty(program, &format!("renderer profile {name} program"))?;
    let cache_identity = validate_nonempty(
        cache_identity,
        &format!("renderer profile {name} cache_identity"),
    )?;
    let timeout_span = timeout_seconds.span().into();
    let timeout_seconds = timeout_seconds.into_inner();
    if timeout_seconds == 0 {
        return Err(ConfigError {
            span: timeout_span,
            message: format!("renderer profile {name} timeout_seconds must be positive"),
        });
    }
    let inputs_span = inputs.span().into();
    let inputs = parse_inputs(name, inputs)?;
    if protocol == Protocol::Stream && inputs.contains(&Input::Bundle) {
        return Err(ConfigError {
            span: inputs_span,
            message: format!(
                "stream renderer profile {name} may accept only \"body\" and \"file\" inputs"
            ),
        });
    }
    let input_extension =
        validate_input_extension(name, protocol, &inputs, input_extension, inputs_span)?;
    let result = validate_result(
        name,
        result,
        ResultOptions {
            media_type,
            extension,
            materializations,
            default_output,
        },
    )?;
    let inline = inline
        .map(|variant| validate_placement(name, "inline", protocol, &inputs, &result, variant))
        .transpose()?;
    let block = block
        .map(|variant| validate_placement(name, "block", protocol, &inputs, &result, variant))
        .transpose()?;
    if inline.is_none() && block.is_none() {
        return Err(ConfigError {
            span: inputs_span,
            message: format!(
                "renderer profile {name} must declare an inline or block placement variant"
            ),
        });
    }
    Ok(Profile {
        protocol,
        program,
        timeout_seconds,
        cache_identity,
        input_extension,
        inputs,
        result,
        inline,
        block,
    })
}

// Execution consumes these values after source resolution has established the
// request inventory. Keeping them here prevents configuration from being
// reparsed or reconstructed at the process boundary.
#[derive(Debug, Clone)]
pub(crate) struct Profile {
    protocol: Protocol,
    program: String,
    timeout_seconds: u64,
    cache_identity: String,
    input_extension: Option<String>,
    inputs: BTreeSet<Input>,
    result: ResultCapabilities,
    inline: Option<PlacementVariants>,
    block: Option<PlacementVariants>,
}

impl Profile {
    pub(crate) fn supports(&self, placement: Placement) -> bool {
        match placement {
            Placement::Inline => self.inline.is_some(),
            Placement::Block => self.block.is_some(),
        }
    }

    pub(crate) fn accepts_body(&self) -> bool {
        self.inputs.contains(&Input::Body)
    }

    pub(crate) fn accepts_named_source(&self, source: NamedInput) -> bool {
        self.inputs.contains(&match source {
            NamedInput::File => Input::File,
            NamedInput::Bundle => Input::Bundle,
        })
    }

    pub(crate) fn result(&self) -> &ResultCapabilities {
        &self.result
    }

    pub(crate) fn uses_workspace(&self) -> bool {
        self.protocol == Protocol::Workspace
    }

    pub(crate) fn resource_extension(&self) -> Option<&str> {
        match &self.result {
            ResultCapabilities::Svg { .. } => Some("svg"),
            ResultCapabilities::Image { extension }
            | ResultCapabilities::Picture { extension, .. }
            | ResultCapabilities::Link { extension } => Some(extension),
            ResultCapabilities::Html | ResultCapabilities::Mathml => None,
        }
    }

    pub(crate) fn variant(
        &self,
        placement: Placement,
        appearance: Option<Appearance>,
    ) -> Result<&Variant, String> {
        let variants = match placement {
            Placement::Inline => self.inline.as_ref(),
            Placement::Block => self.block.as_ref(),
        }
        .ok_or_else(|| "selected profile does not support this placement".to_owned())?;
        variants.variant(appearance)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Variant {
    arguments: Vec<String>,
}

impl Variant {
    pub(crate) fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Debug, Clone)]
enum PlacementVariants {
    Single(Variant),
    Picture { light: Variant, dark: Variant },
}

impl PlacementVariants {
    fn variant(&self, appearance: Option<Appearance>) -> Result<&Variant, String> {
        match (self, appearance) {
            (Self::Single(variant), None) => Ok(variant),
            (Self::Picture { light, .. }, Some(Appearance::Light)) => Ok(light),
            (Self::Picture { dark, .. }, Some(Appearance::Dark)) => Ok(dark),
            (Self::Single(_), Some(_)) => {
                Err("selected renderer profile has no appearance variants".into())
            }
            (Self::Picture { .. }, None) => {
                Err("selected picture renderer requires an appearance variant".into())
            }
        }
    }
}

/// The checked form of a page-owned renderer input. Its filesystem inventory
/// remains with the page loader; Markdown only needs this stable capability
/// when it resolves a named request against its selected profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedInput {
    File,
    Bundle,
}

/// One page-owned renderer input after discovery has fixed its complete
/// filesystem boundary. Later execution receives this checked inventory rather
/// than a raw authored path.
#[derive(Debug, Clone)]
pub(crate) struct Source {
    input: NamedInput,
    root: PathBuf,
    entry: PathBuf,
    files: Vec<PathBuf>,
    declaration_span: SourceSpan,
}

impl Source {
    pub(crate) fn file(path: PathBuf, declaration_span: SourceSpan) -> Self {
        Self {
            input: NamedInput::File,
            root: path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
            entry: path.clone(),
            files: vec![path],
            declaration_span,
        }
    }

    pub(crate) fn bundle(
        root: PathBuf,
        entry: PathBuf,
        files: Vec<PathBuf>,
        declaration_span: SourceSpan,
    ) -> Self {
        Self {
            input: NamedInput::Bundle,
            root,
            entry,
            files,
            declaration_span,
        }
    }

    pub(crate) fn input(&self) -> NamedInput {
        self.input
    }

    pub(crate) fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub(crate) fn root(&self) -> &PathBuf {
        &self.root
    }

    pub(crate) fn entry(&self) -> &PathBuf {
        &self.entry
    }

    pub(crate) fn declaration_span(&self) -> SourceSpan {
        self.declaration_span
    }
}

fn parse_inputs(
    name: &str,
    raw: toml::Spanned<Vec<toml::Spanned<Input>>>,
) -> Result<BTreeSet<Input>, ConfigError> {
    let span = raw.span().into();
    let mut inputs = BTreeSet::new();
    for input in raw.into_inner() {
        let input_span = input.span().into();
        if !inputs.insert(input.into_inner()) {
            return Err(ConfigError {
                span: input_span,
                message: format!("renderer profile {name} inputs must not contain duplicates"),
            });
        }
    }
    if inputs.is_empty() {
        return Err(ConfigError {
            span,
            message: format!("renderer profile {name} inputs must not be empty"),
        });
    }
    Ok(inputs)
}

fn validate_input_extension(
    name: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    value: Option<toml::Spanned<String>>,
    inputs_span: SourceSpan,
) -> Result<Option<String>, ConfigError> {
    match value {
        Some(value) if protocol == Protocol::Workspace && inputs.contains(&Input::Body) => {
            validate_extension(value, &format!("renderer profile {name} input_extension")).map(Some)
        }
        Some(value) => Err(ConfigError {
            span: value.span().into(),
            message: format!(
                "renderer profile {name} input_extension is allowed only for a workspace profile that accepts \"body\""
            ),
        }),
        None if protocol == Protocol::Workspace && inputs.contains(&Input::Body) => {
            Err(ConfigError {
                span: inputs_span,
                message: format!(
                    "workspace renderer profile {name} accepts \"body\" and must declare input_extension"
                ),
            })
        }
        None => Ok(None),
    }
}

fn validate_placement(
    name: &str,
    placement: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    result: &ResultCapabilities,
    raw: toml::Spanned<RawPlacement>,
) -> Result<PlacementVariants, ConfigError> {
    let placement_span = raw.span().into();
    match (
        matches!(result, ResultCapabilities::Picture { .. }),
        raw.into_inner(),
    ) {
        (
            false,
            RawPlacement {
                arguments: Some(arguments),
                light: None,
                dark: None,
            },
        ) => {
            let variant = RawVariant { arguments };
            validate_variant(name, placement, protocol, inputs, variant)
                .map(PlacementVariants::Single)
        }
        (
            true,
            RawPlacement {
                arguments: None,
                light: Some(light),
                dark: Some(dark),
            },
        ) => {
            let light =
                validate_variant(name, &format!("{placement}.light"), protocol, inputs, light)?;
            let dark =
                validate_variant(name, &format!("{placement}.dark"), protocol, inputs, dark)?;
            Ok(PlacementVariants::Picture { light, dark })
        }
        (
            false,
            RawPlacement {
                light: Some(light), ..
            },
        ) => Err(ConfigError {
            span: light.arguments.span().into(),
            message: format!("renderer profile {name} {placement} must declare arguments directly"),
        }),
        (
            true,
            RawPlacement {
                arguments: Some(arguments),
                ..
            },
        ) => Err(ConfigError {
            span: arguments.span().into(),
            message: format!(
                "picture renderer profile {name} {placement} must declare light and dark variants"
            ),
        }),
        (_, RawPlacement { .. }) => Err(ConfigError {
            span: placement_span,
            message: format!(
                "renderer profile {name} {placement} has incomplete appearance variants"
            ),
        }),
    }
}

fn validate_variant(
    name: &str,
    placement: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    raw: RawVariant,
) -> Result<Variant, ConfigError> {
    let arguments_span = raw.arguments.span().into();
    let mut placeholders = BTreeMap::<&str, usize>::new();
    let mut arguments = Vec::new();
    for argument in raw.arguments.into_inner() {
        let span = argument.span().into();
        let argument = argument.into_inner();
        arguments.push(argument.clone());
        for placeholder in PLACEHOLDERS {
            if argument.contains(placeholder) && argument != placeholder {
                return Err(ConfigError {
                    span,
                    message: format!(
                        "renderer profile {name} {placement} arguments must use {placeholder} as a complete argument"
                    ),
                });
            }
        }
        if let Some(placeholder) = PLACEHOLDERS
            .into_iter()
            .find(|placeholder| *placeholder == argument)
        {
            *placeholders.entry(placeholder).or_default() += 1;
        }
    }
    if protocol == Protocol::Stream {
        if let Some((placeholder, _)) = placeholders.into_iter().next() {
            return Err(ConfigError {
                span: arguments_span,
                message: format!(
                    "stream renderer profile {name} {placement} arguments must not use workspace placeholder {placeholder}"
                ),
            });
        }
        return Ok(Variant { arguments });
    }
    if placeholders
        .get("{output_file}")
        .copied()
        .unwrap_or_default()
        != 1
    {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} arguments must contain {{output_file}} exactly once"
            ),
        });
    }
    for placeholder in ["{input_file}", "{input_dir}", "{work_dir}"] {
        if placeholders.get(placeholder).copied().unwrap_or_default() > 1 {
            return Err(ConfigError {
                span: arguments_span,
                message: format!(
                    "workspace renderer profile {name} {placement} arguments must contain {placeholder} at most once"
                ),
            });
        }
    }
    if placeholders.contains_key("{input_file}")
        && !inputs.contains(&Input::Body)
        && !inputs.contains(&Input::File)
    {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} uses {{input_file}} but accepts neither \"body\" nor \"file\""
            ),
        });
    }
    if placeholders.contains_key("{input_dir}") && !inputs.contains(&Input::Bundle) {
        return Err(ConfigError {
            span: arguments_span,
            message: format!(
                "workspace renderer profile {name} {placement} uses {{input_dir}} but does not accept \"bundle\""
            ),
        });
    }
    Ok(Variant { arguments })
}

const PLACEHOLDERS: [&str; 4] = ["{input_file}", "{input_dir}", "{work_dir}", "{output_file}"];

struct ResultOptions {
    media_type: Option<toml::Spanned<String>>,
    extension: Option<toml::Spanned<String>>,
    materializations: Option<toml::Spanned<Vec<toml::Spanned<SvgMaterialization>>>>,
    default_output: Option<toml::Spanned<SvgMaterialization>>,
}

fn validate_result(
    name: &str,
    raw_result: toml::Spanned<ResultKind>,
    options: ResultOptions,
) -> Result<ResultCapabilities, ConfigError> {
    let result_span = raw_result.span().into();
    match raw_result.into_inner() {
        ResultKind::Html => {
            reject_result_options(name, &options, "html").map(|()| ResultCapabilities::Html)
        }
        ResultKind::Mathml => {
            reject_result_options(name, &options, "mathml").map(|()| ResultCapabilities::Mathml)
        }
        ResultKind::Svg => validate_svg_result(name, result_span, options),
        ResultKind::Image => validate_resource_result(name, result_span, options, true),
        ResultKind::Link => validate_resource_result(name, result_span, options, false),
        ResultKind::Picture => validate_picture_result(name, result_span, options),
    }
}

fn validate_picture_result(
    name: &str,
    result_span: SourceSpan,
    options: ResultOptions,
) -> Result<ResultCapabilities, ConfigError> {
    validate_resource_result(name, result_span, options, true).map(|result| match result {
        ResultCapabilities::Image { extension } => ResultCapabilities::Picture {
            svg: extension.eq_ignore_ascii_case("svg"),
            extension,
        },
        _ => unreachable!("image resource validation always returns an image result"),
    })
}

fn reject_result_options(
    name: &str,
    options: &ResultOptions,
    result: &str,
) -> Result<(), ConfigError> {
    if let Some(span) = option_span(options) {
        return Err(ConfigError {
            span,
            message: format!("{result} renderer profile {name} must not declare resource options"),
        });
    }
    Ok(())
}

fn validate_svg_result(
    name: &str,
    result_span: SourceSpan,
    options: ResultOptions,
) -> Result<ResultCapabilities, ConfigError> {
    let fixed_option_span = options
        .media_type
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| options.extension.as_ref().map(|value| value.span().into()));
    if let Some(span) = fixed_option_span {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} has fixed image/svg+xml media type and svg extension"
            ),
        });
    }
    let raw = options.materializations.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("SVG renderer profile {name} must declare supported materializations"),
    })?;
    let materializations_span = raw.span().into();
    let mut materializations = BTreeSet::new();
    for value in raw.into_inner() {
        let span = value.span().into();
        if !materializations.insert(Output::from(value.into_inner())) {
            return Err(ConfigError {
                span,
                message: format!(
                    "SVG renderer profile {name} materializations must not contain duplicates"
                ),
            });
        }
    }
    if materializations.is_empty() {
        return Err(ConfigError {
            span: materializations_span,
            message: format!("SVG renderer profile {name} materializations must not be empty"),
        });
    }
    let default = options
        .default_output
        .map(|value| (value.span().into(), Output::from(value.into_inner())));
    if materializations.len() == 2 && default.is_none() {
        return Err(ConfigError {
            span: materializations_span,
            message: format!(
                "SVG renderer profile {name} with both materializations must declare default_output"
            ),
        });
    }
    if materializations.len() == 1
        && let Some((span, _)) = default
    {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} must omit default_output when it supports one materialization"
            ),
        });
    }
    if let Some((span, default)) = default
        && !materializations.contains(&default)
    {
        return Err(ConfigError {
            span,
            message: format!(
                "SVG renderer profile {name} default_output must be one of its materializations"
            ),
        });
    }
    let default_output = default.map_or_else(
        || {
            *materializations
                .iter()
                .next()
                .expect("non-empty SVG materializations were checked above")
        },
        |(_, output)| output,
    );
    Ok(ResultCapabilities::Svg {
        materializations,
        default_output,
    })
}

fn validate_resource_result(
    name: &str,
    result_span: SourceSpan,
    options: ResultOptions,
    image: bool,
) -> Result<ResultCapabilities, ConfigError> {
    let materialization_span = options
        .materializations
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| {
            options
                .default_output
                .as_ref()
                .map(|value| value.span().into())
        });
    if let Some(span) = materialization_span {
        return Err(ConfigError {
            span,
            message: format!(
                "resource renderer profile {name} must not declare SVG materialization options"
            ),
        });
    }
    let media_type = options.media_type.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("resource renderer profile {name} must declare media_type"),
    })?;
    let media_type_span = media_type.span().into();
    let media_type = media_type
        .into_inner()
        .parse::<mime::Mime>()
        .map_err(|_| ConfigError {
            span: media_type_span,
            message: format!(
                "resource renderer profile {name} media_type must be a valid media type"
            ),
        })?;
    if image && media_type.type_() != mime::IMAGE {
        return Err(ConfigError {
            span: media_type_span,
            message: format!("image renderer profile {name} media_type must start with image/"),
        });
    }
    let extension = options.extension.ok_or_else(|| ConfigError {
        span: result_span,
        message: format!("resource renderer profile {name} must declare extension"),
    })?;
    let extension = validate_extension(
        extension,
        &format!("resource renderer profile {name} extension"),
    )?;
    Ok(if image {
        ResultCapabilities::Image { extension }
    } else {
        ResultCapabilities::Link { extension }
    })
}

fn option_span(options: &ResultOptions) -> Option<SourceSpan> {
    options
        .media_type
        .as_ref()
        .map(|value| value.span().into())
        .or_else(|| options.extension.as_ref().map(|value| value.span().into()))
        .or_else(|| {
            options
                .materializations
                .as_ref()
                .map(|value| value.span().into())
        })
        .or_else(|| {
            options
                .default_output
                .as_ref()
                .map(|value| value.span().into())
        })
}

fn validate_nonempty(value: toml::Spanned<String>, field: &str) -> Result<String, ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if value.trim().is_empty() || value.contains('\0') {
        Err(ConfigError {
            span,
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(value)
    }
}

fn validate_extension(value: toml::Spanned<String>, field: &str) -> Result<String, ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Ok(value)
    } else {
        Err(ConfigError {
            span,
            message: format!("{field} must be an unprefixed ASCII alphanumeric filename extension"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Protocol {
    Stream,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Input {
    Body,
    File,
    Bundle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ResultKind {
    Html,
    Svg,
    Mathml,
    Image,
    Picture,
    Link,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SvgMaterialization {
    Embedded,
    Resource,
}

impl From<SvgMaterialization> for Output {
    fn from(value: SvgMaterialization) -> Self {
        match value {
            SvgMaterialization::Embedded => Self::Embedded,
            SvgMaterialization::Resource => Self::Resource,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawProfile {
    protocol: Protocol,
    program: toml::Spanned<String>,
    timeout_seconds: toml::Spanned<u64>,
    cache_identity: toml::Spanned<String>,
    inputs: toml::Spanned<Vec<toml::Spanned<Input>>>,
    result: toml::Spanned<ResultKind>,
    input_extension: Option<toml::Spanned<String>>,
    media_type: Option<toml::Spanned<String>>,
    extension: Option<toml::Spanned<String>>,
    materializations: Option<toml::Spanned<Vec<toml::Spanned<SvgMaterialization>>>>,
    default_output: Option<toml::Spanned<SvgMaterialization>>,
    inline: Option<toml::Spanned<RawPlacement>>,
    block: Option<toml::Spanned<RawPlacement>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlacement {
    arguments: Option<toml::Spanned<Vec<toml::Spanned<String>>>>,
    light: Option<RawVariant>,
    dark: Option<RawVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVariant {
    arguments: toml::Spanned<Vec<toml::Spanned<String>>>,
}

pub(crate) struct ConfigError {
    pub(crate) span: SourceSpan,
    pub(crate) message: String,
}
