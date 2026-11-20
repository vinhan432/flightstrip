//! A small JSON codec.
//!
//! The wire format is the product (a static site reads `session.json`), so the
//! codec is written to be boring and exact: insertion-ordered objects, 2-space
//! pretty output, compact event lines, and a parser that rejects trailing junk.
//!
//! This replaces `serde`/`serde_json` — two crates plus a proc-macro that made up
//! most of the build time for a schema this small.

use crate::{Error, Result};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    UInt(u64),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Insertion-ordered: output stays byte-stable across runs, which makes
    /// `session.json` diffable in a PR.
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(flag) => Some(*flag),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::UInt(value) => Some(*value),
            Json::Int(value) => u64::try_from(*value).ok(),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(value) => Some(*value),
            Json::UInt(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(fields) => Some(fields),
            _ => None,
        }
    }

    /// `false`/`null`/`0` mean "absent" to callers, but an explicit `null` field
    /// still has to round-trip.
    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Json::Str(value.to_string())
    }
}

impl From<&String> for Json {
    fn from(value: &String) -> Self {
        Json::Str(value.clone())
    }
}

impl From<String> for Json {
    fn from(value: String) -> Self {
        Json::Str(value)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Json::Bool(value)
    }
}

macro_rules! json_uint {
    ($($ty:ty),*) => {$(
        impl From<$ty> for Json {
            fn from(value: $ty) -> Self { Json::UInt(value as u64) }
        }
    )*};
}

json_uint!(u8, u16, u32, u64, usize);

macro_rules! json_int {
    ($($ty:ty),*) => {$(
        impl From<$ty> for Json {
            fn from(value: $ty) -> Self { Json::Int(value as i64) }
        }
    )*};
}

json_int!(i8, i16, i32, i64, isize);

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(inner) => inner.into(),
            None => Json::Null,
        }
    }
}

pub fn obj(fields: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

pub fn arr(items: impl IntoIterator<Item = Json>) -> Json {
    Json::Arr(items.into_iter().collect())
}

// ── parsing ───────────────────────────────────────────────────────────────

/// Deepest nesting the parser will follow.
///
/// The reader is recursive, so without a ceiling a hostile or accidental deeply
/// nested document walks off the stack and the process dies with no message at
/// all (`STATUS_STACK_OVERFLOW`, not a Rust panic). A session file is a few
/// levels deep; 256 is far past anything a client would legitimately send and
/// still cheap to reject.
pub const MAX_DEPTH: usize = 256;

pub fn parse(text: &str) -> Result<Json> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
        depth: 0,
    };
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(Error::Json(format!("trailing data at byte {}", parser.pos)));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            return Ok(());
        }
        Err(Error::Json(format!(
            "expected '{}' at byte {}",
            byte as char, self.pos
        )))
    }

    fn value(&mut self) -> Result<Json> {
        if self.depth >= MAX_DEPTH {
            return Err(Error::Json(format!(
                "nesting deeper than {MAX_DEPTH} levels at byte {}",
                self.pos
            )));
        }
        self.depth += 1;
        let parsed = self.value_inner();
        self.depth -= 1;
        parsed
    }

    fn value_inner(&mut self) -> Result<Json> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.number(),
            Some(byte) => Err(Error::Json(format!(
                "unexpected '{}' at byte {}",
                byte as char, self.pos
            ))),
            None => Err(Error::Json("unexpected end of input".into())),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(Error::Json(format!(
                "expected `{word}` at byte {}",
                self.pos
            )))
        }
    }

    fn object(&mut self) -> Result<Json> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.value()?;
            fields.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => {
                    return Err(Error::Json(format!(
                        "expected ',' or '}}' at byte {}",
                        self.pos
                    )))
                }
            }
        }
        Ok(Json::Obj(fields))
    }

    fn array(&mut self) -> Result<Json> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => {
                    return Err(Error::Json(format!(
                        "expected ',' or ']' at byte {}",
                        self.pos
                    )))
                }
            }
        }
        Ok(Json::Arr(items))
    }

    fn string(&mut self) -> Result<String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = self
                .bump()
                .ok_or_else(|| Error::Json("unterminated string".into()))?;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = self
                        .bump()
                        .ok_or_else(|| Error::Json("unterminated escape".into()))?;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                // Surrogate pair: a lone high surrogate is
                                // invalid JSON, so a missing partner is an error.
                                if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                    return Err(Error::Json("unpaired UTF-16 surrogate".into()));
                                }
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(Error::Json("unpaired UTF-16 surrogate".into()));
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else {
                                first
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| Error::Json("invalid \\u escape".into()))?,
                            );
                        }
                        other => {
                            return Err(Error::Json(format!(
                                "invalid escape '\\{}'",
                                other as char
                            )))
                        }
                    }
                }
                _ => {
                    // Multi-byte UTF-8 arrives one byte at a time; collect the
                    // continuation bytes before decoding the character.
                    let start = self.pos - 1;
                    let width = utf8_width(byte);
                    if width > 1 {
                        let end = start + width;
                        if end > self.bytes.len() {
                            return Err(Error::Json("truncated UTF-8 sequence".into()));
                        }
                        let text = std::str::from_utf8(&self.bytes[start..end])
                            .map_err(|_| Error::Json("invalid UTF-8".into()))?;
                        out.push_str(text);
                        self.pos = end;
                    } else {
                        out.push(byte as char);
                    }
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self
                .bump()
                .ok_or_else(|| Error::Json("truncated \\u escape".into()))?;
            let digit = (byte as char)
                .to_digit(16)
                .ok_or_else(|| Error::Json("invalid hex in \\u escape".into()))?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<Json> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
            self.pos += 1;
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            float = true;
            self.pos += 1;
            while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let raw = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| Error::Json("invalid number".into()))?;
        if float {
            raw.parse::<f64>()
                .map(Json::Float)
                .map_err(|_| Error::Json(format!("invalid number `{raw}`")))
        } else if let Some(rest) = raw.strip_prefix('-') {
            rest.parse::<i64>()
                .map(|value| Json::Int(-value))
                .map_err(|_| Error::Json(format!("invalid number `{raw}`")))
        } else {
            raw.parse::<u64>()
                .map(Json::UInt)
                .map_err(|_| Error::Json(format!("invalid number `{raw}`")))
        }
    }
}

