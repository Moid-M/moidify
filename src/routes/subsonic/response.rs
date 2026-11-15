//! Subsonic response building: an ordered element tree serialized as XML
//! (ElementTree byte style), JSON (`f=json`, derived from the same tree the
//! way Python derives it from the serialized XML), or JSONP (`f=jsonp`).
//!
//! Like Python's `_ok`/`_error`/`_xml_response`:
//! - XML: `<?xml version='1.0' encoding='UTF-8'?>` declaration, single-line
//!   document, self-closing tags as `<tag attrs />`, `xmlns` on the root only.
//! - JSON: root object is `{"subsonic-response": {...attrs, children...}}`
//!   with `xmlns` omitted (ElementTree strips it on parse) and repeated child
//!   tags grouped into arrays. Pretty-printed with 2-space indent like
//!   `json.dumps(..., indent=2)`; all values are strings.
//! - JSONP: `{callback}({json})`; falls back to XML when `callback` is absent.

use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{Map, Value};

pub const API_VERSION: &str = "1.16.1";
const XMLNS: &str = "http://subsonic.org/restapi";

/// One element: tag + ordered attributes + ordered children. Attributes are
/// always strings (Python ElementTree can only serialize strings); there are
/// no text nodes anywhere in the Subsonic surface.
#[derive(Clone, Debug, Default)]
pub struct Elem {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<Elem>,
}

impl Elem {
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Append an attribute (builder style). Values are rendered as strings.
    pub fn attr(mut self, key: &str, value: impl ToString) -> Self {
        self.attrs.push((key.to_string(), value.to_string()));
        self
    }

    /// Append a child element.
    pub fn with(mut self, child: Elem) -> Self {
        self.children.push(child);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

/// `<subsonic-response status="ok" version="...">` root element.
pub fn ok() -> Elem {
    Elem::new("subsonic-response")
        .attr("status", "ok")
        .attr("version", API_VERSION)
}

/// `<subsonic-response status="failed" version="..."><error code message/>`.
pub fn error(code: u16, message: &str) -> Elem {
    Elem::new("subsonic-response")
        .attr("status", "failed")
        .attr("version", API_VERSION)
        .with(
            Elem::new("error")
                .attr("code", code)
                .attr("message", message),
        )
}

/// Response format requested via `f=` / `callback=` query parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Xml,
    Json,
    Jsonp(String),
}

impl Format {
    /// Python `_xml_response(request)` argument plumbing: `f=json` → JSON,
    /// `f=jsonp` *with* a callback → JSONP, anything else (including `jsonp`
    /// without callback) → XML.
    pub fn new(f: Option<&str>, callback: Option<&str>) -> Self {
        match f {
            Some("json") => Format::Json,
            Some("jsonp") => match callback {
                Some(cb) if !cb.is_empty() => Format::Jsonp(cb.to_string()),
                _ => Format::Xml,
            },
            _ => Format::Xml,
        }
    }
}

/// Render `root` in the requested format. Always HTTP 200 (Subsonic carries
/// failures in the body, like Python).
pub fn render(fmt: &Format, root: &Elem) -> Response {
    match fmt {
        Format::Xml => {
            let mut body = String::with_capacity(256);
            body.push_str("<?xml version='1.0' encoding='UTF-8'?>\n");
            write_xml(&mut body, root, true);
            text_response(body, "application/xml")
        }
        Format::Json => text_response(to_json(root), "application/json"),
        Format::Jsonp(cb) => {
            text_response(format!("{cb}({})", to_json(root)), "application/javascript")
        }
    }
}

fn text_response(body: String, content_type: &'static str) -> Response {
    let mut resp = body.into_response();
    if let Ok(v) = HeaderValue::from_str(content_type) {
        resp.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    resp
}

/// `json.dumps({"subsonic-response": el_to_dict(root)}, indent=2)` — the JSON
/// shape mirrors Python's `_xml_to_json` exactly (attrs first, children
/// grouped, first occurrence stays a value, repeats promote to an array).
fn to_json(root: &Elem) -> String {
    let wrapper = Map::from_iter([("subsonic-response".to_string(), elem_to_value(root))]);
    serde_json::to_string_pretty(&Value::Object(wrapper)).expect("infallible")
}

fn elem_to_value(elem: &Elem) -> Value {
    let mut map = Map::new();
    for (k, v) in &elem.attrs {
        map.insert(k.clone(), Value::String(v.clone()));
    }
    // Group children by tag preserving first-appearance order.
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for child in &elem.children {
        let value = elem_to_value(child);
        match groups.iter_mut().find(|(tag, _)| tag == &child.tag) {
            Some((_, list)) => list.push(value),
            None => groups.push((child.tag.clone(), vec![value])),
        }
    }
    for (tag, mut list) in groups {
        let value = if list.len() == 1 {
            list.pop().expect("len checked")
        } else {
            Value::Array(list)
        };
        map.insert(tag, value);
    }
    Value::Object(map)
}

/// ElementTree `tostring(..., encoding="UTF-8", xml_declaration=True)` —
/// single line, root gains the `xmlns` namespace attribute, self-closing
/// tags are written as `<tag attrs />` (space before the slash).
fn write_xml(buf: &mut String, elem: &Elem, is_root: bool) {
    buf.push('<');
    buf.push_str(&elem.tag);
    for (k, v) in &elem.attrs {
        buf.push(' ');
        buf.push_str(k);
        buf.push_str("=\"");
        escape_attr(buf, v);
        buf.push('"');
    }
    if is_root {
        buf.push_str(" xmlns=\"");
        escape_attr(buf, XMLNS);
        buf.push('"');
    }
    if elem.children.is_empty() {
        buf.push_str(" />");
        return;
    }
    buf.push('>');
    for child in &elem.children {
        write_xml(buf, child, false);
    }
    buf.push_str("</");
    buf.push_str(&elem.tag);
    buf.push('>');
}

/// ElementTree `_escape_attrib`: `& < > "` plus tab/newline/CR as character
/// references. Single quotes are kept literal (attributes use double quotes).
fn escape_attr(buf: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            '"' => buf.push_str("&quot;"),
            '\t' => buf.push_str("&#x9;"),
            '\n' => buf.push_str("&#xA;"),
            '\r' => buf.push_str("&#xD;"),
            other => buf.push(other),
        }
    }
}

