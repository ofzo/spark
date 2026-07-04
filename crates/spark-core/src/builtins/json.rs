#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! JSON built-in.
//!
//! Implements the JavaScript JSON object.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// JSON.parse
// ============================================================================

/// JSON.parse(text, reviver)
pub fn json_parse(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = match args.first() {
        Some(v) => v.to_string(),
        None => return JSValue::undefined(),
    };

    let reviver = args.get(1).cloned();

    let result = parse_json_value(&text, &mut 0);
    match result {
        Some(val) => {
            if let Some(rev) = reviver {
                apply_reviver(&val, &rev)
            } else {
                val
            }
        }
        None => JSValue::undefined(),
    }
}

/// Apply a reviver function to parsed JSON
fn apply_reviver(val: &JSValue, reviver: &JSValue) -> JSValue {
    match val {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            let keys: Vec<String> = borrow.properties.keys().cloned().collect();
            let mut result = JSValue::object("Object");

            for key in &keys {
                let child_val = borrow.properties.get(key).cloned().unwrap_or(JSValue::undefined());
                let transformed = apply_reviver(&child_val, reviver);

                // Call reviver: reviver(key, value)
                if let JSValue::Function(f) = reviver {
                    let f_borrow = f.borrow();
                    if let FunctionBody::Native(native_fn) = &f_borrow.body {
                        let new_val = native_fn(
                            &JSValue::undefined(),
                            &[JSValue::string(key), transformed],
                        );
                        // If reviver returns undefined, delete the key
                        if !new_val.is_undefined() {
                            result.set_property(key, new_val);
                        }
                    } else {
                        result.set_property(key, transformed);
                    }
                } else {
                    result.set_property(key, transformed);
                }
            }
            result
        }
        _ => val.clone(),
    }
}

// ============================================================================
// JSON tokenizer / parser
// ============================================================================

/// Parse a JSON value from text starting at pos
fn parse_json_value(text: &str, pos: &mut usize) -> Option<JSValue> {
    skip_whitespace(text, pos);

    if *pos >= text.len() {
        return None;
    }

    match text.as_bytes()[*pos] {
        b'{' => parse_json_object(text, pos),
        b'[' => parse_json_array(text, pos),
        b'"' => parse_json_string(text, pos),
        b't' => {
            if text[*pos..].starts_with("true") {
                *pos += 4;
                Some(JSValue::bool(true))
            } else {
                None
            }
        }
        b'f' => {
            if text[*pos..].starts_with("false") {
                *pos += 5;
                Some(JSValue::bool(false))
            } else {
                None
            }
        }
        b'n' => {
            if text[*pos..].starts_with("null") {
                *pos += 4;
                Some(JSValue::null())
            } else {
                None
            }
        }
        b'-' | b'0'..=b'9' => parse_json_number(text, pos),
        _ => None,
    }
}

/// Skip whitespace
fn skip_whitespace(text: &str, pos: &mut usize) {
    while *pos < text.len() {
        match text.as_bytes()[*pos] {
            b' ' | b'\t' | b'\n' | b'\r' => *pos += 1,
            _ => break,
        }
    }
}

/// Parse a JSON object
fn parse_json_object(text: &str, pos: &mut usize) -> Option<JSValue> {
    if text.as_bytes()[*pos] != b'{' {
        return None;
    }
    *pos += 1;

    let obj = JSValue::object("Object");

    skip_whitespace(text, pos);

    // Empty object
    if *pos < text.len() && text.as_bytes()[*pos] == b'}' {
        *pos += 1;
        return Some(obj);
    }

    loop {
        skip_whitespace(text, pos);

        // Parse key
        let key = parse_json_string(text, pos)?;
        let key_str = key.to_string();

        skip_whitespace(text, pos);

        // Expect ':'
        if *pos >= text.len() || text.as_bytes()[*pos] != b':' {
            return None;
        }
        *pos += 1;

        // Parse value
        let value = parse_json_value(text, pos)?;
        obj.set_property(&key_str, value);

        skip_whitespace(text, pos);

        if *pos >= text.len() {
            return None;
        }

        match text.as_bytes()[*pos] {
            b',' => *pos += 1,
            b'}' => {
                *pos += 1;
                return Some(obj);
            }
            _ => return None,
        }
    }
}

