//! A JSON writer that reproduces Python's `json.dump(obj, f, indent=4)`
//! byte for byte (`ensure_ascii=True`, `", "` / `": "` separators, insertion
//! ordered keys), plus Python-flavoured helpers for reading loosely typed
//! values out of a `serde_json::Value` tree the way the Python code's
//! `dict.get(...)` / truthiness idioms do.
//!
//! Route files, config files and custom-gen metadata are all produced this way
//! so that a file saved by the Rust app is identical to one saved by the
//! Python app on the same platform (including the platform newline, since
//! Python opens the file in text mode).

use serde_json::Value;
use std::fmt::Write as _;

/// Newline written by Python's text-mode `open(path, 'w')` on this platform.
#[cfg(windows)]
pub const PLATFORM_NEWLINE: &str = "\r\n";
#[cfg(not(windows))]
pub const PLATFORM_NEWLINE: &str = "\n";

/// `json.dumps(value, indent=4)` (with `\n` newlines).
pub fn dumps_indent4(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, 0, 4);
    out
}

/// `json.dumps(value, indent=indent)`.
pub fn dumps_indent(value: &Value, indent: usize) -> String {
    let mut out = String::new();
    write_value(&mut out, value, 0, indent);
    out
}

/// `json.dumps(value)` (compact-ish: Python's default separators are
/// `', '` and `': '`).
pub fn dumps_compact(value: &Value) -> String {
    let mut out = String::new();
    write_value_compact(&mut out, value);
    out
}

/// The exact bytes Python writes for `json.dump(value, f, indent=4)` when `f`
/// was opened with `open(path, 'w')` on this platform.
pub fn dump_indent4_platform_bytes(value: &Value) -> Vec<u8> {
    let text = dumps_indent4(value);
    if PLATFORM_NEWLINE == "\n" {
        text.into_bytes()
    } else {
        text.replace('\n', PLATFORM_NEWLINE).into_bytes()
    }
}

fn write_indent(out: &mut String, depth: usize, indent: usize) {
    out.push('\n');
    for _ in 0..(depth * indent) {
        out.push(' ');
    }
}

fn write_value(out: &mut String, value: &Value, depth: usize, indent: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                write_indent(out, depth + 1, indent);
                write_value(out, item, depth + 1, indent);
            }
            write_indent(out, depth, indent);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (idx, (key, item)) in map.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                write_indent(out, depth + 1, indent);
                write_string(out, key);
                out.push_str(": ");
                write_value(out, item, depth + 1, indent);
            }
            write_indent(out, depth, indent);
            out.push('}');
        }
    }
}

fn write_value_compact(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(out, n),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                write_value_compact(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (idx, (key, item)) in map.iter().enumerate() {
                if idx > 0 {
                    out.push_str(", ");
                }
                write_string(out, key);
                out.push_str(": ");
                write_value_compact(out, item);
            }
            out.push('}');
        }
    }
}

fn write_number(out: &mut String, n: &serde_json::Number) {
    if let Some(i) = n.as_i64() {
        let _ = write!(out, "{}", i);
    } else if let Some(u) = n.as_u64() {
        let _ = write!(out, "{}", u);
    } else if let Some(f) = n.as_f64() {
        out.push_str(&python_float_repr(f));
    } else {
        out.push_str("null");
    }
}

/// `json.dumps` string escaping with `ensure_ascii=True`.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if (c as u32) < 0x7f || (c as u32) == 0x7f => out.push(c),
            c => {
                let cp = c as u32;
                if cp <= 0xffff {
                    let _ = write!(out, "\\u{:04x}", cp);
                } else {
                    let v = cp - 0x10000;
                    let hi = 0xd800 | (v >> 10);
                    let lo = 0xdc00 | (v & 0x3ff);
                    let _ = write!(out, "\\u{:04x}\\u{:04x}", hi, lo);
                }
            }
        }
    }
    out.push('"');
}

/// Python's `repr(float)` / `float.__repr__` (shortest round-trip form).
pub fn python_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".to_string() } else { "0.0".to_string() };
    }
    // Rust's `{:e}` gives the shortest round-trip mantissa; decide between
    // positional and exponent notation the way Python's repr does
    // (exponent < -4 or >= 16 -> scientific).
    let sci = format!("{:e}", f);
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches('-');
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let mut result = String::new();
    if negative {
        result.push('-');
    }
    if exp < -4 || exp >= 16 {
        // d.ddddde+XX
        result.push_str(&digits[..1]);
        if digits.len() > 1 {
            result.push('.');
            result.push_str(&digits[1..]);
        }
        result.push('e');
        if exp < 0 {
            result.push('-');
        } else {
            result.push('+');
        }
        let _ = write!(result, "{:02}", exp.abs());
    } else if exp < 0 {
        result.push_str("0.");
        for _ in 0..(-exp - 1) {
            result.push('0');
        }
        result.push_str(&digits);
    } else {
        let exp = exp as usize;
        if digits.len() > exp + 1 {
            result.push_str(&digits[..exp + 1]);
            result.push('.');
            result.push_str(&digits[exp + 1..]);
        } else {
            result.push_str(&digits);
            for _ in 0..(exp + 1 - digits.len()) {
                result.push('0');
            }
            result.push_str(".0");
        }
    }
    result
}

