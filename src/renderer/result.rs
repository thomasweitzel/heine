//! Checked renderer-result capabilities and result validation.

use std::{io::Cursor, path::PathBuf};

use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesStart, Event},
};
use sha2::{Digest, Sha256};

use super::{Profile, cache::hex_digest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Inline,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
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
        media_type: String,
    },
    Picture {
        extension: String,
        svg: bool,
        media_type: String,
    },
    Link {
        extension: String,
        media_type: String,
    },
}

impl ResultCapabilities {
    /// Returns the conventional filename extension for a workspace result.
    ///
    /// Workspace programs receive a file rather than a stream. Supplying the
    /// checked result extension lets programs that infer their output format
    /// from the destination name use the same contract as Heine's publisher.
    pub(crate) fn workspace_output_extension(&self) -> &str {
        match self {
            Self::Html => "html",
            Self::Mathml => "mml",
            Self::Svg { .. } => "svg",
            Self::Image { extension, .. }
            | Self::Picture { extension, .. }
            | Self::Link { extension, .. } => extension,
        }
    }

    pub(crate) fn is_picture(&self) -> bool {
        matches!(self, Self::Picture { .. })
    }

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
            Self::Image { .. } | Self::Picture { .. } | Self::Link { .. } => match requested {
                None | Some(Output::Resource) => Some(Output::Resource),
                Some(Output::Embedded) => None,
            },
        }
    }

    pub(crate) fn needs_alt(&self, output: Output) -> bool {
        matches!(
            self,
            Self::Svg { .. } | Self::Image { .. } | Self::Picture { .. }
        ) && output == Output::Resource
    }

    pub(crate) fn needs_label(&self, output: Output) -> bool {
        matches!(self, Self::Link { .. }) && output == Output::Resource
    }

    pub(crate) fn is_embedded_svg(&self, output: Output) -> bool {
        matches!(self, Self::Svg { .. }) && output == Output::Embedded
    }

    pub(crate) fn resource_media_type(&self) -> Option<&str> {
        match self {
            Self::Svg {
                materializations, ..
            } => materializations
                .contains(&Output::Resource)
                .then_some("image/svg+xml"),
            Self::Image { media_type, .. }
            | Self::Picture { media_type, .. }
            | Self::Link { media_type, .. } => Some(media_type),
            Self::Html | Self::Mathml => None,
        }
    }
}

pub(crate) fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
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

/// Adds Markdown-owned classes to one previously checked embedded SVG root.
/// The renderer cache intentionally retains the undecorated renderer result.
pub(crate) fn add_embedded_svg_class(svg: &str, class: &str) -> Result<String, String> {
    write_xml_fragment(svg.as_bytes(), "svg", "SVG", Some(class))
}

fn validate_embedded(result: &ResultCapabilities, bytes: Vec<u8>) -> Result<String, String> {
    match result {
        ResultCapabilities::Html => String::from_utf8(bytes)
            .map_err(|_| "HTML renderer returned non-UTF-8 embedded output".to_owned()),
        ResultCapabilities::Mathml => validate_xml_fragment(&bytes, "math", "MathML"),
        ResultCapabilities::Svg { .. } => validate_xml_fragment(&bytes, "svg", "SVG"),
        ResultCapabilities::Image { .. }
        | ResultCapabilities::Picture { .. }
        | ResultCapabilities::Link { .. } => {
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
            validate_xml_fragment(&bytes, "svg", "SVG")?;
            Ok(bytes)
        }
        ResultCapabilities::Picture { svg: true, .. } => {
            validate_xml_fragment(&bytes, "svg", "SVG")?;
            Ok(bytes)
        }
        ResultCapabilities::Image { .. }
        | ResultCapabilities::Picture { .. }
        | ResultCapabilities::Link { .. } => Ok(bytes),
        ResultCapabilities::Html | ResultCapabilities::Mathml => {
            Err("selected renderer result cannot produce a resource".to_owned())
        }
    }
}

fn validate_xml_fragment(bytes: &[u8], expected_root: &str, kind: &str) -> Result<String, String> {
    write_xml_fragment(bytes, expected_root, kind, None)
}

fn write_xml_fragment(
    bytes: &[u8],
    expected_root: &str,
    kind: &str,
    root_class: Option<&str>,
) -> Result<String, String> {
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
                let event = if depth == 1 {
                    add_root_class(event, root_class)?
                } else {
                    event.into_owned()
                };
                writer
                    .write_event(Event::Start(event))
                    .map_err(|error| error.to_string())?;
            }
            Ok(Event::Empty(event)) => {
                let root = !root_seen;
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
                let event = if root {
                    add_root_class(event, root_class)?
                } else {
                    event.into_owned()
                };
                writer
                    .write_event(Event::Empty(event))
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
                    || !matches!(event, Event::Text(ref text) if text.as_ref().bytes().all(|byte| byte.is_ascii_whitespace()))
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

fn add_root_class(
    event: BytesStart<'_>,
    root_class: Option<&str>,
) -> Result<BytesStart<'static>, String> {
    let Some(root_class) = root_class else {
        return Ok(event.into_owned());
    };
    let mut root = BytesStart::new("svg");
    let mut class_found = false;
    for attribute in event.attributes() {
        let attribute =
            attribute.map_err(|error| format!("renderer returned malformed SVG: {error}"))?;
        if attribute.key.as_ref() == "class" {
            class_found = true;
            let existing = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|error| format!("renderer returned malformed SVG: {error}"))?;
            let value = if existing.is_empty() {
                root_class.to_owned()
            } else {
                format!("{existing} {root_class}")
            };
            root.push_attribute(("class", value.as_str()));
        } else {
            root.push_attribute(attribute);
        }
    }
    if !class_found {
        root.push_attribute(("class", root_class));
    }
    Ok(root.into_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Output, ResultCapabilities, add_embedded_svg_class};

    #[test]
    fn workspace_output_extensions_follow_checked_result_capabilities() {
        let capabilities = [
            (ResultCapabilities::Html, "html"),
            (ResultCapabilities::Mathml, "mml"),
            (
                ResultCapabilities::Svg {
                    materializations: BTreeSet::from([Output::Embedded]),
                    default_output: Output::Embedded,
                },
                "svg",
            ),
            (
                ResultCapabilities::Image {
                    extension: "webp".into(),
                    media_type: "image/webp".into(),
                },
                "webp",
            ),
            (
                ResultCapabilities::Picture {
                    extension: "png".into(),
                    svg: false,
                    media_type: "image/png".into(),
                },
                "png",
            ),
            (
                ResultCapabilities::Link {
                    extension: "pdf".into(),
                    media_type: "application/pdf".into(),
                },
                "pdf",
            ),
        ];

        for (capability, extension) in capabilities {
            assert_eq!(capability.workspace_output_extension(), extension);
        }
    }

    #[test]
    fn adds_classes_only_to_the_checked_svg_root() {
        let svg = add_embedded_svg_class(
            r#"<?xml version="1.0"?><svg class="renderer" xmlns="http://www.w3.org/2000/svg"><g class="child"/></svg>"#,
            "typst-inline-math",
        )
        .expect("checked SVG");

        assert_eq!(
            svg,
            r#"<svg class="renderer typst-inline-math" xmlns="http://www.w3.org/2000/svg"><g class="child"/></svg>"#
        );
    }
}