fn utf8_width(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

// ── writing ───────────────────────────────────────────────────────────────

pub fn to_compact(value: &Json) -> String {
    let mut out = String::new();
    write_value(&mut out, value, None, 0);
    out
}

pub fn to_pretty(value: &Json) -> String {
    let mut out = String::new();
    write_value(&mut out, value, Some(2), 0);
    out
}

fn write_value(out: &mut String, value: &Json, indent: Option<usize>, depth: usize) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::UInt(number) => {
            let _ = write!(out, "{number}");
        }
        Json::Int(number) => {
            let _ = write!(out, "{number}");
        }
        Json::Float(number) => {
            if number.is_finite() {
                let _ = write!(out, "{number}");
            } else {
                // JSON has no NaN/Infinity; `null` is the honest encoding.
                out.push_str("null");
            }
        }
        Json::Str(text) => write_string(out, text),
        Json::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_value(out, item, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push(']');
        }
        Json::Obj(fields) => {
            if fields.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (index, (name, item)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_string(out, name);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_value(out, item, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push('}');
        }
    }
}

fn newline(out: &mut String, indent: Option<usize>, depth: usize) {
    if let Some(width) = indent {
        out.push('\n');
        for _ in 0..width * depth {
            out.push(' ');
        }
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_nested_document() {
        let text = r#"{"a":[1,2,{"b":null}],"c":"x\"y","d":true,"e":-3}"#;
        let value = parse(text).unwrap();
        assert_eq!(value.get("d").and_then(Json::as_bool), Some(true));
        assert_eq!(value.get("e").and_then(Json::as_i64), Some(-3));
        assert_eq!(parse(&to_compact(&value)).unwrap(), value);
    }

    #[test]
    fn pretty_output_is_two_spaces_and_stable() {
        let value = obj(vec![("a", Json::from(1u32)), ("b", arr([Json::from("x")]))]);
        let pretty = to_pretty(&value);
        assert_eq!(pretty, "{\n  \"a\": 1,\n  \"b\": [\n    \"x\"\n  ]\n}");
    }

    #[test]
    fn escapes_and_unicode_survive() {
        let value = parse(r#""line\nnext \u00e9 \ud83d\ude00""#).unwrap();
        assert_eq!(value.as_str().unwrap(), "line\nnext é 😀");
        let encoded = to_compact(&Json::from("tab\there \"quoted\""));
        assert_eq!(
            parse(&encoded).unwrap().as_str().unwrap(),
            "tab\there \"quoted\""
        );
    }

    #[test]
    fn rejects_trailing_data_and_broken_documents() {
        assert!(parse("{} junk").is_err());
        assert!(parse("{\"a\":}").is_err());
        assert!(parse("[1,2").is_err());
        assert!(parse("\"unterminated").is_err());
    }

    #[test]
    fn nesting_deeper_than_the_limit_is_an_error_not_a_crash() {
        // A recursive descent parser on 5,000 levels of `[` aborts the process
        // with STATUS_STACK_OVERFLOW before it can return an error, which the
        // `verify` and `report` fuzz checks caught. Depth has to be refused.
        let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
        let error = parse(&deep).unwrap_err();
        assert!(error.to_string().contains("nesting"), "{error}");

        // The innermost array of an all-empty nest never calls value() again,
        // so exactly MAX_DEPTH levels parse and MAX_DEPTH + 1 is refused.
        let at_limit = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(
            parse(&at_limit).is_ok(),
            "{} levels should parse",
            MAX_DEPTH
        );
        let past_limit = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert!(
            parse(&past_limit).is_err(),
            "{} levels should be refused",
            MAX_DEPTH + 1
        );

        let shallow = format!("{}{}", "[".repeat(20), "]".repeat(20));
        assert!(parse(&shallow).is_ok());
    }

    #[test]
    fn objects_keep_insertion_order() {
        let value = parse(r#"{"z":1,"a":2}"#).unwrap();
        let names: Vec<&str> = value
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["z", "a"]);
    }
}
