//! Checked light and dark image pairs in authored Markdown.
//!
//! The parser deliberately recognizes the construct only after
//! `pulldown-cmark` has identified one whole paragraph. That keeps the
//! original Markdown offsets available to diagnostics and avoids introducing a
//! second block parser beside the Markdown parser Heine already uses.

use std::{fmt::Write, ops::Range};

use pulldown_cmark::Event;
use serde::Deserialize;

use crate::asset::AssetLinks;

use super::{
    HtmlClassList, RenderError, closing_colon_fence_run, push_html_attribute, push_html_text,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPicture {
    alt: toml::Spanned<String>,
    light: toml::Spanned<String>,
    dark: toml::Spanned<String>,
    caption: Option<toml::Spanned<String>>,
    class: Option<toml::Spanned<String>>,
    width: Option<toml::Spanned<u32>>,
    height: Option<toml::Spanned<u32>>,
}

#[derive(Debug)]
pub(crate) struct Error {
    message: String,
    span: Range<usize>,
    label: &'static str,
    help: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl Error {
    pub(super) fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    pub(super) fn label(&self) -> &'static str {
        self.label
    }

    pub(super) fn help(&self) -> &str {
        &self.help
    }
}

/// Replaces complete paragraph event blocks that use the native `picture`
/// construct. Other paragraphs retain their normal Markdown event stream.
pub(super) fn transform(
    input: &str,
    events: Vec<(Event<'_>, Range<usize>)>,
    assets: Option<&AssetLinks>,
) -> Result<Vec<(Event<'static>, Range<usize>)>, RenderError> {
    let mut output = Vec::with_capacity(events.len());
    let mut events = events.into_iter();

    while let Some((event, span)) = events.next() {
        if !matches!(event, Event::Start(pulldown_cmark::Tag::Paragraph)) {
            output.push((event.into_static(), span));
            continue;
        }

        let mut paragraph = vec![(event, span.clone())];
        for event in events.by_ref() {
            let ends_paragraph = matches!(event.0, Event::End(pulldown_cmark::TagEnd::Paragraph));
            paragraph.push(event);
            if ends_paragraph {
                break;
            }
        }

        let source = &input[span.clone()];
        match parse(source, span.start, assets).map_err(RenderError::Picture)? {
            Some(picture) => output.push((Event::Html(picture.html().into()), span)),
            None => output.extend(
                paragraph
                    .into_iter()
                    .map(|(event, span)| (event.into_static(), span)),
            ),
        }
    }

    Ok(output)
}

/// Parses and validates one picture paragraph without constructing its HTML.
/// Build-time source analysis uses this so validation cannot allocate rendered
/// markup that only the later template render would consume.
pub(super) fn validate_and_search_text(
    source: &str,
    offset: usize,
    assets: Option<&AssetLinks>,
) -> Result<Option<String>, Error> {
    parse(source, offset, assets).map(|picture| picture.map(Picture::search_text))
}

/// Provides facts-only callers with the authored text of a syntactically valid
/// picture directive. Resolved site sources use the checked parser; this
/// fallback keeps the standalone facts API independent of asset lookup.
pub(super) fn source_text(source: &str) -> Option<String> {
    let (body, _) = body(source)?;
    toml::from_str::<RawPicture>(body).ok().map(|raw| {
        PictureText {
            alt: raw.alt.into_inner(),
            caption: raw.caption.map(toml::Spanned::into_inner),
        }
        .into_text()
    })
}

/// Identifies a paragraph that begins a picture directive. Margin-footnote
/// planning uses this to ignore directive metadata as prose.
pub(super) fn starts_directive(source: &str) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.trim_end_matches('\r') == ":::picture")
}

