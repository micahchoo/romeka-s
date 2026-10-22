//! The W3C Web Annotation JSON of one media: the server's IRIs, the parser and
//! the renderer. This is the only Rust file that knows the wire key names.
//!
//! The parser is the boundary. It checks the target against the media's canvas,
//! the selector against the media's `Surface` and `Extent`, and every body. What
//! comes out is a [`Draft`], the canonical form the database stores, so a client
//! that PUTs back what it GOT round-trips byte for byte.

use omeka_db::DbError;
use omeka_db::ids::{ItemId, MediaId};
use omeka_media::kind::{MediaKind, Surface};
use serde_json::{Map, Value, json};

use super::model::{
    Body, Draft, Extensions, MAX_BODIES, MAX_EXTENSION_BYTES, MAX_TEXT_CHARS, MediaRef, Motivation,
    Problem, Selector, TextFormat,
};
use super::{Annotation, AnnotationError, AnnotationId};

/// The largest annotation document the server reads.
pub(crate) const MAX_DOC_BYTES: usize = 64 * 1024;

const MEDIA_FRAGS: &str = "http://www.w3.org/TR/media-frags/";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// The server's IRIs, from the configured base. The one place the URL scheme is
/// written.
pub struct Iris {
    base: String,
}

impl Iris {
    /// The server's IRIs, from the configured base.
    #[must_use]
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim().trim_end_matches('/').to_string(),
        }
    }

    /// `/iiif/3/item/{item}/canvas/{media}`: the canvas every annotation
    /// targets, whether or not the kind paints one.
    #[must_use]
    pub fn canvas(&self, media: &MediaRef) -> String {
        format!(
            "{}/iiif/3/item/{}/canvas/{}",
            self.base, media.item.0, media.id.0
        )
    }

    #[must_use]
    pub fn item_manifest(&self, item: ItemId) -> String {
        format!("{}/iiif/3/item/{}/manifest", self.base, item.0)
    }

    #[must_use]
    pub fn media_manifest(&self, media: MediaId) -> String {
        format!("{}/iiif/3/media/{}/manifest", self.base, media.0)
    }

    /// `/iiif/image/3/{media}`: the Image API service an image canvas names,
    /// and the one a `Romeka-Surface` header points the editor's zoom at.
    #[must_use]
    pub fn image(&self, media: MediaId) -> String {
        format!("{}/iiif/image/3/{}", self.base, media.0)
    }

    /// `/annotations/media/{media}/`: the WAP container.
    #[must_use]
    pub fn container(&self, media: MediaId) -> String {
        format!("{}/annotations/media/{}/", self.base, media.0)
    }

    /// `…/page`, with the keyset cursor when this is not the first page.
    #[must_use]
    pub fn page(&self, media: MediaId, after: Option<AnnotationId>) -> String {
        let page = format!("{}/annotations/media/{}/page", self.base, media.0);
        match after {
            Some(AnnotationId(id)) => format!("{page}?after={id}"),
            None => page,
        }
    }

    #[must_use]
    pub fn annotation(&self, id: AnnotationId) -> String {
        format!("{}/annotations/{}", self.base, id.0)
    }

    /// `captions.vtt`, narrowed to one language when the caller names one.
    #[must_use]
    pub fn captions(&self, media: MediaId, lang: Option<&str>) -> String {
        let path = format!("{}/annotations/media/{}/captions.vtt", self.base, media.0);
        match lang {
            Some(lang) => format!("{path}?lang={lang}"),
            None => path,
        }
    }

    /// The item's Content Search 2.0 service.
    #[must_use]
    pub fn search(&self, item: ItemId) -> String {
        format!("{}/iiif/search/2/item/{}", self.base, item.0)
    }
}