/// FastAPI validation error for a missing required query parameter —
/// byte-shape of `{"detail":[{"type":"missing","loc":["query","<name>"],
/// "msg":"Field required","input":null}]}`.
pub fn missing_param(name: &str) -> Response {
    let body = serde_json::json!({
        "detail": [{
            "type": "missing",
            "loc": ["query", name],
            "msg": "Field required",
            "input": null,
        }]
    });
    (axum::http::StatusCode::UNPROCESSABLE_ENTITY, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_declaration_and_self_close_style() {
        let root = ok();
        assert_eq!(
            render(&Format::Xml, &root).headers()[header::CONTENT_TYPE],
            "application/xml"
        );
        let xml = xml_string(&root);
        assert_eq!(
            xml,
            "<?xml version='1.0' encoding='UTF-8'?>\n\
             <subsonic-response status=\"ok\" version=\"1.16.1\" \
             xmlns=\"http://subsonic.org/restapi\" />"
        );
    }

    #[test]
    fn xml_escapes_attribute_specials() {
        let root = ok().with(Elem::new("artist").attr("name", "A&B <\"C\">"));
        let xml = xml_string(&root);
        assert!(
            xml.contains("name=\"A&amp;B &lt;&quot;C&quot;&gt;\""),
            "{xml}"
        );
    }

    #[test]
    fn json_groups_repeated_children_and_omits_xmlns() {
        let root = ok()
            .with(Elem::new("index").attr("name", "A"))
            .with(Elem::new("index").attr("name", "B"));
        let json: Value = serde_json::from_str(&to_json(&root)).unwrap();
        assert_eq!(json["subsonic-response"]["status"], "ok");
        assert!(json["subsonic-response"].get("xmlns").is_none());
        let idx = &json["subsonic-response"]["index"];
        assert!(idx.is_array(), "{idx:?}");
        assert_eq!(idx[0]["name"], "A");
        assert_eq!(idx[1]["name"], "B");
    }

    #[test]
    fn json_single_child_is_object_not_array() {
        let root = ok().with(Elem::new("license").attr("valid", "true"));
        let json: Value = serde_json::from_str(&to_json(&root)).unwrap();
        assert_eq!(json["subsonic-response"]["license"]["valid"], "true");
    }

    #[test]
    fn error_element_shape() {
        let xml = xml_string(&error(40, "Wrong username or password"));
        assert!(
            xml.contains("<error code=\"40\" message=\"Wrong username or password\" />"),
            "{xml}"
        );
        // The root status flips to "failed" (Python `_error`, not `_ok`).
        assert!(
            xml.contains("<subsonic-response status=\"failed\""),
            "{xml}"
        );
        let json: Value = serde_json::from_str(&to_json(&error(70, "Nope"))).unwrap();
        assert_eq!(json["subsonic-response"]["status"], "failed");
        assert_eq!(json["subsonic-response"]["error"]["code"], "70");
    }

    #[test]
    fn format_selection_matches_python() {
        assert_eq!(Format::new(None, None), Format::Xml);
        assert_eq!(Format::new(Some("xml"), None), Format::Xml);
        assert_eq!(Format::new(Some("json"), None), Format::Json);
        // jsonp without callback falls back to XML (Python behavior).
        assert_eq!(Format::new(Some("jsonp"), None), Format::Xml);
        assert_eq!(
            Format::new(Some("jsonp"), Some("cb")),
            Format::Jsonp("cb".into())
        );
        assert_eq!(Format::new(Some("jsonp"), Some("")), Format::Xml);
    }

    fn xml_string(root: &Elem) -> String {
        let mut body = String::new();
        body.push_str("<?xml version='1.0' encoding='UTF-8'?>\n");
        write_xml(&mut body, root, true);
        body
    }
}