fn parse(
    source: &str,
    offset: usize,
    assets: Option<&AssetLinks>,
) -> Result<Option<Picture>, Error> {
    if !starts_directive(source) {
        return Ok(None);
    }
    let Some((body, body_offset)) = body(source) else {
        return Err(Error {
            message: "Markdown picture directive is missing its closing ::: line".into(),
            span: offset..offset + source.len(),
            label: "unterminated picture directive",
            help: "close the standalone picture block with :::".into(),
        });
    };
    let raw = toml::from_str::<RawPicture>(body).map_err(|error| {
        let span = error
            .span()
            .map(|span| offset + body_offset + span.start..offset + body_offset + span.end)
            .unwrap_or(offset..offset + source.len());
        Error {
            message: format!("invalid Markdown picture fields: {error}"),
            span,
            label: "invalid picture field",
            help: "use the required alt, light, and dark fields, with optional caption and positive integer width and height fields".into(),
        }
    })?;
    if let Some(caption) = &raw.caption
        && caption.get_ref().trim().is_empty()
    {
        return Err(Error {
            message: "Markdown picture caption must not be empty".into(),
            span: value_span(caption, offset + body_offset),
            label: "empty caption",
            help: "omit caption when the image needs no visible caption, or provide its text"
                .into(),
        });
    }
    let Some(assets) = assets else {
        return Err(Error {
            message: "Markdown pictures require checked asset resolution".into(),
            span: offset..offset + source.len(),
            label: "picture cannot resolve its assets",
            help: "render this Markdown through Heine's page or template Markdown APIs".into(),
        });
    };

    let light = resolve("light", &raw.light, offset + body_offset, assets)?;
    let dark = resolve("dark", &raw.dark, offset + body_offset, assets)?;
    if let (Some((light_width, light_height)), Some((dark_width, dark_height))) =
        (light.dimensions(), dark.dimensions())
        && u64::from(light_width) * u64::from(dark_height)
            != u64::from(dark_width) * u64::from(light_height)
    {
        return Err(Error {
            message: "light and dark Markdown picture assets have different aspect ratios".into(),
            span: value_span(&raw.dark, offset + body_offset),
            label: "dark image ratio differs here",
            help: "use visual variants with the same aspect ratio so selecting a color scheme does not change the layout".into(),
        });
    }

    let dimensions = dimensions(
        &raw,
        light.dimensions(),
        dark.dimensions(),
        offset + body_offset,
    )?;
    Ok(Some(Picture {
        alt: raw.alt.into_inner(),
        light,
        dark,
        caption: raw.caption.map(toml::Spanned::into_inner),
        class: raw
            .class
            .as_ref()
            .map(|class| {
                HtmlClassList::parse(class.get_ref()).map_err(|reason| Error {
                    message: format!("Markdown picture {reason}"),
                    span: value_span(class, offset + body_offset),
                    label: "invalid picture class",
                    help: "use one or more distinct HTML class tokens separated by whitespace"
                        .into(),
                })
            })
            .transpose()?,
        dimensions,
    }))
}

struct Picture {
    alt: String,
    light: crate::asset::AssetLink,
    dark: crate::asset::AssetLink,
    caption: Option<String>,
    class: Option<HtmlClassList>,
    dimensions: Option<(u32, u32)>,
}

impl Picture {
    fn html(self) -> String {
        generated_html(
            self.light.url(),
            self.dark.url(),
            &self.alt,
            self.caption.as_deref(),
            self.class.as_ref(),
            self.dimensions,
        )
    }

    fn search_text(self) -> String {
        PictureText {
            alt: self.alt,
            caption: self.caption,
        }
        .into_text()
    }
}

pub(crate) fn generated_html(
    light: &str,
    dark: &str,
    alt: &str,
    caption: Option<&str>,
    class: Option<&HtmlClassList>,
    dimensions: Option<(u32, u32)>,
) -> String {
    let mut html = String::new();
    if caption.is_some() {
        html.push_str("<figure");
        push_class(class, &mut html);
        html.push_str(">\n");
    }
    html.push_str("<picture");
    push_class(class, &mut html);
    html.push_str("><source media=\"(prefers-color-scheme: dark)\" srcset=\"");
    push_html_attribute(dark, &mut html);
    html.push_str("\"><img src=\"");
    push_html_attribute(light, &mut html);
    html.push('"');
    if let Some((width, height)) = dimensions {
        write!(html, " width=\"{width}\" height=\"{height}\"")
            .expect("writing to a String cannot fail");
    }
    html.push_str(" alt=\"");
    push_html_attribute(alt, &mut html);
    html.push_str("\"></picture>");
    if let Some(caption) = caption {
        html.push_str("\n<figcaption>");
        push_html_text(caption, &mut html);
        html.push_str("</figcaption>\n</figure>");
    }
    html.push('\n');
    html
}