/// Parse a client document for `media` into the canonical draft.
pub(crate) fn parse(doc: &[u8], media: &MediaRef, iris: &Iris) -> Result<Draft, AnnotationError> {
    if doc.len() > MAX_DOC_BYTES {
        return Err(AnnotationError::TooLarge);
    }
    let value: Value = serde_json::from_slice(doc).map_err(|_| Problem::NotAnnotation)?;
    if !value.is_object() || !is_annotation(&value) {
        return Err(Problem::NotAnnotation.into());
    }
    let motivation = match text_member(&value, "motivation") {
        None => Motivation::Commenting,
        Some(text) => Motivation::of(text).ok_or(Problem::BadMotivation)?,
    };
    let draft = Draft {
        motivation,
        selector: selector_of(&value, media, iris)?,
        extensions: extensions_of(&value)?,
        bodies: bodies_of(member(&value, "body"))?,
    };
    check_draft(&draft)?;
    Ok(draft)
}

/// The rows' stored body JSON is the canonical array, so the same parser reads
/// it back.
pub(crate) fn bodies_of_row(body_json: &str) -> Result<Vec<Body>, DbError> {
    let value: Value = serde_json::from_str(body_json)
        .map_err(|error| row_error("body_json", &error.to_string()))?;
    bodies_of(Some(&value)).map_err(|problem| row_error("body_json", &problem.to_string()))
}