/// Parse a JSON array
fn parse_json_array(text: &str, pos: &mut usize) -> Option<JSValue> {
    if text.as_bytes()[*pos] != b'[' {
        return None;
    }
    *pos += 1;

    let arr = JSValue::object("Array");
    let mut length = 0;

    skip_whitespace(text, pos);

    // Empty array
    if *pos < text.len() && text.as_bytes()[*pos] == b']' {
        *pos += 1;
        arr.set_property("length", JSValue::int(0));
        return Some(arr);
    }

    loop {
        skip_whitespace(text, pos);

        let value = parse_json_value(text, pos)?;
        arr.set_property(&length.to_string(), value);
        length += 1;

        skip_whitespace(text, pos);

        if *pos >= text.len() {
            return None;
        }

        match text.as_bytes()[*pos] {
            b',' => *pos += 1,
            b']' => {
                *pos += 1;
                arr.set_property("length", JSValue::int(length));
                return Some(arr);
            }
            _ => return None,
        }
    }
}

/// Parse a JSON string
fn parse_json_string(text: &str, pos: &mut usize) -> Option<JSValue> {
    if text.as_bytes()[*pos] != b'"' {
        return None;
    }
    *pos += 1;

    let mut result = String::new();

    loop {
        if *pos >= text.len() {
            return None;
        }

        let ch = text.as_bytes()[*pos];
        match ch {
            b'"' => {
                *pos += 1;
                return Some(JSValue::string(&result));
            }
            b'\\' => {
                *pos += 1;
                if *pos >= text.len() {
                    return None;
                }
                match text.as_bytes()[*pos] {
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    b'/' => result.push('/'),
                    b'b' => result.push('\x08'),
                    b'f' => result.push('\x0C'),
                    b'n' => result.push('\n'),
                    b'r' => result.push('\r'),
                    b't' => result.push('\t'),
                    b'u' => {
                        // Unicode escape: \uXXXX
                        *pos += 1;
                        if *pos + 4 > text.len() {
                            return None;
                        }
                        let hex: String = text[*pos..*pos + 4].to_string();
                        *pos += 3; // Will be incremented again below
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            }
                        }
                    }
                    _ => return None,
                }
                *pos += 1;
            }
            _ => {
                // Handle multi-byte UTF-8 characters
                let ch_str = &text[*pos..];
                let first_byte = ch_str.as_bytes()[0];
                let char_len = if first_byte < 0x80 {
                    1
                } else if first_byte < 0xE0 {
                    2
                } else if first_byte < 0xF0 {
                    3
                } else {
                    4
                };
                if *pos + char_len > text.len() {
                    return None;
                }
                result.push_str(&ch_str[..char_len]);
                *pos += char_len;
            }
        }
    }
}