fn push_class(class: Option<&HtmlClassList>, html: &mut String) {
    if let Some(class) = class {
        html.push_str(" class=\"");
        push_html_attribute(class.as_str(), html);
        html.push('"');
    }
}

pub(crate) fn dimensions_for_picture(
    width: Option<u32>,
    height: Option<u32>,
    light_intrinsic: Option<(u32, u32)>,
    dark_intrinsic: Option<(u32, u32)>,
) -> Result<Option<(u32, u32)>, &'static str> {
    if let (Some((light_width, light_height)), Some((dark_width, dark_height))) =
        (light_intrinsic, dark_intrinsic)
        && u64::from(light_width) * u64::from(dark_height)
            != u64::from(dark_width) * u64::from(light_height)
    {
        return Err("light and dark picture images have different aspect ratios");
    }
    match (width, height, light_intrinsic) {
        (None, None, Some(dimensions)) => Ok(Some(dimensions)),
        (None, None, None) => Ok(None),
        (Some(width), Some(height), Some((natural_width, natural_height)))
            if u64::from(width) * u64::from(natural_height)
                != u64::from(height) * u64::from(natural_width) =>
        {
            Err("picture width and height would distort the light image")
        }
        (Some(width), Some(height), _) => Ok(Some((width, height))),
        (Some(width), None, Some((natural_width, natural_height))) => {
            Ok(Some((width, scaled(width, natural_height, natural_width))))
        }
        (None, Some(height), Some((natural_width, natural_height))) => Ok(Some((
            scaled(height, natural_width, natural_height),
            height,
        ))),
        (Some(_), None, None) | (None, Some(_), None) => {
            Err("picture cannot derive the missing dimension")
        }
    }
}

struct PictureText {
    alt: String,
    caption: Option<String>,
}

impl PictureText {
    fn into_text(self) -> String {
        let mut text = self.alt;
        if let Some(caption) = self.caption
            && !caption.is_empty()
        {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&caption);
        }
        text
    }
}

fn resolve(
    name: &str,
    value: &toml::Spanned<String>,
    offset: usize,
    assets: &AssetLinks,
) -> Result<crate::asset::AssetLink, Error> {
    let Some(id) = value.get_ref().strip_prefix("asset:") else {
        return Err(Error {
            message: format!("Markdown picture {name} source must use an asset: reference"),
            span: value_span(value, offset),
            label: "unchecked picture source",
            help: "use asset:<id> for a copied image below content/".into(),
        });
    };
    let asset = assets.resolve(id).map_err(|error| Error {
        message: format!("could not resolve Markdown picture {name} asset {id:?}: {error}"),
        span: value_span(value, offset),
        label: "invalid picture asset reference",
        help: "use an existing copied asset ID relative to content/, without a leading '/', or '.' or '..' path segments".into(),
    })?;
    if !asset.is_image() {
        return Err(Error {
            message: format!("Markdown picture {name} source is not an image asset"),
            span: value_span(value, offset),
            label: "non-image picture source",
            help: "use a copied image asset for each picture source".into(),
        });
    }
    Ok(asset)
}

