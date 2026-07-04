#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! String built-in.
//!
//! Implements the JavaScript String constructor and its methods.

use crate::builtins::regexp::regexp_exec;
use crate::context::JSContext;
use crate::value::{FunctionBody, JSObject, JSValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================================
// Helper: extract the string data from `this`
// ============================================================================

fn get_this_string(this: &JSValue) -> String {
    match this {
        JSValue::String(s) => s.borrow().data.clone(),
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            if let Some(val) = borrow.internal_slots.get("PrimitiveValue") {
                val.to_string()
            } else {
                this.to_string()
            }
        }
        _ => this.to_string(),
    }
}

/// Check if a JSValue is a RegExp object by class name.
fn is_regexp_object(val: &JSValue) -> bool {
    match val {
        JSValue::Object(obj) => obj.borrow().class_name == "RegExp",
        _ => false,
    }
}

/// Get the `global` flag from a RegExp object's internal slots.
fn regexp_is_global(obj: &JSValue) -> bool {
    match obj {
        JSValue::Object(o) => o
            .borrow()
            .internal_slots
            .get("global")
            .and_then(|v| match v {
                JSValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(false),
        _ => false,
    }
}

// ============================================================================
// String constructor
// ============================================================================

/// String constructor - `new String(value)` or `String(value)`
pub fn string_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    JSValue::string(&val)
}

// ============================================================================
// String static methods
// ============================================================================

/// String.fromCharCode(charCode, ...)
pub fn string_from_char_code(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let mut s = String::new();
    for arg in args {
        let code = arg.to_uint32() as u16;
        if let Some(ch) = char::from_u32(code as u32) {
            s.push(ch);
        }
    }
    JSValue::string(&s)
}

/// String.fromCodePoint(codePoint, ...)
pub fn string_from_code_point(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let mut s = String::new();
    for arg in args {
        let code = arg.to_number() as u32;
        if let Some(ch) = char::from_u32(code) {
            s.push(ch);
        }
    }
    JSValue::string(&s)
}

/// String.raw(template, ...substitutions)
///
/// Creates a string from a tagged template literal's raw string values,
/// without processing escape sequences.
pub fn string_raw(_this: &JSValue, args: &[JSValue]) -> JSValue {
    // First argument is the template object with a `raw` property (array-like)
    let template = match args.get(0) {
        Some(v) => v.clone(),
        _ => return JSValue::string(""),
    };

    let raw_strings = template.get_property("raw").unwrap_or(JSValue::undefined());

    let raw_len = raw_strings
        .get_property("length")
        .map(|v| v.to_int32() as usize)
        .unwrap_or(0);

    let mut result = String::new();

    for i in 0..raw_len {
        // Append the raw string (unprocessed, preserving backslash escapes)
        if let Some(raw_str) = raw_strings.get_property(&i.to_string()) {
            result.push_str(&raw_str.to_string());
        }

        // Append the corresponding substitution value (if provided)
        if let Some(sub) = args.get(i + 1) {
            result.push_str(&sub.to_string());
        }
    }

    JSValue::string(&result)
}

// ============================================================================
// String prototype methods
// ============================================================================

/// String.prototype.charAt(index)
pub fn string_char_at(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let pos = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0);

    if pos < 0 || pos as usize >= len {
        return JSValue::string("");
    }

    JSValue::string(&chars[pos as usize].to_string())
}

/// String.prototype.charCodeAt(index)
pub fn string_char_code_at(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let pos = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0);

    if pos < 0 || pos as usize >= len {
        return JSValue::float(f64::NAN);
    }

    JSValue::float(chars[pos as usize] as u32 as f64)
}

/// String.prototype.codePointAt(index)
pub fn string_code_point_at(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let pos = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0);

    if pos < 0 || pos as usize >= len {
        return JSValue::float(f64::NAN);
    }

    JSValue::float(chars[pos as usize] as u32 as f64)
}

/// String.prototype.concat(string2, ..., stringN)
pub fn string_concat(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let mut result = s;
    for arg in args {
        result.push_str(&arg.to_string());
    }
    JSValue::string(&result)
}

/// String.prototype.indexOf(searchString, position)
///
/// Returns the index of the first occurrence of searchString, or -1 if not found.
/// The optional position argument specifies the index to start searching from.
pub fn string_index_of(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let search_chars: Vec<char> = search.chars().collect();

    let start = match args.get(1) {
        None => 0,
        Some(v) => {
            let n = v.to_number();
            if n.is_nan() || n < 0.0 {
                0
            } else {
                (n as usize).min(len)
            }
        }
    };

    if search_chars.is_empty() {
        return JSValue::int(start as i32);
    }

    if search_chars.len() > len {
        return JSValue::int(-1);
    }

    for i in start..=len.saturating_sub(search_chars.len()) {
        if chars[i..].starts_with(&search_chars[..]) {
            return JSValue::int(i as i32);
        }
    }

    JSValue::int(-1)
}

/// String.prototype.lastIndexOf(searchString, position)
///
/// Returns the index of the last occurrence of searchString, or -1 if not found.
/// The optional position argument specifies the farthest index to search from.
pub fn string_last_index_of(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let search_chars: Vec<char> = search.chars().collect();

    // position is undefined or NaN => search entire string
    let pos = match args.get(1) {
        None => len,
        Some(v) => {
            let n = v.to_number();
            if n.is_nan() {
                len
            } else if n < 0.0 {
                0
            } else {
                (n as usize).min(len)
            }
        }
    };

    if search_chars.is_empty() {
        return JSValue::int(pos.min(len) as i32);
    }

    if search_chars.len() > len {
        return JSValue::int(-1);
    }

    let max_start = pos.min(len - search_chars.len());
    for i in (0..=max_start).rev() {
        if chars[i..].starts_with(&search_chars[..]) {
            return JSValue::int(i as i32);
        }
    }

    JSValue::int(-1)
}

