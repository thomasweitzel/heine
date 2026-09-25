//! Validation for site-owned renderer-profile configuration.

use std::collections::{BTreeMap, BTreeSet};

use miette::SourceSpan;
use serde::Deserialize;

use crate::source_path;

#[derive(Debug, Clone, Default)]
pub(crate) struct Profiles {
    profiles: BTreeMap<String, Profile>,
}

impl Profiles {
    pub(crate) fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
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
    validate_nonempty(program, &format!("renderer profile {name} program"))?;
    validate_nonempty(
        cache_identity,
        &format!("renderer profile {name} cache_identity"),
    )?;
    let timeout_span = timeout_seconds.span().into();
    if timeout_seconds.into_inner() == 0 {
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
        .map(|variant| validate_variant(name, "inline", protocol, &inputs, variant))
        .transpose()?;
    let block = block
        .map(|variant| validate_variant(name, "block", protocol, &inputs, variant))
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
        inputs,
        result,
        inline: inline.is_some(),
        block: block.is_some(),
    })
}

#[derive(Debug, Clone)]
pub(crate) struct Profile {
    inputs: BTreeSet<Input>,
    result: ResultCapabilities,
    inline: bool,
    block: bool,
}

impl Profile {
    pub(crate) fn supports(&self, placement: Placement) -> bool {
        match placement {
            Placement::Inline => self.inline,
            Placement::Block => self.block,
        }
    }

    pub(crate) fn accepts_body(&self) -> bool {
        self.inputs.contains(&Input::Body)
    }

    pub(crate) fn accepts_named_source(&self) -> bool {
        self.inputs.contains(&Input::File) || self.inputs.contains(&Input::Bundle)
    }

    pub(crate) fn result(&self) -> &ResultCapabilities {
        &self.result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Inline,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Output {
    Embedded,
    Resource,
}

#[derive(Debug, Clone)]
pub(crate) enum ResultCapabilities {
    Embedded,
    Svg {
        materializations: BTreeSet<Output>,
        default_output: Output,
    },
    Image,
    Link,
}

impl ResultCapabilities {
    pub(crate) fn output(&self, requested: Option<Output>) -> Option<Output> {
        match self {
            Self::Embedded => match requested {
                None | Some(Output::Embedded) => Some(Output::Embedded),
                Some(Output::Resource) => None,
            },
            Self::Svg {
                materializations,
                default_output,
            } => {
                let output = requested.unwrap_or(*default_output);
                materializations.contains(&output).then_some(output)
            }
            Self::Image | Self::Link => match requested {
                None | Some(Output::Resource) => Some(Output::Resource),
                Some(Output::Embedded) => None,
            },
        }
    }

    pub(crate) fn needs_alt(&self, output: Output) -> bool {
        matches!(self, Self::Svg { .. } | Self::Image) && output == Output::Resource
    }

    pub(crate) fn needs_label(&self, output: Output) -> bool {
        matches!(self, Self::Link) && output == Output::Resource
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
) -> Result<(), ConfigError> {
    match value {
        Some(value) if protocol == Protocol::Workspace && inputs.contains(&Input::Body) => {
            validate_extension(value, &format!("renderer profile {name} input_extension"))
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
        None => Ok(()),
    }
}

fn validate_variant(
    name: &str,
    placement: &str,
    protocol: Protocol,
    inputs: &BTreeSet<Input>,
    raw: RawVariant,
) -> Result<(), ConfigError> {
    let arguments_span = raw.arguments.span().into();
    let mut placeholders = BTreeMap::<&str, usize>::new();
    for argument in raw.arguments.into_inner() {
        let span = argument.span().into();
        let argument = argument.into_inner();
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
        return Ok(());
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
    Ok(())
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
            reject_result_options(name, &options, "html").map(|()| ResultCapabilities::Embedded)
        }
        ResultKind::Mathml => {
            reject_result_options(name, &options, "mathml").map(|()| ResultCapabilities::Embedded)
        }
        ResultKind::Svg => validate_svg_result(name, result_span, options),
        ResultKind::Image => validate_resource_result(name, result_span, options, true),
        ResultKind::Link => validate_resource_result(name, result_span, options, false),
    }
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
    Ok(ResultCapabilities::Svg {
        default_output: default.map_or(Output::Embedded, |(_, output)| output),
        materializations,
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
    validate_extension(
        extension,
        &format!("resource renderer profile {name} extension"),
    )?;
    Ok(if image {
        ResultCapabilities::Image
    } else {
        ResultCapabilities::Link
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

fn validate_nonempty(value: toml::Spanned<String>, field: &str) -> Result<(), ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if value.trim().is_empty() || value.contains('\0') {
        Err(ConfigError {
            span,
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(())
    }
}

fn validate_extension(value: toml::Spanned<String>, field: &str) -> Result<(), ConfigError> {
    let span = value.span().into();
    let value = value.into_inner();
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Ok(())
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
    inline: Option<RawVariant>,
    block: Option<RawVariant>,
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