/// Parse a JSON number
fn parse_json_number(text: &str, pos: &mut usize) -> Option<JSValue> {
    let start = *pos;

    // Optional minus
    if *pos < text.len() && text.as_bytes()[*pos] == b'-' {
        *pos += 1;
    }

    // Integer part
    if *pos < text.len() && text.as_bytes()[*pos] == b'0' {
        *pos += 1;
    } else if *pos < text.len() && text.as_bytes()[*pos].is_ascii_digit() {
        while *pos < text.len() && text.as_bytes()[*pos].is_ascii_digit() {
            *pos += 1;
        }
    } else {
        return None;
    }

    // Optional fraction
    if *pos < text.len() && text.as_bytes()[*pos] == b'.' {
        *pos += 1;
        if *pos >= text.len() || !text.as_bytes()[*pos].is_ascii_digit() {
            return None;
        }
        while *pos < text.len() && text.as_bytes()[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }

    // Optional exponent
    if *pos < text.len() && (text.as_bytes()[*pos] == b'e' || text.as_bytes()[*pos] == b'E') {
        *pos += 1;
        if *pos < text.len() && (text.as_bytes()[*pos] == b'+' || text.as_bytes()[*pos] == b'-') {
            *pos += 1;
        }
        if *pos >= text.len() || !text.as_bytes()[*pos].is_ascii_digit() {
            return None;
        }
        while *pos < text.len() && text.as_bytes()[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }

    let num_str = &text[start..*pos];
    match num_str.parse::<f64>() {
        Ok(n) => Some(JSValue::float(n)),
        Err(_) => None,
    }
}

// ============================================================================
// JSON.stringify
// ============================================================================

/// JSON.stringify(value, replacer, space)
pub fn json_stringify(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let value = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };

    let replacer = args.get(1).cloned();
    let gap = determine_gap(args.get(2));

    let mut state = StringifyState {
        replacer,
        gap,
        indent_level: 0,
    };

    let result = stringify_value(value, &mut state, &mut Vec::new());
    match result {
        Some(s) => JSValue::string(&s),
        None => JSValue::undefined(),
    }
}

struct StringifyState {
    replacer: Option<JSValue>,
    gap: String,
    indent_level: usize,
}

/// Determine the gap string from the space argument
fn determine_gap(space: Option<&JSValue>) -> String {
    match space {
        Some(JSValue::Int(n)) => {
            let n = (*n).min(10) as usize;
            " ".repeat(n)
        }
        Some(JSValue::Float(f)) => {
            let n = (*f as usize).min(10);
            " ".repeat(n)
        }
        Some(JSValue::String(s)) => {
            let s = s.borrow().data.clone();
            if s.len() > 10 {
                s[..10].to_string()
            } else {
                s
            }
        }
        _ => String::new(),
    }
}

/// Stringify a value
fn stringify_value(value: &JSValue, state: &mut StringifyState, stack: &mut Vec<JSValue>) -> Option<String> {
    match value {
        JSValue::Undefined => None,
        JSValue::Null => Some("null".to_string()),
        JSValue::Bool(b) => Some(b.to_string()),
        JSValue::Int(i) => Some(i.to_string()),
        JSValue::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                Some("null".to_string())
            } else {
                Some(format_number_for_json(*f))
            }
        }
        JSValue::String(s) => Some(quote_string(&s.borrow().data)),
        JSValue::Object(obj) => {
            let class_name = obj.borrow().class_name.clone();

            // Check for circular references
            for existing in stack.iter() {
                if let JSValue::Object(e) = existing {
                    if Rc::ptr_eq(e, obj) {
                        return None; // Circular reference: skip
                    }
                }
            }

            // Check for toJSON method
            if let Some(to_json) = obj.borrow().properties.get("toJSON") {
                if let JSValue::Function(f) = to_json {
                    let f_borrow = f.borrow();
                    if let FunctionBody::Native(native_fn) = &f_borrow.body {
                        let result = native_fn(value, &[]);
                        return stringify_value(&result, state, stack);
                    }
                }
            }

            // Array check
            if class_name == "Array" {
                let length = obj.borrow().properties.get("length")
                    .map(|v| v.to_number() as usize)
                    .unwrap_or(0);

                stack.push(value.clone());
                let mut parts = Vec::new();

                for i in 0..length {
                    let elem = obj.borrow().properties.get(&i.to_string())
                        .cloned()
                        .unwrap_or(JSValue::undefined());

                    let elem_str = if elem.is_undefined() || elem.is_null() {
                        Some("null".to_string())
                    } else {
                        stringify_value(&elem, state, stack)
                    };

                    match elem_str {
                        Some(s) => parts.push(s),
                        None => parts.push("null".to_string()),
                    }
                }

                stack.pop();

                if state.gap.is_empty() {
                    Some(format!("[{}]", parts.join(",")))
                } else {
                    let indent = " ".repeat(state.indent_level * state.gap.len());
                    let inner_indent = " ".repeat((state.indent_level + 1) * state.gap.len());
                    let formatted: Vec<String> = parts.iter().map(|p| format!("{}{}", inner_indent, p)).collect();
                    Some(format!("[\n{}\n{}]", formatted.join(",\n"), indent))
                }
            } else {
                // Regular object
                stack.push(value.clone());
                let mut key_value_pairs = Vec::new();

                let keys: Vec<String> = {
                    let borrow = obj.borrow();
                    let mut keys: Vec<String> = borrow.properties.keys().cloned().collect();
                    keys.sort();
                    keys
                };

                for key in &keys {
                    let val = obj.borrow().properties.get(key).cloned().unwrap_or(JSValue::undefined());

                    // Apply replacer
                    let val = apply_replacer(&JSValue::string(key), &val, &state.replacer);

                    if val.is_undefined() || val.is_null() {
                        key_value_pairs.push(format!("{}:null", quote_string(key)));
                    } else {
                        let val_str = stringify_value(&val, state, stack);
                        if let Some(s) = val_str {
                            key_value_pairs.push(format!("{}:{}", quote_string(key), s));
                        }
                    }
                }

                stack.pop();

                if key_value_pairs.is_empty() {
                    Some("{}".to_string())
                } else if state.gap.is_empty() {
                    Some(format!("{{{}}}", key_value_pairs.join(",")))
                } else {
                    let indent = " ".repeat(state.indent_level * state.gap.len());
                    let inner_indent = " ".repeat((state.indent_level + 1) * state.gap.len());
                    let formatted: Vec<String> = key_value_pairs.iter()
                        .map(|p| format!("{}{}", inner_indent, p))
                        .collect();
                    Some(format!("{{\n{}\n{}}}", formatted.join(",\n"), indent))
                }
            }
        }
        JSValue::Function(_) => None, // Functions are not serialized
        JSValue::Symbol(_) => None,    // Symbols are not serialized
        JSValue::BigInt(_) => None,     // BigInts are not serialized
        JSValue::Int(_) => Some(value.to_number().to_string()),
    }
}