/// String.prototype.includes(searchString, position)
///
/// Returns true if searchString is found within the string.
pub fn string_includes(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let search_chars: Vec<char> = search.chars().collect();

    let start = match args.get(1) {
        None => 0,
        Some(v) => {
            let n = v.to_number();
            if n.is_nan() || n < 0.0 {
                0
            } else {
                (n as usize).min(len)
            }
        }
    };

    if search_chars.is_empty() {
        return JSValue::bool(true);
    }

    if search_chars.len() > len.saturating_sub(start) {
        return JSValue::bool(false);
    }

    for i in start..=len.saturating_sub(search_chars.len()) {
        if chars[i..].starts_with(&search_chars[..]) {
            return JSValue::bool(true);
        }
    }

    JSValue::bool(false)
}

/// String.prototype.startsWith(searchString, position)
///
/// Returns true if the string starts with searchString.
pub fn string_starts_with(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let search_chars: Vec<char> = search.chars().collect();

    let start = match args.get(1) {
        None => 0,
        Some(v) => {
            let n = v.to_number();
            if n.is_nan() || n < 0.0 {
                0
            } else {
                (n as usize).min(len)
            }
        }
    };

    if search_chars.is_empty() {
        return JSValue::bool(true);
    }

    if search_chars.len() > len.saturating_sub(start) {
        return JSValue::bool(false);
    }

    JSValue::bool(chars[start..].starts_with(&search_chars[..]))
}

/// String.prototype.endsWith(searchString, endPosition)
///
/// Returns true if the string ends with searchString.
pub fn string_ends_with(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let search_chars: Vec<char> = search.chars().collect();

    let end = match args.get(1) {
        None => len,
        Some(v) => {
            let n = v.to_number();
            if n.is_nan() || n < 0.0 {
                0
            } else {
                (n as usize).min(len)
            }
        }
    };

    if search_chars.is_empty() {
        return JSValue::bool(true);
    }

    if search_chars.len() > end {
        return JSValue::bool(false);
    }

    JSValue::bool(chars[..end].ends_with(&search_chars[..]))
}

/// String.prototype.slice(start, end)
///
/// Extracts a section of the string. Negative indices count from the end.
pub fn string_slice(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let len = s.chars().count();
    let start = normalize_index(
        args.get(0)
            .map(|v| v.to_number() as isize)
            .unwrap_or(0),
        len,
    );
    let end = normalize_index(
        args.get(1)
            .map(|v| v.to_number() as isize)
            .unwrap_or(len as isize),
        len,
    );

    if start >= end {
        return JSValue::string("");
    }

    let result: String = s.chars().skip(start).take(end - start).collect();
    JSValue::string(&result)
}

/// String.prototype.substring(start, end)
///
/// Extracts a section of the string. Negative indices are treated as 0.
/// If start > end, they are swapped.
pub fn string_substring(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let len = s.chars().count();

    let mut start = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0)
        .max(0) as usize;
    let mut end = args
        .get(1)
        .map(|v| v.to_number() as isize)
        .unwrap_or(len as isize)
        .max(0) as usize;

    start = start.min(len);
    end = end.min(len);

    if start > end {
        std::mem::swap(&mut start, &mut end);
    }

    let result: String = s.chars().skip(start).take(end - start).collect();
    JSValue::string(&result)
}

/// String.prototype.substr(start, length)
///
/// Extracts a section of the string starting at start and spanning length characters.
/// Negative start counts from the end of the string.
pub fn string_substr(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as isize;

    let start_arg = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0);
    let length_arg = args
        .get(1)
        .map(|v| v.to_number() as isize)
        .unwrap_or(len);

    // Normalize start: if negative, count from end
    let start = if start_arg < 0 {
        (len + start_arg).max(0) as usize
    } else {
        start_arg.min(len) as usize
    };

    // Clamp length to available characters
    let length = length_arg.max(0).min(len - start as isize) as usize;

    let result: String = chars[start..start + length].iter().collect();
    JSValue::string(&result)
}

/// String.prototype.split(separator, limit)
///
/// Splits the string into an array of substrings.
pub fn string_split(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let limit = args
        .get(1)
        .map(|v| {
            let n = v.to_number();
            if n.is_nan() || n < 0.0 {
                0
            } else {
                n as usize
            }
        })
        .unwrap_or(usize::MAX);

    match args.get(0) {
        Some(JSValue::Undefined) | None => {
            // No separator: return array with whole string
            let arr = create_array_with_limit(1.min(limit));
            set_array_element(&arr, 0, JSValue::string(&s));
            arr
        }
        Some(sep) => {
            let sep_str = sep.to_string();
            if sep_str.is_empty() {
                // Split into individual characters
                let chars: Vec<char> = s.chars().collect();
                let count = chars.len().min(limit);
                let arr = create_array_with_limit(count);
                for (i, ch) in chars.iter().take(count).enumerate() {
                    set_array_element(&arr, i, JSValue::string(&ch.to_string()));
                }
                return arr;
            }
            let parts: Vec<&str> = s.split(&sep_str[..]).collect();
            let count = parts.len().min(limit);
            let arr = create_array_with_limit(count);
            for (i, part) in parts.iter().take(count).enumerate() {
                set_array_element(&arr, i, JSValue::string(part));
            }
            arr
        }
    }
}

/// String.prototype.trim()
///
/// Trims whitespace from both ends of the string.
pub fn string_trim(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    JSValue::string(s.trim())
}

/// String.prototype.trimStart()
///
/// Trims whitespace from the start of the string.
pub fn string_trim_start(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    JSValue::string(s.trim_start())
}

/// String.prototype.trimEnd()
///
/// Trims whitespace from the end of the string.
pub fn string_trim_end(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    JSValue::string(s.trim_end())
}

/// String.prototype.toUpperCase()
///
/// Converts the string to uppercase.
pub fn string_to_upper_case(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    JSValue::string(&s.to_uppercase())
}

/// String.prototype.toLowerCase()
///
/// Converts the string to lowercase.
pub fn string_to_lower_case(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    JSValue::string(&s.to_lowercase())
}