/// `f"{x:.1f}"` and friends: Rust's fixed formatting is correctly rounded with
/// ties-to-even on the exact binary value, which is what CPython does too.
pub fn python_fixed(f: f64, decimals: usize) -> String {
    format!("{:.*}", decimals, f)
}

/// Python's `round(x)` -> int (round half to even).
pub fn python_round_int(f: f64) -> i64 {
    f.round_ties_even() as i64
}

/// Python's `str(int)` for a float that is integral, or the float repr
/// otherwise (used by a few `f"{value}"` sites that may see either).
pub fn python_str_number(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                python_float_repr(f)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Null => "None".to_string(),
        other => python_repr(other),
    }
}

/// Python's `str()`/`repr()` of a loaded JSON value (dicts/lists print the
/// way Python prints them, which a couple of error strings rely on).
pub fn python_repr(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(_) => python_str_number(v),
        Value::String(s) => python_repr_str(s),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(python_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(map) => {
            let inner: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", python_repr_str(k), python_repr(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// Python's `repr(str)`: single quotes unless the string contains a single
/// quote and no double quotes.
pub fn python_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python truthiness of a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `dict.get(key)` returning `None` for a missing key. Note that a present
/// `null` returns `Some(&Value::Null)`, matching Python's behaviour of
/// returning the stored `None`.
pub fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_object().and_then(|o| o.get(key))
}

/// `dict.get(key)` collapsing both "missing" and `null` to `None`.
pub fn get_non_null<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match get(v, key) {
        Some(Value::Null) | None => None,
        other => other,
    }
}

pub fn get_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    get(v, key).and_then(|x| x.as_str())
}

pub fn get_i64(v: &Value, key: &str) -> Option<i64> {
    get(v, key).and_then(value_as_i64)
}

pub fn get_bool(v: &Value, key: &str) -> Option<bool> {
    get(v, key).and_then(|x| x.as_bool())
}

/// Python `int()`-like view of a number: integers as-is, floats truncated.
pub fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|u| u as i64))
            .or_else(|| n.as_f64().map(|f| f as i64)),
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        _ => None,
    }
}

pub fn value_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Convert a `&str` list into a JSON array of strings.
pub fn str_array<I, S>(items: I) -> Value
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    Value::Array(items.into_iter().map(|s| Value::String(s.as_ref().to_string())).collect())
}

/// Build a JSON object from `(key, value)` pairs, preserving order.
pub fn object(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = serde_json::Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

/// Parse JSON text the way `json.load` does (duplicate keys: last wins,
/// which `serde_json` with `preserve_order` also does).
pub fn loads(text: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(text)
}

/// Decode file bytes to text: UTF-8 (which is also what every ASCII data
/// file decodes to) with a cp1252 fallback for hand-edited custom gens that
/// the Python app would have read with the Windows locale codec.
pub fn decode_text(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            cow.into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn indent4_matches_python() {
        let v = json!({"a": 1, "b": [1, 2, {"c": null}], "d": {}, "e": [], "f": "x\u{2019}y\n"});
        let s = dumps_indent4(&v);
        let expected = "{\n    \"a\": 1,\n    \"b\": [\n        1,\n        2,\n        {\n            \"c\": null\n        }\n    ],\n    \"d\": {},\n    \"e\": [],\n    \"f\": \"x\\u2019y\\n\"\n}";
        assert_eq!(s, expected);
    }

    #[test]
    fn float_repr() {
        assert_eq!(python_float_repr(1.0), "1.0");
        assert_eq!(python_float_repr(0.1), "0.1");
        assert_eq!(python_float_repr(1e16), "1e+16");
        assert_eq!(python_float_repr(1.5e-7), "1.5e-07");
        assert_eq!(python_float_repr(123456.789), "123456.789");
        assert_eq!(python_float_repr(0.0001), "0.0001");
        assert_eq!(python_float_repr(0.00001), "1e-05");
        assert_eq!(python_float_repr(1234567890123456.0), "1234567890123456.0");
    }

    #[test]
    fn repr_str() {
        assert_eq!(python_repr_str("abc"), "'abc'");
        assert_eq!(python_repr_str("it's"), "\"it's\"");
        assert_eq!(python_repr_str("it's \"x\""), "'it\\'s \"x\"'");
    }
}