/// The text of one stored row's bodies, joined the way a track renders: what a
/// cue says, with a link body contributing nothing.
pub(crate) fn text_of_body(body_json: &str) -> Result<String, AnnotationError> {
    let bodies = bodies_of_row(body_json)?;
    Ok(bodies
        .iter()
        .filter_map(|body| match body {
            Body::Text { value, .. } => Some(value.as_str()),
            Body::Link { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// The W3C JSON of one stored annotation, without `@context` (the envelope
/// adds it).
#[must_use]
pub fn render(annotation: &Annotation, iris: &Iris) -> Value {
    let draft = &annotation.draft;
    let mut map = Map::new();
    map.insert("id".to_string(), iris.annotation(annotation.id).into());
    map.insert("type".to_string(), "Annotation".into());
    map.insert("motivation".to_string(), draft.motivation.as_str().into());
    map.insert("created".to_string(), iso(annotation.created).into());
    map.insert("modified".to_string(), iso(annotation.modified).into());
    if let Some(body) = bodies_value(&draft.bodies) {
        map.insert("body".to_string(), body);
    }
    map.insert(
        "target".to_string(),
        target_value(&draft.selector, annotation, iris),
    );
    for (key, value) in draft.extensions.iter() {
        map.insert(key.clone(), value.clone());
    }
    Value::Object(map)
}

/// The write shape of a draft: the canonical columns plus the derived ones.
pub(crate) struct Stored {
    pub selector_kind: &'static str,
    pub selector: Option<String>,
    pub time_ms: Option<(i64, i64)>,
    pub body_json: String,
    pub body_text: String,
    pub lang: Option<String>,
    pub extensions: Option<String>,
}

/// One draft as the repository stores it. `body_text` and `lang` are derived
/// here and nowhere else, so a write cannot forget them.
pub(crate) fn stored(draft: &Draft) -> Stored {
    let (selector_kind, selector, time_ms) = match &draft.selector {
        Selector::Whole => ("whole", None, None),
        Selector::Rect { x, y, w, h } => ("rect", Some(format!("{x},{y},{w},{h}")), None),
        Selector::Polygon(points) => (
            "polygon",
            Some(
                points
                    .iter()
                    .map(|(x, y)| format!("{x},{y}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            None,
        ),
        Selector::Time { start_ms, end_ms } => ("time", None, Some(time_ms(*start_ms, *end_ms))),
        Selector::BoxDuring {
            x,
            y,
            w,
            h,
            start_ms,
            end_ms,
        } => (
            "rect",
            Some(format!("{x},{y},{w},{h}")),
            Some(time_ms(*start_ms, *end_ms)),
        ),
    };
    let body_text = draft
        .bodies
        .iter()
        .filter_map(|body| match body {
            Body::Text { value, .. } => Some(value.as_str()),
            Body::Link { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let lang = draft.bodies.iter().find_map(|body| match body {
        Body::Text { language, .. } => language.clone(),
        Body::Link { .. } => None,
    });
    let extensions = (!draft.extensions.is_empty())
        .then(|| Value::Object(draft.extensions.map().clone()).to_string());
    Stored {
        selector_kind,
        selector,
        time_ms,
        body_json: bodies_value(&draft.bodies)
            .unwrap_or_else(|| Value::Array(Vec::new()))
            .to_string(),
        body_text,
        lang,
        extensions,
    }
}

fn time_ms(start_ms: u64, end_ms: u64) -> (i64, i64) {
    (
        i64::try_from(start_ms).unwrap_or(i64::MAX),
        i64::try_from(end_ms).unwrap_or(i64::MAX),
    )
}

fn row_error(column: &'static str, reason: &str) -> DbError {
    DbError::Column {
        column,
        reason: reason.to_string(),
    }
}

fn is_annotation(value: &Value) -> bool {
    match member(value, "type") {
        Some(Value::String(text)) => text == "Annotation",
        Some(Value::Array(types)) => types.iter().any(|kind| kind.as_str() == Some("Annotation")),
        _ => false,
    }
}

fn member<'v>(value: &'v Value, key: &str) -> Option<&'v Value> {
    value.as_object().and_then(|map| map.get(key))
}

fn text_member<'v>(value: &'v Value, key: &str) -> Option<&'v str> {
    member(value, key).and_then(Value::as_str)
}

fn selector_of(doc: &Value, media: &MediaRef, iris: &Iris) -> Result<Selector, Problem> {
    let canvas = iris.canvas(media);
    let target = member(doc, "target").ok_or(Problem::ForeignTarget)?;
    if let Some(iri) = target.as_str() {
        return if iri == canvas {
            Ok(Selector::Whole)
        } else {
            Err(Problem::ForeignTarget)
        };
    }
    let source = member(target, "source").ok_or(Problem::ForeignTarget)?;
    let source_iri = source
        .as_str()
        .or_else(|| text_member(source, "id"))
        .ok_or(Problem::ForeignTarget)?;
    if source_iri != canvas {
        return Err(Problem::ForeignTarget);
    }
    let Some(selector) = member(target, "selector").filter(|value| !value.is_null()) else {
        return Ok(Selector::Whole);
    };
    if media.kind.surface() == Surface::Whole {
        return Err(Problem::WholeFileOnly);
    }
    let kind = text_member(selector, "type").ok_or(Problem::BadSelector)?;
    let value = text_member(selector, "value").ok_or(Problem::BadSelector)?;
    match kind {
        "FragmentSelector" => fragment_of(value, media),
        "SvgSelector" => polygon_of(value, media),
        _ => Err(Problem::BadSelector),
    }
}

fn fragment_of(value: &str, media: &MediaRef) -> Result<Selector, Problem> {
    let mut boxed = None;
    let mut timed = None;
    for part in value.split('&') {
        if let Some(rest) = part.strip_prefix("t=") {
            timed = Some(rest);
        } else if let Some(rest) = part.strip_prefix("xywh=") {
            boxed = Some(rest);
        }
    }
    match (boxed, timed) {
        (Some(boxed), Some(timed)) => {
            shape_allowed(media, true)?;
            let Selector::Rect { x, y, w, h } = rect_of(boxed, media)? else {
                return Err(Problem::BadSelector);
            };
            let Selector::Time { start_ms, end_ms } = time_of(timed, media)? else {
                return Err(Problem::BadSelector);
            };
            Ok(Selector::BoxDuring {
                x,
                y,
                w,
                h,
                start_ms,
                end_ms,
            })
        }
        (Some(rest), None) => {
            shape_allowed(media, false)?;
            rect_of(rest, media)
        }
        (None, Some(rest)) => {
            if media.kind.surface() != Surface::Time {
                return Err(Problem::TimeOnImage);
            }
            time_of(rest, media)
        }
        (None, None) => Err(Problem::BadSelector),
    }
}

/// Whether `media` takes a shape: an image always, a video frame only over a
/// time range, and a sound never.
fn shape_allowed(media: &MediaRef, timed: bool) -> Result<(), Problem> {
    match (media.kind.surface(), media.kind) {
        (Surface::Image, _) if !timed => Ok(()),
        (Surface::Image, _) => Err(Problem::TimeOnImage),
        (Surface::Time, MediaKind::Video) if timed => Ok(()),
        (Surface::Time, MediaKind::Video) => Err(Problem::VideoShapeNeedsTime),
        (Surface::Time, _) => Err(Problem::ShapeOnSound),
        (Surface::Whole, _) => Err(Problem::WholeFileOnly),
    }
}

fn rect_of(rest: &str, media: &MediaRef) -> Result<Selector, Problem> {
    let (percent, numbers) = match rest.strip_prefix("percent:") {
        Some(numbers) => (true, numbers),
        None => (false, rest.strip_prefix("pixel:").unwrap_or(rest)),
    };
    let values = numbers
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Problem::BadSelector)?;
    let [x, y, w, h] = values.as_slice() else {
        return Err(Problem::BadSelector);
    };
    if [x, y, w, h]
        .iter()
        .any(|value| !value.is_finite() || **value < 0.0)
    {
        return Err(Problem::BadSelector);
    }
    if *w <= 0.0 || *h <= 0.0 {
        return Err(Problem::BadSelector);
    }
    let (x, y, w, h) = if percent {
        let Some((width, height)) = media.extent.size else {
            return Err(Problem::BadSelector);
        };
        let width = f64::from(width);
        let height = f64::from(height);
        (
            x / 100.0 * width,
            y / 100.0 * height,
            w / 100.0 * width,
            h / 100.0 * height,
        )
    } else {
        (*x, *y, *w, *h)
    };
    let (x, y, w, h) = (round_u32(x)?, round_u32(y)?, round_u32(w)?, round_u32(h)?);
    if w == 0 || h == 0 {
        return Err(Problem::BadSelector);
    }
    if let Some((width, height)) = media.extent.size
        && (u64::from(x) + u64::from(w) > u64::from(width)
            || u64::from(y) + u64::from(h) > u64::from(height))
    {
        return Err(Problem::BoxOutsideCanvas);
    }
    Ok(Selector::Rect { x, y, w, h })
}

fn time_of(rest: &str, media: &MediaRef) -> Result<Selector, Problem> {
    let parts: Vec<&str> = rest.split(',').collect();
    let [start, end] = parts.as_slice() else {
        return Err(Problem::BadSelector);
    };
    let start_ms =
        ms_of(start.parse().map_err(|_| Problem::BadSelector)?).ok_or(Problem::BadSelector)?;
    let end_ms =
        ms_of(end.parse().map_err(|_| Problem::BadSelector)?).ok_or(Problem::BadSelector)?;
    if end_ms <= start_ms {
        return Err(Problem::EmptyTimeRange);
    }
    if start_ms > i64::MAX as u64 || end_ms > i64::MAX as u64 {
        return Err(Problem::BadSelector);
    }
    if let Some(duration) = media.extent.duration_ms
        && end_ms > duration
    {
        return Err(Problem::PastDuration);
    }
    Ok(Selector::Time { start_ms, end_ms })
}

fn polygon_of(value: &str, media: &MediaRef) -> Result<Selector, Problem> {
    shape_allowed(media, false)?;
    let lower = value.to_ascii_lowercase();
    let start = lower.find("<polygon").ok_or(Problem::BadSelector)?;
    let tag = &value[start..];
    let end = tag.find('>').ok_or(Problem::BadSelector)?;
    let tag = &tag[..end];
    let points_at = tag
        .to_ascii_lowercase()
        .find("points=")
        .ok_or(Problem::BadSelector)?;
    let after = &tag[points_at + "points=".len()..];
    let quote = after.chars().next().ok_or(Problem::BadSelector)?;
    if quote != '"' && quote != '\'' {
        return Err(Problem::BadSelector);
    }
    let rest = &after[quote.len_utf8()..];
    let close = rest.find(quote).ok_or(Problem::BadSelector)?;
    polygon_points(&rest[..close], media)
}

fn polygon_points(text: &str, media: &MediaRef) -> Result<Selector, Problem> {
    let mut numbers = Vec::new();
    for token in text.split(|character: char| character.is_whitespace() || character == ',') {
        if token.is_empty() {
            continue;
        }
        numbers.push(token.parse::<f64>().map_err(|_| Problem::BadSelector)?);
    }
    if numbers.len() % 2 != 0 {
        return Err(Problem::BadSelector);
    }
    let count = numbers.len() / 2;
    if !(3..=500).contains(&count) {
        return Err(Problem::BadSelector);
    }
    let mut points = Vec::with_capacity(count);
    for pair in numbers.chunks(2) {
        let [x, y] = pair else {
            return Err(Problem::BadSelector);
        };
        let (x, y) = (round_u32(*x)?, round_u32(*y)?);
        if let Some((width, height)) = media.extent.size
            && (x > width || y > height)
        {
            return Err(Problem::BoxOutsideCanvas);
        }
        points.push((x, y));
    }
    Ok(Selector::Polygon(points))
}

fn round_u32(value: f64) -> Result<u32, Problem> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return Err(Problem::BadSelector);
    }
    Ok(value.round() as u32)
}

fn ms_of(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let milliseconds = (seconds * 1000.0).round();
    if milliseconds > u64::MAX as f64 {
        None
    } else {
        Some(milliseconds as u64)
    }
}

fn bodies_of(value: Option<&Value>) -> Result<Vec<Body>, Problem> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let entries: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(_) => vec![value],
        _ => return Err(Problem::BadBody),
    };
    if entries.len() > MAX_BODIES {
        return Err(Problem::TooManyBodies);
    }
    entries.iter().map(|entry| body_of(entry)).collect()
}

fn body_of(value: &Value) -> Result<Body, Problem> {
    if !value.is_object() {
        return Err(Problem::BadBody);
    }
    let purpose = purpose_of(value)?;
    if text_member(value, "type") == Some("TextualBody") {
        let raw = text_member(value, "value").ok_or(Problem::BadBody)?;
        let format = match text_member(value, "format") {
            None => TextFormat::Plain,
            Some(text) => TextFormat::of(text).ok_or(Problem::BadBody)?,
        };
        let cleaned = match format {
            TextFormat::Html => omeka_core::purify::purify(raw),
            TextFormat::Plain | TextFormat::Markdown => raw.to_string(),
        };
        return Ok(Body::Text {
            value: cleaned,
            format,
            language: language_of(value)?,
            purpose,
        });
    }
    let id = text_member(value, "id").ok_or(Problem::BadBody)?;
    if !is_web_iri(id) {
        return Err(Problem::BadBody);
    }
    Ok(Body::Link {
        id: id.to_string(),
        purpose,
    })
}

fn language_of(value: &Value) -> Result<Option<String>, Problem> {
    match member(value, "language") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text))
            if !text.is_empty()
                && text.len() <= 35
                && text
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-') =>
        {
            Ok(Some(text.clone()))
        }
        _ => Err(Problem::BadBody),
    }
}