/// String.prototype.toLocaleUpperCase()
///
/// Converts the string to uppercase using locale-specific rules.
/// Falls back to toUpperCase().
pub fn string_to_locale_upper_case(this: &JSValue, _args: &[JSValue]) -> JSValue {
    string_to_upper_case(this, _args)
}

/// String.prototype.toLocaleLowerCase()
///
/// Converts the string to lowercase using locale-specific rules.
/// Falls back to toLowerCase().
pub fn string_to_locale_lower_case(this: &JSValue, _args: &[JSValue]) -> JSValue {
    string_to_lower_case(this, _args)
}

/// String.prototype.replace(searchValue, replaceValue)
///
/// Replaces the first occurrence of searchValue with replaceValue.
pub fn string_replace(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let search = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let replacement = args.get(1).map(|v| v.to_string()).unwrap_or_default();

    if search.is_empty() {
        // Replace empty string at position 0: insert replacement before the string
        return JSValue::string(&format!("{}{}", replacement, s));
    }

    // Use char-based search to find the first occurrence
    let chars: Vec<char> = s.chars().collect();
    let search_chars: Vec<char> = search.chars().collect();

    if search_chars.len() > chars.len() {
        return JSValue::string(&s);
    }

    for i in 0..=chars.len() - search_chars.len() {
        if chars[i..].starts_with(&search_chars[..]) {
            let before: String = chars[..i].iter().collect();
            let after: String = chars[i + search_chars.len()..].iter().collect();
            return JSValue::string(&format!("{}{}{}", before, replacement, after));
        }
    }

    JSValue::string(&s)
}

/// String.prototype.replaceAll(searchValue, replaceValue)
///
/// Replaces all occurrences of searchValue with replaceValue.
pub fn string_replace_all(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let search = args
        .get(0)
        .map(|v| v.to_string())
        .unwrap_or_default();
    let replacement = args
        .get(1)
        .map(|v| v.to_string())
        .unwrap_or_default();

    if search.is_empty() {
        // Replace empty string between each character and at the ends
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::new();
        for ch in &chars {
            result.push_str(&replacement);
            result.push(*ch);
        }
        result.push_str(&replacement);
        return JSValue::string(&result);
    }

    // Rust's String::replace handles multi-byte UTF-8 correctly
    let result = s.replace(&search[..], &replacement[..]);
    JSValue::string(&result)
}

/// String.prototype.match(regexp)
///
/// Matches the string against a regular expression.
/// For a global regex, returns an array of all match strings.
/// For a non-global regex, returns the first match as a single-element array.
pub fn string_match(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);

    match args.get(0) {
        Some(JSValue::Object(obj)) if is_regexp_object(args.get(0).unwrap()) => {
            let global = regexp_is_global(args.get(0).unwrap());

            if global {
                // Reset lastIndex before starting
                obj.borrow_mut()
                    .internal_slots
                    .insert("lastIndex".to_string(), JSValue::Float(0.0));

                let mut matches = Vec::new();
                let re = args.get(0).unwrap();

                loop {
                    let exec_result = regexp_exec(re, &[JSValue::string(&s)]);
                    if exec_result.is_null() {
                        break;
                    }
                    if exec_result.is_object() {
                        let matched = exec_result
                            .get_property("0")
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        matches.push(matched);
                    } else {
                        break;
                    }
                }

                let result_arr = create_array_with_limit(matches.len());
                for (i, m) in matches.iter().enumerate() {
                    set_array_element(&result_arr, i, JSValue::string(m));
                }
                result_arr
            } else {
                // Non-global: single match
                let re = args.get(0).unwrap();
                let exec_result = regexp_exec(re, &[JSValue::string(&s)]);
                if exec_result.is_null() {
                    return JSValue::null();
                }
                if exec_result.is_object() {
                    let matched = exec_result
                        .get_property("0")
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let result_arr = create_array_with_limit(1);
                    set_array_element(&result_arr, 0, JSValue::string(&matched));
                    // Preserve index property
                    if let Some(idx) = exec_result.get_property("index") {
                        result_arr.set_property("index", idx);
                    }
                    result_arr
                } else {
                    JSValue::null()
                }
            }
        }
        _ => {
            // String pattern: literal match
            let pattern = args
                .get(0)
                .map(|v| v.to_string())
                .unwrap_or_default();

            if pattern.is_empty() {
                let arr = create_array_with_limit(1);
                set_array_element(&arr, 0, JSValue::string(""));
                return arr;
            }

            let chars: Vec<char> = s.chars().collect();
            let search_chars: Vec<char> = pattern.chars().collect();

            if search_chars.len() <= chars.len() {
                for i in 0..=chars.len() - search_chars.len() {
                    if chars[i..].starts_with(&search_chars[..]) {
                        let arr = create_array_with_limit(1);
                        set_array_element(&arr, 0, JSValue::string(&pattern));
                        return arr;
                    }
                }
            }

            JSValue::null()
        }
    }
}