/// Apply replacer function
fn apply_replacer(key: &JSValue, value: &JSValue, replacer: &Option<JSValue>) -> JSValue {
    match replacer {
        Some(JSValue::Function(f)) => {
            let f_borrow = f.borrow();
            if let FunctionBody::Native(native_fn) = &f_borrow.body {
                native_fn(&JSValue::undefined(), &[key.clone(), value.clone()])
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

/// Quote a string for JSON output
fn quote_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            c if c < '\x20' => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

/// Format a number for JSON output
fn format_number_for_json(n: f64) -> String {
    if n == 0.0 {
        "0".to_string()
    } else {
        let s = format!("{}", n);
        // Clean up Rust's formatting to match JS
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the JSON object.
pub fn init_json(ctx: &mut JSContext) {
    let json = JSValue::object("JSON");

    json.set_property("parse", JSValue::function(
        Some("parse"),
        vec!["text".to_string(), "reviver".to_string()],
        FunctionBody::Native(json_parse),
    ));

    json.set_property("stringify", JSValue::function(
        Some("stringify"),
        vec!["value".to_string(), "replacer".to_string(), "space".to_string()],
        FunctionBody::Native(json_stringify),
    ));

    ctx.global
        .borrow_mut()
        .properties
        .insert("JSON".to_string(), json);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_json_parse_string() {
        let this = JSValue::undefined();
        let result = json_parse(&this, &[JSValue::string(r#""hello""#)]);
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_json_parse_number() {
        let this = JSValue::undefined();
        let result = json_parse(&this, &[JSValue::string("42")]);
        assert_eq!(result.to_number(), 42.0);

        let result = json_parse(&this, &[JSValue::string("-3.14")]);
        assert!((result.to_number() - (-3.14)).abs() < 1e-10);
    }

    #[test]
    fn test_json_parse_bool_null() {
        let this = JSValue::undefined();
        assert!(json_parse(&this, &[JSValue::string("true")]).to_boolean());
        assert!(!json_parse(&this, &[JSValue::string("false")]).to_boolean());
        assert!(json_parse(&this, &[JSValue::string("null")]).is_null());
    }

    #[test]
    fn test_json_parse_object() {
        let this = JSValue::undefined();
        let result = json_parse(&this, &[JSValue::string(r#"{"a": 1, "b": "hello"}"#)]);
        assert_eq!(result.get_property("a").unwrap().to_int32(), 1);
        assert_eq!(result.get_property("b").unwrap().to_string(), "hello");
    }

    #[test]
    fn test_json_parse_array() {
        let this = JSValue::undefined();
        let result = json_parse(&this, &[JSValue::string("[1, 2, 3]")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
        assert_eq!(result.get_property("0").unwrap().to_int32(), 1);
        assert_eq!(result.get_property("1").unwrap().to_int32(), 2);
        assert_eq!(result.get_property("2").unwrap().to_int32(), 3);
    }

    #[test]
    fn test_json_parse_nested() {
        let this = JSValue::undefined();
        let result = json_parse(&this, &[JSValue::string(r#"{"arr": [1, {"x": true}]}"#)]);
        let arr = result.get_property("arr").unwrap();
        assert_eq!(arr.get_property("0").unwrap().to_int32(), 1);
        let inner = arr.get_property("1").unwrap();
        assert!(inner.get_property("x").unwrap().to_boolean());
    }

    #[test]
    fn test_json_stringify_primitives() {
        let this = JSValue::undefined();

        let result = json_stringify(&this, &[JSValue::int(42)]);
        assert_eq!(result.to_string(), "42");

        let result = json_stringify(&this, &[JSValue::string("hello")]);
        assert_eq!(result.to_string(), r#""hello""#);

        let result = json_stringify(&this, &[JSValue::bool(true)]);
        assert_eq!(result.to_string(), "true");

        let result = json_stringify(&this, &[JSValue::null()]);
        assert_eq!(result.to_string(), "null");
    }

    #[test]
    fn test_json_stringify_object() {
        let this = JSValue::undefined();
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::string("hello"));

        let result = json_stringify(&this, &[obj]);
        let s = result.to_string();
        assert!(s.contains(r#""a":1"#));
        assert!(s.contains(r#""b":"hello""#));
    }

    #[test]
    fn test_json_stringify_array() {
        let this = JSValue::undefined();
        let arr = JSValue::object("Array");
        arr.set_property("0", JSValue::int(1));
        arr.set_property("1", JSValue::int(2));
        arr.set_property("2", JSValue::int(3));
        arr.set_property("length", JSValue::int(3));

        let result = json_stringify(&this, &[arr]);
        assert_eq!(result.to_string(), "[1,2,3]");
    }

    #[test]
    fn test_json_stringify_undefined_function() {
        let this = JSValue::undefined();
        let result = json_stringify(&this, &[JSValue::undefined()]);
        assert!(result.is_undefined());

        let func = JSValue::function(Some("test"), vec![], FunctionBody::Native(|_this, _args| JSValue::undefined()));
        let result = json_stringify(&this, &[func]);
        assert!(result.is_undefined());
    }

    #[test]
    fn test_json_stringify_with_space() {
        let this = JSValue::undefined();
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));

        let result = json_stringify(&this, &[obj, JSValue::undefined(), JSValue::int(2)]);
        let s = result.to_string();
        assert!(s.contains('\n'));
    }

    #[test]
    fn test_json_roundtrip() {
        let this = JSValue::undefined();
        let original = r#"{"name":"test","value":42,"arr":[1,2,3],"nested":{"x":true}}"#;
        let parsed = json_parse(&this, &[JSValue::string(original)]);
        let stringified = json_stringify(&this, &[parsed]);
        let reparsed = json_parse(&this, &[stringified]);

        assert_eq!(reparsed.get_property("name").unwrap().to_string(), "test");
        assert_eq!(reparsed.get_property("value").unwrap().to_number(), 42.0);
    }

    #[test]
    fn test_json_stringify_circular() {
        let this = JSValue::undefined();
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));

        // For a truly circular reference in our model, we'd need to set a property to the object itself
        // which creates a reference cycle. Since our objects use Rc, this is possible.
        obj.set_property("self", obj.clone());

        // This should handle the circular reference without infinite recursion
        let result = json_stringify(&this, &[obj]);
        // The result might be {"x":1} or similar - the circular ref is skipped
        assert!(!result.is_undefined());
    }

    #[test]
    fn test_init_json() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        init_json(&mut ctx);
        let json = ctx.global.borrow().properties.get("JSON").cloned();
        assert!(json.is_some());
        let json = json.unwrap();
        assert!(json.get_property("parse").is_some());
        assert!(json.get_property("stringify").is_some());
    }
}