fn purpose_of(value: &Value) -> Result<Option<Motivation>, Problem> {
    match text_member(value, "purpose") {
        None => Ok(None),
        Some(text) => Motivation::of(text).map(Some).ok_or(Problem::BadBody),
    }
}

fn is_web_iri(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && text.len() > "https://".len()
        && !text.contains(char::is_whitespace)
}

fn extensions_of(doc: &Value) -> Result<Extensions, Problem> {
    let mut map = Map::new();
    if let Some(object) = doc.as_object() {
        for (key, value) in object {
            if !is_compact_iri(key) {
                continue;
            }
            map.insert(key.clone(), value.clone());
        }
    }
    if Value::Object(map.clone()).to_string().len() > MAX_EXTENSION_BYTES {
        return Err(Problem::ExtensionsTooLarge);
    }
    Ok(Extensions::of(map))
}

fn is_compact_iri(key: &str) -> bool {
    match key.split_once(':') {
        Some((prefix, local)) => {
            !prefix.is_empty()
                && !prefix.starts_with('@')
                && !local.is_empty()
                && prefix.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
                })
        }
        None => false,
    }
}

fn check_draft(draft: &Draft) -> Result<(), Problem> {
    let text_chars: usize = draft
        .bodies
        .iter()
        .map(|body| match body {
            Body::Text { value, .. } => value.chars().count(),
            Body::Link { .. } => 0,
        })
        .sum();
    if text_chars > MAX_TEXT_CHARS {
        return Err(Problem::TextTooLong);
    }
    let has_text = draft.bodies.iter().any(|body| match body {
        Body::Text { value, .. } => !value.trim().is_empty(),
        Body::Link { .. } => false,
    });
    if matches!(
        draft.motivation,
        Motivation::Tagging | Motivation::Supplementing
    ) && !has_text
    {
        return Err(Problem::NoTextBody);
    }
    if draft.motivation == Motivation::Supplementing
        && !matches!(draft.selector, Selector::Time { .. })
    {
        return Err(Problem::CaptionNeedsTime);
    }
    Ok(())
}