/// String.prototype.matchAll(regexp)
///
/// Returns an iterator (represented as an array) of all match results.
pub fn string_match_all(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);

    match args.get(0) {
        Some(JSValue::Object(obj)) if is_regexp_object(args.get(0).unwrap()) => {
            // Reset lastIndex
            obj.borrow_mut()
                .internal_slots
                .insert("lastIndex".to_string(), JSValue::Float(0.0));

            let mut results: Vec<JSValue> = Vec::new();
            let re = args.get(0).unwrap();

            loop {
                let exec_result = regexp_exec(re, &[JSValue::string(&s)]);
                if exec_result.is_null() {
                    break;
                }
                if exec_result.is_object() {
                    results.push(exec_result);
                } else {
                    break;
                }
            }

            let result_arr = create_array_with_limit(results.len());
            for (i, match_val) in results.into_iter().enumerate() {
                // Each match is returned as a single-element array with index
                let matched = match_val
                    .get_property("0")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let match_arr = create_array_with_limit(1);
                set_array_element(&match_arr, 0, JSValue::string(&matched));

                if let Some(idx) = match_val.get_property("index") {
                    match_arr.set_property("index", idx);
                }
                match_arr.set_property("input", JSValue::string(&s));

                set_array_element(&result_arr, i, match_arr);
            }
            result_arr
        }
        _ => {
            // String pattern: literal matching (all occurrences)
            let pattern = args
                .get(0)
                .map(|v| v.to_string())
                .unwrap_or_default();

            if pattern.is_empty() {
                let result_arr = create_array_with_limit(0);
                return result_arr;
            }

            let chars: Vec<char> = s.chars().collect();
            let search_chars: Vec<char> = pattern.chars().collect();
            let mut results = Vec::new();

            if search_chars.len() <= chars.len() {
                let mut pos = 0;
                while pos <= chars.len() - search_chars.len() {
                    if chars[pos..].starts_with(&search_chars[..]) {
                        let match_arr = create_array_with_limit(1);
                        set_array_element(&match_arr, 0, JSValue::string(&pattern));
                        match_arr.set_property("index", JSValue::Float(pos as f64));
                        match_arr.set_property("input", JSValue::string(&s));
                        results.push(match_arr);
                        pos += search_chars.len();
                        if search_chars.is_empty() {
                            pos += 1;
                        }
                    } else {
                        pos += 1;
                    }
                }
            }

            let result_arr = create_array_with_limit(results.len());
            for (i, item) in results.into_iter().enumerate() {
                set_array_element(&result_arr, i, item);
            }
            result_arr
        }
    }
}

/// String.prototype.search(regexp)
///
/// Searches the string for a match against a regular expression.
/// Returns the index of the first match, or -1 if not found.
pub fn string_search(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);

    match args.get(0) {
        Some(JSValue::Object(obj)) if is_regexp_object(args.get(0).unwrap()) => {
            // Reset lastIndex for search
            obj.borrow_mut()
                .internal_slots
                .insert("lastIndex".to_string(), JSValue::Float(0.0));

            let re = args.get(0).unwrap();
            let exec_result = regexp_exec(re, &[JSValue::string(&s)]);
            if exec_result.is_object() {
                exec_result
                    .get_property("index")
                    .map(|idx| JSValue::float(idx.to_number()))
                    .unwrap_or(JSValue::float(0.0))
            } else {
                JSValue::float(-1.0)
            }
        }
        _ => {
            // String pattern: use indexOf
            let pattern = args
                .get(0)
                .map(|v| v.to_string())
                .unwrap_or_default();
            let result = string_index_of(this, &[JSValue::string(&pattern)]);
            match result {
                JSValue::Int(-1) => JSValue::float(-1.0),
                other => JSValue::float(other.to_number()),
            }
        }
    }
}

/// String.prototype.padStart(maxLength, fillString)
///
/// Pads the current string from the start with another string
/// until it reaches the given length.
pub fn string_pad_start(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let char_count = s.chars().count();
    let max_len = args.get(0).map(|v| v.to_number() as usize).unwrap_or(0);
    let fill = args
        .get(1)
        .map(|v| v.to_string())
        .unwrap_or_else(|| " ".to_string());

    if fill.is_empty() || char_count >= max_len {
        return JSValue::string(&s);
    }

    let padding_needed = max_len - char_count;
    let fill_chars: Vec<char> = fill.chars().collect();
    let mut padding = String::new();
    for i in 0..padding_needed {
        padding.push(fill_chars[i % fill_chars.len()]);
    }

    JSValue::string(&format!("{}{}", padding, s))
}

/// String.prototype.padEnd(maxLength, fillString)
///
/// Pads the current string from the end with another string
/// until it reaches the given length.
pub fn string_pad_end(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let char_count = s.chars().count();
    let max_len = args.get(0).map(|v| v.to_number() as usize).unwrap_or(0);
    let fill = args
        .get(1)
        .map(|v| v.to_string())
        .unwrap_or_else(|| " ".to_string());

    if fill.is_empty() || char_count >= max_len {
        return JSValue::string(&s);
    }

    let padding_needed = max_len - char_count;
    let fill_chars: Vec<char> = fill.chars().collect();
    let mut padding = String::new();
    for i in 0..padding_needed {
        padding.push(fill_chars[i % fill_chars.len()]);
    }

    JSValue::string(&format!("{}{}", s, padding))
}

/// String.prototype.repeat(count)
///
/// Returns a string consisting of the specified number of copies of the string.
pub fn string_repeat(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let count = args.get(0).map(|v| v.to_int32()).unwrap_or(0);

    if count < 0 {
        return JSValue::undefined(); // Should throw RangeError in full impl
    }
    if count == 0 || s.is_empty() {
        return JSValue::string("");
    }

    let result = s.repeat(count as usize);
    JSValue::string(&result)
}

/// String.prototype.at(index)
///
/// Returns the character at the given index. Supports negative indexing.
pub fn string_at(this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as isize;
    let idx = args
        .get(0)
        .map(|v| v.to_number() as isize)
        .unwrap_or(0);

    let pos = if idx < 0 {
        let p = len + idx;
        if p < 0 {
            return JSValue::undefined();
        }
        p as usize
    } else {
        idx as usize
    };

    if pos >= chars.len() {
        return JSValue::undefined();
    }

    JSValue::string(&chars[pos].to_string())
}

/// String.prototype.normalize(form)
///
/// Returns the Unicode normalization form of the string.
/// Defaults to NFC normalization. This is a simplified implementation.
pub fn string_normalize(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let s = get_this_string(this);
    // Default to NFC normalization (simplified: return as-is)
    JSValue::string(&s)
}

/// String.prototype.valueOf()
///
/// Returns the primitive string value.
pub fn string_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::string(&get_this_string(this))
}

/// String.prototype.toString()
///
/// Returns the string representation.
pub fn string_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::string(&get_this_string(this))
}

// ============================================================================
// Helpers
// ============================================================================

/// Normalize a string index (handle negative values, wrap around)
fn normalize_index(idx: isize, len: usize) -> usize {
    let len = len as isize;
    if idx < 0 {
        (len + idx).max(0) as usize
    } else {
        idx.min(len) as usize
    }
}