fn dimensions(
    raw: &RawPicture,
    light_intrinsic: Option<(u32, u32)>,
    dark_intrinsic: Option<(u32, u32)>,
    offset: usize,
) -> Result<Option<(u32, u32)>, Error> {
    let width = raw.width.as_ref();
    let height = raw.height.as_ref();
    for value in [width, height].into_iter().flatten() {
        if *value.get_ref() == 0 {
            return Err(Error {
                message: "Markdown picture dimensions must be positive integers".into(),
                span: value_span(value, offset),
                label: "zero dimension",
                help: "use a positive width or height".into(),
            });
        }
    }
    match (width, height, light_intrinsic, dark_intrinsic) {
        (None, None, Some(dimensions), Some(_)) => Ok(Some(dimensions)),
        (None, None, _, _) => Ok(None),
        (Some(width), Some(height), Some((natural_width, natural_height)), _) => {
            if u64::from(*width.get_ref()) * u64::from(natural_height)
                != u64::from(*height.get_ref()) * u64::from(natural_width)
            {
                return Err(Error {
                    message: "Markdown picture width and height would distort the light image".into(),
                    span: value_span(height, offset),
                    label: "height does not preserve the image ratio",
                    help: "use dimensions with the light image's intrinsic aspect ratio, or omit them to use its detected dimensions".into(),
                });
            }
            Ok(Some((*width.get_ref(), *height.get_ref())))
        }
        (Some(width), Some(height), None, _) => Ok(Some((*width.get_ref(), *height.get_ref()))),
        (Some(width), None, Some((natural_width, natural_height)), _) => Ok(Some((
            *width.get_ref(),
            scaled(*width.get_ref(), natural_height, natural_width),
        ))),
        (None, Some(height), Some((natural_width, natural_height)), _) => Ok(Some((
            scaled(*height.get_ref(), natural_width, natural_height),
            *height.get_ref(),
        ))),
        (Some(value), None, None, _) | (None, Some(value), None, _) => Err(Error {
            message: "Markdown picture cannot derive the missing dimension".into(),
            span: value_span(value, offset),
            label: "one dimension is insufficient here",
            help: "provide both width and height when the light image has no detected dimensions"
                .into(),
        }),
    }
}

fn scaled(value: u32, numerator: u32, denominator: u32) -> u32 {
    let numerator = u64::from(value) * u64::from(numerator);
    let rounded = (numerator + u64::from(denominator) / 2) / u64::from(denominator);
    rounded.min(u64::from(u32::MAX)) as u32
}

fn value_span<T>(value: &toml::Spanned<T>, offset: usize) -> Range<usize> {
    let span = value.span();
    offset + span.start..offset + span.end
}

fn body(source: &str) -> Option<(&str, usize)> {
    let lines = lines(source);
    if lines.len() < 2
        || lines.first()?.1 != ":::picture"
        || closing_colon_fence_run(lines.last()?.1) != Some(3)
    {
        return None;
    }
    let body_start = lines.first()?.0 + lines.first()?.2;
    let body_end = lines.last()?.0;
    Some((&source[body_start..body_end], body_start))
}

/// Returns each source line without its line ending, plus its byte offset and
/// original byte length. The latter preserves CRLF offsets in diagnostics.
fn lines(source: &str) -> Vec<(usize, &str, usize)> {
    let mut lines = Vec::new();
    let mut offset = 0;
    for part in source.split_inclusive('\n') {
        let line = part.trim_end_matches(['\r', '\n']);
        lines.push((offset, line, part.len()));
        offset += part.len();
    }
    if !source.is_empty() && !source.ends_with('\n') {
        return lines;
    }
    if source.is_empty() {
        lines.push((0, "", 0));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::dimensions_for_picture;

    #[test]
    fn generated_pictures_check_and_derive_dimensions_from_the_light_fallback() {
        assert_eq!(
            dimensions_for_picture(None, None, Some((20, 10)), Some((40, 20))),
            Ok(Some((20, 10)))
        );
        assert_eq!(
            dimensions_for_picture(Some(30), None, Some((20, 10)), None),
            Ok(Some((30, 15)))
        );
        assert_eq!(
            dimensions_for_picture(Some(30), Some(15), None, Some((20, 10))),
            Ok(Some((30, 15)))
        );
        assert_eq!(
            dimensions_for_picture(Some(30), None, None, Some((20, 10))),
            Err("picture cannot derive the missing dimension")
        );
    }

    #[test]
    fn generated_pictures_reject_incompatible_known_dimensions() {
        assert_eq!(
            dimensions_for_picture(None, None, Some((20, 10)), Some((10, 20))),
            Err("light and dark picture images have different aspect ratios")
        );
        assert_eq!(
            dimensions_for_picture(Some(20), Some(20), Some((20, 10)), Some((40, 20))),
            Err("picture width and height would distort the light image")
        );
    }
}