fn bodies_value(bodies: &[Body]) -> Option<Value> {
    match bodies {
        [] => None,
        [one] => Some(body_value(one)),
        many => Some(Value::Array(many.iter().map(body_value).collect())),
    }
}

fn body_value(body: &Body) -> Value {
    match body {
        Body::Text {
            value,
            format,
            language,
            purpose,
        } => {
            let mut map = Map::new();
            map.insert("type".to_string(), "TextualBody".into());
            map.insert("value".to_string(), value.clone().into());
            map.insert("format".to_string(), format.as_str().into());
            if let Some(language) = language {
                map.insert("language".to_string(), language.clone().into());
            }
            if let Some(purpose) = purpose {
                map.insert("purpose".to_string(), purpose.as_str().into());
            }
            Value::Object(map)
        }
        Body::Link { id, purpose } => {
            let mut map = Map::new();
            map.insert("id".to_string(), id.clone().into());
            if let Some(purpose) = purpose {
                map.insert("purpose".to_string(), purpose.as_str().into());
            }
            Value::Object(map)
        }
    }
}

fn target_value(selector: &Selector, annotation: &Annotation, iris: &Iris) -> Value {
    let canvas = iris.canvas(&annotation.media);
    if selector == &Selector::Whole {
        return canvas.into();
    }
    let source = json!({
        "id": canvas,
        "type": "Canvas",
        "partOf": [{ "id": iris.item_manifest(annotation.media.item), "type": "Manifest" }],
    });
    json!({
        "type": "SpecificResource",
        "source": source,
        "selector": selector_value(selector),
    })
}