/// Create an array object with a "length" property
fn create_array_with_limit(length: usize) -> JSValue {
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(length as i32));
    arr
}

/// Set an element in an array-like object
fn set_array_element(arr: &JSValue, index: usize, value: JSValue) {
    arr.set_property(&index.to_string(), value);
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the String constructor and prototype.
pub fn init_string(ctx: &mut JSContext) {
    // Create the String constructor function
    let constructor = JSValue::function(
        Some("String"),
        vec!["value".to_string()],
        FunctionBody::Native(string_constructor),
    );

    // Create String.prototype
    let prototype = JSValue::object("String");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("charAt", string_char_at),
        ("charCodeAt", string_char_code_at),
        ("codePointAt", string_code_point_at),
        ("concat", string_concat),
        ("indexOf", string_index_of),
        ("lastIndexOf", string_last_index_of),
        ("includes", string_includes),
        ("startsWith", string_starts_with),
        ("endsWith", string_ends_with),
        ("slice", string_slice),
        ("substring", string_substring),
        ("substr", string_substr),
        ("split", string_split),
        ("trim", string_trim),
        ("trimStart", string_trim_start),
        ("trimEnd", string_trim_end),
        ("toUpperCase", string_to_upper_case),
        ("toLowerCase", string_to_lower_case),
        ("toLocaleUpperCase", string_to_locale_upper_case),
        ("toLocaleLowerCase", string_to_locale_lower_case),
        ("replace", string_replace),
        ("replaceAll", string_replace_all),
        ("match", string_match),
        ("matchAll", string_match_all),
        ("search", string_search),
        ("padStart", string_pad_start),
        ("padEnd", string_pad_end),
        ("repeat", string_repeat),
        ("at", string_at),
        ("normalize", string_normalize),
        ("valueOf", string_value_of),
        ("toString", string_to_string),
    ];

    for &(name, func) in methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // Set String.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Set length property on constructor
    constructor.set_property("length", JSValue::int(1));

    // Add static methods
    constructor.set_property(
        "fromCharCode",
        JSValue::function(
            Some("fromCharCode"),
            vec![],
            FunctionBody::Native(string_from_char_code),
        ),
    );
    constructor.set_property(
        "fromCodePoint",
        JSValue::function(
            Some("fromCodePoint"),
            vec![],
            FunctionBody::Native(string_from_code_point),
        ),
    );
    constructor.set_property(
        "raw",
        JSValue::function(
            Some("raw"),
            vec![],
            FunctionBody::Native(string_raw),
        ),
    );

    // Set String on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("String".to_string(), constructor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::JSRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    // -----------------------------------------------------------------------
    // charAt
    // -----------------------------------------------------------------------

    #[test]
    fn test_char_at() {
        let this = JSValue::string("hello");
        assert_eq!(string_char_at(&this, &[JSValue::int(1)]).to_string(), "e");
        assert_eq!(string_char_at(&this, &[JSValue::int(0)]).to_string(), "h");
        assert_eq!(string_char_at(&this, &[JSValue::int(4)]).to_string(), "o");
        assert_eq!(string_char_at(&this, &[JSValue::int(10)]).to_string(), "");
        assert_eq!(string_char_at(&this, &[JSValue::int(-1)]).to_string(), "");
    }

    #[test]
    fn test_char_at_unicode() {
        let this = JSValue::string("a\u{1f600}b");
        assert_eq!(
            string_char_at(&this, &[JSValue::int(0)]).to_string(),
            "a"
        );
        assert_eq!(
            string_char_at(&this, &[JSValue::int(1)]).to_string(),
            "\u{1f600}"
        );
        assert_eq!(
            string_char_at(&this, &[JSValue::int(2)]).to_string(),
            "b"
        );
        assert_eq!(
            string_char_at(&this, &[JSValue::int(3)]).to_string(),
            ""
        );
    }

    #[test]
    fn test_char_at_empty() {
        let this = JSValue::string("");
        assert_eq!(string_char_at(&this, &[JSValue::int(0)]).to_string(), "");
    }

    // -----------------------------------------------------------------------
    // charCodeAt
    // -----------------------------------------------------------------------

    #[test]
    fn test_char_code_at() {
        let this = JSValue::string("ABC");
        assert_eq!(string_char_code_at(&this, &[JSValue::int(0)]).to_number(), 65.0);
        assert_eq!(string_char_code_at(&this, &[JSValue::int(1)]).to_number(), 66.0);
        assert_eq!(string_char_code_at(&this, &[JSValue::int(3)]).to_number().is_nan(), true);
    }

    // -----------------------------------------------------------------------
    // codePointAt
    // -----------------------------------------------------------------------

    #[test]
    fn test_code_point_at() {
        let this = JSValue::string("ABC");
        assert_eq!(
            string_code_point_at(&this, &[JSValue::int(0)]).to_number(),
            65.0
        );
        let emoji = JSValue::string("\u{1f600}");
        assert_eq!(
            string_code_point_at(&emoji, &[JSValue::int(0)]).to_number(),
            128512.0
        );
    }

    // -----------------------------------------------------------------------
    // concat
    // -----------------------------------------------------------------------

    #[test]
    fn test_concat() {
        let this = JSValue::string("hello");
        let result = string_concat(
            &this,
            &[JSValue::string(" "), JSValue::string("world")],
        );
        assert_eq!(result.to_string(), "hello world");
    }

    #[test]
    fn test_concat_no_args() {
        let this = JSValue::string("hello");
        let result = string_concat(&this, &[]);
        assert_eq!(result.to_string(), "hello");
    }

    // -----------------------------------------------------------------------
    // indexOf
    // -----------------------------------------------------------------------

    #[test]
    fn test_index_of() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_index_of(&this, &[JSValue::string("world")]).to_int32(),
            6
        );
        assert_eq!(
            string_index_of(&this, &[JSValue::string("xyz")]).to_int32(),
            -1
        );
        assert_eq!(
            string_index_of(&this, &[JSValue::string("o")]).to_int32(),
            4
        );
    }

    #[test]
    fn test_index_of_from_position() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_index_of(&this, &[JSValue::string("o"), JSValue::int(5)]).to_int32(),
            7
        );
        assert_eq!(
            string_index_of(&this, &[JSValue::string("h"), JSValue::int(1)]).to_int32(),
            -1
        );
    }

    #[test]
    fn test_index_of_empty_search() {
        let this = JSValue::string("hello");
        assert_eq!(
            string_index_of(&this, &[JSValue::string("")]).to_int32(),
            0
        );
        assert_eq!(
            string_index_of(&this, &[JSValue::string(""), JSValue::int(3)]).to_int32(),
            3
        );
    }

    #[test]
    fn test_index_of_unicode() {
        let this = JSValue::string("a\u{1f600}b\u{1f600}c");
        assert_eq!(
            string_index_of(&this, &[JSValue::string("\u{1f600}")]).to_int32(),
            1
        );
        assert_eq!(
            string_index_of(&this, &[JSValue::string("\u{1f600}"), JSValue::int(2)]).to_int32(),
            3
        );
    }

    #[test]
    fn test_index_of_nan_position() {
        // In JS, NaN position is treated as 0 (via ToIntegerOrInfinity)
        let this = JSValue::string("hello");
        assert_eq!(
            string_index_of(&this, &[JSValue::string("o"), JSValue::float(f64::NAN)]).to_int32(),
            4
        );
    }

    // -----------------------------------------------------------------------
    // lastIndexOf
    // -----------------------------------------------------------------------

    #[test]
    fn test_last_index_of() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("o")]).to_int32(),
            7
        );
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("xyz")]).to_int32(),
            -1
        );
    }

    #[test]
    fn test_last_index_of_from_position() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("o"), JSValue::int(4)]).to_int32(),
            4
        );
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("o"), JSValue::int(3)]).to_int32(),
            -1
        );
    }

    #[test]
    fn test_last_index_of_empty() {
        let this = JSValue::string("abc");
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("")]).to_int32(),
            3
        );
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string(""), JSValue::int(1)]).to_int32(),
            1
        );
    }

    #[test]
    fn test_last_index_of_negative_pos() {
        // In JS, negative pos is clamped to 0, so lastIndexOf("a", -1) searches from 0
        let this = JSValue::string("abc");
        assert_eq!(
            string_last_index_of(&this, &[JSValue::string("a"), JSValue::int(-1)]).to_int32(),
            0
        );
    }

    // -----------------------------------------------------------------------
    // includes
    // -----------------------------------------------------------------------

    #[test]
    fn test_includes() {
        let this = JSValue::string("hello world");
        assert!(string_includes(&this, &[JSValue::string("world")]).to_boolean());
        assert!(!string_includes(&this, &[JSValue::string("xyz")]).to_boolean());
    }

    #[test]
    fn test_includes_from_position() {
        let this = JSValue::string("hello world");
        assert!(string_includes(&this, &[JSValue::string("world"), JSValue::int(6)]).to_boolean());
        assert!(!string_includes(&this, &[JSValue::string("hello"), JSValue::int(1)]).to_boolean());
    }

    // -----------------------------------------------------------------------
    // startsWith / endsWith
    // -----------------------------------------------------------------------

    #[test]
    fn test_starts_with() {
        let this = JSValue::string("hello world");
        assert!(string_starts_with(&this, &[JSValue::string("hello")]).to_boolean());
        assert!(!string_starts_with(&this, &[JSValue::string("world")]).to_boolean());
    }

    #[test]
    fn test_starts_with_position() {
        let this = JSValue::string("hello world");
        // "hello world" from position 2 is "llo world" - starts with "lo"? No
        assert!(!string_starts_with(&this, &[JSValue::string("lo"), JSValue::int(2)]).to_boolean());
        // "hello world" from position 2 is "llo world" - starts with "ll"? Yes
        assert!(string_starts_with(&this, &[JSValue::string("ll"), JSValue::int(2)]).to_boolean());
        // "hello world" from position 1 is "ello world" - starts with "hello"? No
        assert!(!string_starts_with(&this, &[JSValue::string("hello"), JSValue::int(1)]).to_boolean());
    }

    #[test]
    fn test_ends_with() {
        let this = JSValue::string("hello world");
        assert!(string_ends_with(&this, &[JSValue::string("world")]).to_boolean());
        assert!(!string_ends_with(&this, &[JSValue::string("hello")]).to_boolean());
    }

    #[test]
    fn test_ends_with_position() {
        let this = JSValue::string("hello world");
        assert!(string_ends_with(&this, &[JSValue::string("llo"), JSValue::int(5)]).to_boolean());
        assert!(!string_ends_with(&this, &[JSValue::string("world"), JSValue::int(5)]).to_boolean());
    }

    // -----------------------------------------------------------------------
    // slice
    // -----------------------------------------------------------------------

    #[test]
    fn test_slice() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_slice(&this, &[JSValue::int(0), JSValue::int(5)]).to_string(),
            "hello"
        );
        assert_eq!(
            string_slice(&this, &[JSValue::int(6)]).to_string(),
            "world"
        );
        assert_eq!(
            string_slice(&this, &[JSValue::int(-5)]).to_string(),
            "world"
        );
        assert_eq!(
            string_slice(&this, &[JSValue::int(-5), JSValue::int(-1)]).to_string(),
            "worl"
        );
    }

    // -----------------------------------------------------------------------
    // substring
    // -----------------------------------------------------------------------

    #[test]
    fn test_substring() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_substring(&this, &[JSValue::int(0), JSValue::int(5)]).to_string(),
            "hello"
        );
        assert_eq!(
            string_substring(&this, &[JSValue::int(6)]).to_string(),
            "world"
        );
        // swap when start > end
        assert_eq!(
            string_substring(&this, &[JSValue::int(5), JSValue::int(0)]).to_string(),
            "hello"
        );
    }

    #[test]
    fn test_substring_negative() {
        let this = JSValue::string("hello");
        // negative indices treated as 0
        assert_eq!(
            string_substring(&this, &[JSValue::int(-1), JSValue::int(3)]).to_string(),
            "hel"
        );
    }

    // -----------------------------------------------------------------------
    // substr
    // -----------------------------------------------------------------------

    #[test]
    fn test_substr() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_substr(&this, &[JSValue::int(0), JSValue::int(5)]).to_string(),
            "hello"
        );
        assert_eq!(
            string_substr(&this, &[JSValue::int(6)]).to_string(),
            "world"
        );
    }

    #[test]
    fn test_substr_negative_start() {
        let this = JSValue::string("hello world");
        assert_eq!(
            string_substr(&this, &[JSValue::int(-5), JSValue::int(5)]).to_string(),
            "world"
        );
    }

    #[test]
    fn test_substr_zero_length() {
        let this = JSValue::string("hello");
        assert_eq!(
            string_substr(&this, &[JSValue::int(0), JSValue::int(0)]).to_string(),
            ""
        );
    }

    // -----------------------------------------------------------------------
    // split
    // -----------------------------------------------------------------------

    #[test]
    fn test_split() {
        let this = JSValue::string("a,b,c");
        let result = string_split(&this, &[JSValue::string(",")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
        assert_eq!(result.get_property("0").unwrap().to_string(), "a");
        assert_eq!(result.get_property("1").unwrap().to_string(), "b");
        assert_eq!(result.get_property("2").unwrap().to_string(), "c");
    }

    #[test]
    fn test_split_with_limit() {
        let this = JSValue::string("a,b,c");
        let result = string_split(
            &this,
            &[JSValue::string(","), JSValue::int(2)],
        );
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 2);
        assert_eq!(result.get_property("0").unwrap().to_string(), "a");
        assert_eq!(result.get_property("1").unwrap().to_string(), "b");
    }

    #[test]
    fn test_split_empty_separator() {
        let this = JSValue::string("abc");
        let result = string_split(&this, &[JSValue::string("")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
        assert_eq!(result.get_property("0").unwrap().to_string(), "a");
        assert_eq!(result.get_property("1").unwrap().to_string(), "b");
        assert_eq!(result.get_property("2").unwrap().to_string(), "c");
    }

    #[test]
    fn test_split_no_separator() {
        let this = JSValue::string("hello");
        let result = string_split(&this, &[JSValue::undefined()]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
        assert_eq!(result.get_property("0").unwrap().to_string(), "hello");
    }

    #[test]
    fn test_split_unicode() {
        let this = JSValue::string("a\u{1f600}b");
        let result = string_split(&this, &[JSValue::string("\u{1f600}")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 2);
        assert_eq!(result.get_property("0").unwrap().to_string(), "a");
        assert_eq!(result.get_property("1").unwrap().to_string(), "b");
    }

    // -----------------------------------------------------------------------
    // trim / trimStart / trimEnd
    // -----------------------------------------------------------------------

    #[test]
    fn test_trim() {
        let this = JSValue::string("  hello  ");
        assert_eq!(string_trim(&this, &[]).to_string(), "hello");
    }

    #[test]
    fn test_trim_start() {
        let this = JSValue::string("  hello  ");
        assert_eq!(string_trim_start(&this, &[]).to_string(), "hello  ");
    }

    #[test]
    fn test_trim_end() {
        let this = JSValue::string("  hello  ");
        assert_eq!(string_trim_end(&this, &[]).to_string(), "  hello");
    }

    // -----------------------------------------------------------------------
    // toUpperCase / toLowerCase
    // -----------------------------------------------------------------------

    #[test]
    fn test_to_upper_lower() {
        let this = JSValue::string("Hello");
        assert_eq!(string_to_upper_case(&this, &[]).to_string(), "HELLO");
        assert_eq!(string_to_lower_case(&this, &[]).to_string(), "hello");
    }

    #[test]
    fn test_to_locale_upper_lower() {
        let this = JSValue::string("Hello");
        assert_eq!(string_to_locale_upper_case(&this, &[]).to_string(), "HELLO");
        assert_eq!(string_to_locale_lower_case(&this, &[]).to_string(), "hello");
    }

    // -----------------------------------------------------------------------
    // replace
    // -----------------------------------------------------------------------

    #[test]
    fn test_replace() {
        let this = JSValue::string("hello world");
        let result = string_replace(&this, &[JSValue::string("world"), JSValue::string("JS")]);
        assert_eq!(result.to_string(), "hello JS");
    }

    #[test]
    fn test_replace_first_only() {
        let this = JSValue::string("aaa");
        let result = string_replace(&this, &[JSValue::string("a"), JSValue::string("b")]);
        assert_eq!(result.to_string(), "baa");
    }

    #[test]
    fn test_replace_empty_search() {
        let this = JSValue::string("hello");
        let result = string_replace(&this, &[JSValue::string(""), JSValue::string("X")]);
        assert_eq!(result.to_string(), "Xhello");
    }

    #[test]
    fn test_replace_not_found() {
        let this = JSValue::string("hello");
        let result = string_replace(&this, &[JSValue::string("xyz"), JSValue::string("a")]);
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_replace_unicode() {
        let this = JSValue::string("a\u{1f600}b");
        let result = string_replace(&this, &[JSValue::string("\u{1f600}"), JSValue::string("X")]);
        assert_eq!(result.to_string(), "aXb");
    }

    // -----------------------------------------------------------------------
    // replaceAll
    // -----------------------------------------------------------------------

    #[test]
    fn test_replace_all() {
        let this = JSValue::string("aaa");
        let result = string_replace_all(&this, &[JSValue::string("a"), JSValue::string("b")]);
        assert_eq!(result.to_string(), "bbb");
    }

    #[test]
    fn test_replace_all_empty_search() {
        let this = JSValue::string("abc");
        let result = string_replace_all(&this, &[JSValue::string(""), JSValue::string("-")]);
        assert_eq!(result.to_string(), "-a-b-c-");
    }

    #[test]
    fn test_replace_all_not_found() {
        let this = JSValue::string("hello");
        let result = string_replace_all(&this, &[JSValue::string("xyz"), JSValue::string("a")]);
        assert_eq!(result.to_string(), "hello");
    }

    // -----------------------------------------------------------------------
    // match
    // -----------------------------------------------------------------------

    #[test]
    fn test_match_literal() {
        let this = JSValue::string("hello world");
        let result = string_match(&this, &[JSValue::string("world")]);
        // Should return array with the match
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
        assert_eq!(result.get_property("0").unwrap().to_string(), "world");
    }

    #[test]
    fn test_match_not_found() {
        let this = JSValue::string("hello world");
        let result = string_match(&this, &[JSValue::string("xyz")]);
        assert!(result.is_null());
    }

    #[test]
    fn test_match_empty_pattern() {
        let this = JSValue::string("hello");
        let result = string_match(&this, &[JSValue::string("")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
        assert_eq!(result.get_property("0").unwrap().to_string(), "");
    }

    // -----------------------------------------------------------------------
    // matchAll
    // -----------------------------------------------------------------------

    #[test]
    fn test_match_all_literal() {
        let this = JSValue::string("aaa");
        let result = string_match_all(&this, &[JSValue::string("a")]);
        let len = result.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
        assert_eq!(result.get_property("0").unwrap().get_property("0").unwrap().to_string(), "a");
    }

    // -----------------------------------------------------------------------
    // search
    // -----------------------------------------------------------------------

    #[test]
    fn test_search_literal() {
        let this = JSValue::string("hello world");
        let result = string_search(&this, &[JSValue::string("world")]);
        assert_eq!(result.to_number(), 6.0);
    }

    #[test]
    fn test_search_not_found() {
        let this = JSValue::string("hello world");
        let result = string_search(&this, &[JSValue::string("xyz")]);
        assert_eq!(result.to_number(), -1.0);
    }

    // -----------------------------------------------------------------------
    // padStart / padEnd
    // -----------------------------------------------------------------------

    #[test]
    fn test_pad_start() {
        let this = JSValue::string("5");
        let result = string_pad_start(&this, &[JSValue::int(3), JSValue::string("0")]);
        assert_eq!(result.to_string(), "005");
    }

    #[test]
    fn test_pad_end() {
        let this = JSValue::string("5");
        let result = string_pad_end(&this, &[JSValue::int(3), JSValue::string("0")]);
        assert_eq!(result.to_string(), "500");
    }

    #[test]
    fn test_pad_start_already_long() {
        let this = JSValue::string("hello");
        let result = string_pad_start(&this, &[JSValue::int(3)]);
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_pad_start_unicode() {
        let this = JSValue::string("5");
        let result = string_pad_start(&this, &[JSValue::int(4), JSValue::string("\u{1f600}")]);
        assert_eq!(result.to_string(), "\u{1f600}\u{1f600}\u{1f600}5");
    }

    // -----------------------------------------------------------------------
    // repeat
    // -----------------------------------------------------------------------

    #[test]
    fn test_repeat() {
        let this = JSValue::string("abc");
        assert_eq!(string_repeat(&this, &[JSValue::int(3)]).to_string(), "abcabcabc");
        assert_eq!(
            string_repeat(&this, &[JSValue::int(0)]).to_string(),
            ""
        );
    }

    #[test]
    fn test_repeat_negative() {
        let this = JSValue::string("abc");
        assert!(string_repeat(&this, &[JSValue::int(-1)]).is_undefined());
    }

    // -----------------------------------------------------------------------
    // at
    // -----------------------------------------------------------------------

    #[test]
    fn test_at() {
        let this = JSValue::string("hello");
        assert_eq!(string_at(&this, &[JSValue::int(0)]).to_string(), "h");
        assert_eq!(string_at(&this, &[JSValue::int(-1)]).to_string(), "o");
        assert_eq!(string_at(&this, &[JSValue::int(-5)]).to_string(), "h");
        assert!(string_at(&this, &[JSValue::int(5)]).is_undefined());
        assert!(string_at(&this, &[JSValue::int(-6)]).is_undefined());
    }

    // -----------------------------------------------------------------------
    // fromCharCode / fromCodePoint / raw
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_char_code() {
        let this = JSValue::undefined();
        let result =
            string_from_char_code(&this, &[JSValue::int(72), JSValue::int(101), JSValue::int(108)]);
        assert_eq!(result.to_string(), "Hel");
    }

    #[test]
    fn test_from_code_point() {
        let this = JSValue::undefined();
        let result = string_from_code_point(&this, &[JSValue::int(65), JSValue::int(0x1f600)]);
        assert_eq!(result.to_string(), "A\u{1f600}");
    }

    #[test]
    fn test_raw() {
        let this = JSValue::undefined();
        let template = JSValue::object("Object");
        let raw = JSValue::object("Object");
        raw.set_property("0", JSValue::string("hello\\nworld"));
        raw.set_property("length", JSValue::int(1));
        template.set_property("raw", raw);
        let result = string_raw(&this, &[template]);
        assert_eq!(result.to_string(), "hello\\nworld");
    }

    #[test]
    fn test_raw_with_substitutions() {
        let this = JSValue::undefined();
        let template = JSValue::object("Object");
        let raw = JSValue::object("Object");
        raw.set_property("0", JSValue::string("hello "));
        raw.set_property("1", JSValue::string(" world"));
        raw.set_property("length", JSValue::int(2));
        template.set_property("raw", raw);
        let result = string_raw(&this, &[template, JSValue::string("beautiful")]);
        assert_eq!(result.to_string(), "hello beautiful world");
    }

    // -----------------------------------------------------------------------
    // normalize / valueOf / toString
    // -----------------------------------------------------------------------

    #[test]
    fn test_normalize() {
        let this = JSValue::string("hello");
        assert_eq!(string_normalize(&this, &[]).to_string(), "hello");
    }

    #[test]
    fn test_value_of_to_string() {
        let this = JSValue::string("hello");
        assert_eq!(string_value_of(&this, &[]).to_string(), "hello");
        assert_eq!(string_to_string(&this, &[]).to_string(), "hello");
    }
}
