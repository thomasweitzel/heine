//! Checked renderer-result capabilities and result validation.

use std::path::PathBuf;

use quick_xml::{Reader, Writer, events::Event};
use sha2::{Digest, Sha256};

use super::{Profile, cache::hex_digest};

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
    Html,
    Mathml,
    Svg {
        materializations: std::collections::BTreeSet<Output>,
        default_output: Output,
    },
    Image {
        extension: String,
    },
    Link {
        extension: String,
    },
}

impl ResultCapabilities {
    pub(crate) fn output(&self, requested: Option<Output>) -> Option<Output> {
        match self {
            Self::Html | Self::Mathml => match requested {
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
            Self::Image { .. } | Self::Link { .. } => match requested {
                None | Some(Output::Resource) => Some(Output::Resource),
                Some(Output::Embedded) => None,
            },
        }
    }

    pub(crate) fn needs_alt(&self, output: Output) -> bool {
        matches!(self, Self::Svg { .. } | Self::Image { .. }) && output == Output::Resource
    }

    pub(crate) fn needs_label(&self, output: Output) -> bool {
        matches!(self, Self::Link { .. }) && output == Output::Resource
    }
}

pub(crate) enum RendererOutput {
    Embedded(String),
    Resource {
        output_path: crate::output::OutputPath,
        bytes: Vec<u8>,
    },
}

impl RendererOutput {
    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Embedded(value) => value.as_bytes(),
            Self::Resource { bytes, .. } => bytes,
        }
    }
}

fn resource_output_path(
    profile: &Profile,
    bytes: &[u8],
) -> Result<crate::output::OutputPath, String> {
    if bytes.is_empty() {
        return Err("renderer returned empty output".into());
    }
    let extension = profile
        .resource_extension()
        .ok_or_else(|| "selected renderer result cannot produce a resource".to_owned())?;
    crate::output::OutputPath::try_new(PathBuf::from(format!(
        "heine-renderers/sha256-{}.{}",
        hex_digest(&Sha256::digest(bytes)),
        extension
    )))
}

pub(crate) fn validate_renderer_output(
    profile: &Profile,
    output: Output,
    bytes: Vec<u8>,
) -> Result<RendererOutput, String> {
    match output {
        Output::Embedded => {
            validate_embedded(profile.result(), bytes).map(RendererOutput::Embedded)
        }
        Output::Resource => validate_resource(profile.result(), bytes).and_then(|bytes| {
            resource_output_path(profile, &bytes)
                .map(|output_path| RendererOutput::Resource { output_path, bytes })
        }),
    }
}

fn validate_embedded(result: &ResultCapabilities, bytes: Vec<u8>) -> Result<String, String> {
    match result {
        ResultCapabilities::Html => String::from_utf8(bytes)
            .map_err(|_| "HTML renderer returned non-UTF-8 embedded output".to_owned()),
        ResultCapabilities::Mathml => validate_xml_fragment(&bytes, b"math", "MathML"),
        ResultCapabilities::Svg { .. } => validate_xml_fragment(&bytes, b"svg", "SVG"),
        ResultCapabilities::Image { .. } | ResultCapabilities::Link { .. } => {
            Err("selected renderer result cannot be embedded".to_owned())
        }
    }
    .and_then(|result| {
        (!result.trim().is_empty())
            .then_some(result)
            .ok_or_else(|| "renderer returned empty embedded output".to_owned())
    })
}

fn validate_resource(result: &ResultCapabilities, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("renderer returned empty output".to_owned());
    }
    match result {
        ResultCapabilities::Svg { .. } => {
            validate_xml_fragment(&bytes, b"svg", "SVG")?;
            Ok(bytes)
        }
        ResultCapabilities::Image { .. } | ResultCapabilities::Link { .. } => Ok(bytes),
        ResultCapabilities::Html | ResultCapabilities::Mathml => {
            Err("selected renderer result cannot produce a resource".to_owned())
        }
    }
}

fn validate_xml_fragment(bytes: &[u8], expected_root: &[u8], kind: &str) -> Result<String, String> {
    let mut reader = Reader::from_reader(bytes);
    let mut writer = Writer::new(Vec::new());
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut root_seen = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Decl(_) | Event::DocType(_)) => {}
            Ok(Event::Start(event)) => {
                if !root_seen {
                    if event.name().as_ref() != expected_root {
                        return Err(format!(
                            "renderer returned {kind} with a different document element"
                        ));
                    }
                    root_seen = true;
                } else if depth == 0 {
                    return Err(format!(
                        "renderer returned more than one {kind} document element"
                    ));
                }
                depth += 1;
                writer
                    .write_event(Event::Start(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::Empty(event)) => {
                if !root_seen {
                    if event.name().as_ref() != expected_root {
                        return Err(format!(
                            "renderer returned {kind} with a different document element"
                        ));
                    }
                    root_seen = true;
                } else if depth == 0 {
                    return Err(format!(
                        "renderer returned more than one {kind} document element"
                    ));
                }
                writer
                    .write_event(Event::Empty(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::End(event)) => {
                if depth == 0 {
                    return Err(format!("renderer returned malformed {kind}"));
                }
                depth -= 1;
                writer
                    .write_event(Event::End(event.into_owned()))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::Eof) => break,
            Ok(event) => {
                if root_seen
                    || !matches!(event, Event::Text(ref text) if text.iter().all(|byte| byte.is_ascii_whitespace()))
                {
                    writer
                        .write_event(event.into_owned())
                        .map_err(|error| error.to_string())?;
                }
            }
            Err(error) => return Err(format!("renderer returned malformed {kind}: {error}")),
        }
        buffer.clear();
    }
    if !root_seen || depth != 0 {
        return Err(format!("renderer returned malformed {kind}"));
    }
    String::from_utf8(writer.into_inner())
        .map_err(|_| format!("renderer returned non-UTF-8 {kind}"))
}