fn selector_value(selector: &Selector) -> Value {
    match selector {
        Selector::Whole => Value::Null,
        Selector::Rect { x, y, w, h } => json!({
            "type": "FragmentSelector",
            "conformsTo": MEDIA_FRAGS,
            "value": format!("xywh=pixel:{x},{y},{w},{h}"),
        }),
        Selector::Polygon(points) => {
            let points = points
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .collect::<Vec<_>>()
                .join(" ");
            json!({
                "type": "SvgSelector",
                "value": format!(
                    "<svg xmlns=\"{SVG_NAMESPACE}\"><polygon points=\"{points}\"/></svg>"
                ),
            })
        }
        Selector::Time { start_ms, end_ms } => json!({
            "type": "FragmentSelector",
            "conformsTo": MEDIA_FRAGS,
            "value": format!("t={},{}", seconds(*start_ms), seconds(*end_ms)),
        }),
        Selector::BoxDuring {
            x,
            y,
            w,
            h,
            start_ms,
            end_ms,
        } => json!({
            "type": "FragmentSelector",
            "conformsTo": MEDIA_FRAGS,
            "value": format!(
                "xywh=pixel:{x},{y},{w},{h}&t={},{}",
                seconds(*start_ms),
                seconds(*end_ms)
            ),
        }),
    }
}

/// Seconds with at most three decimals, trailing zeros dropped.
fn seconds(milliseconds: u64) -> String {
    let whole = milliseconds / 1000;
    let fraction = milliseconds % 1000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_string()
    }
}

/// A stored clock as the wire writes it: seconds, UTC.
#[must_use]
pub fn iso(at: chrono::NaiveDateTime) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
