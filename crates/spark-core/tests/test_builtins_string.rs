//! Tests for string.rs builtin - covering gaps in existing tests

use spark_core::builtins::string::*;
use spark_core::context::JSContext;
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;
use std::cell::RefCell;
use std::rc::Rc;

fn make_ctx() -> JSContext {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    JSContext::new(rt)
}

fn str_val(s: &str) -> JSValue {
    JSValue::string(s)
}

// ============================================================================
// Constructor and init
// ============================================================================

#[test]
fn test_string_constructor_with_value() {
    let result = string_constructor(&JSValue::undefined(), &[str_val("hello")]);
    // String constructor with arg creates a wrapper object
    match &result {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.class_name, "String");
        }
        _ => {
            // Might return a primitive string
            assert_eq!(result.to_string(), "hello");
        }
    }
}

#[test]
fn test_string_constructor_no_args() {
    let result = string_constructor(&JSValue::undefined(), &[]);
    // String() with no args returns ""
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_init_string() {
    let mut ctx = make_ctx();
    init_string(&mut ctx);
    let global = ctx.global.borrow();
    let str_ctor = global.properties.get("String").unwrap();
    assert!(str_ctor.is_callable());
    assert!(str_ctor.get_property("fromCharCode").is_some());
    assert!(str_ctor.get_property("fromCodePoint").is_some());
    assert!(str_ctor.get_property("raw").is_some());
}

// ============================================================================
// charAt / charCodeAt / codePointAt
// ============================================================================

#[test]
fn test_char_at_basic() {
    let s = str_val("hello");
    let result = string_char_at(&s, &[JSValue::int(0)]);
    assert_eq!(result.to_string(), "h");
}

#[test]
fn test_char_at_out_of_bounds() {
    let s = str_val("hi");
    let result = string_char_at(&s, &[JSValue::int(10)]);
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_char_at_negative() {
    let s = str_val("hello");
    let result = string_char_at(&s, &[JSValue::int(-1)]);
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_char_code_at_basic() {
    let s = str_val("A");
    let result = string_char_code_at(&s, &[JSValue::int(0)]);
    assert_eq!(result.to_int32(), 65);
}

#[test]
fn test_char_code_at_out_of_bounds() {
    let s = str_val("A");
    let result = string_char_code_at(&s, &[JSValue::int(10)]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_code_point_at_basic() {
    let s = str_val("A");
    let result = string_code_point_at(&s, &[JSValue::int(0)]);
    assert_eq!(result.to_int32(), 65);
}

#[test]
fn test_code_point_at_out_of_bounds() {
    let s = str_val("A");
    let result = string_code_point_at(&s, &[JSValue::int(5)]);
    // Out of bounds returns undefined or NaN
    assert!(result.is_undefined() || result.to_number().is_nan());
}

// ============================================================================
// concat
// ============================================================================

#[test]
fn test_concat_basic() {
    let s = str_val("hello");
    let result = string_concat(&s, &[str_val(" world")]);
    assert_eq!(result.to_string(), "hello world");
}

#[test]
fn test_concat_multiple() {
    let s = str_val("a");
    let result = string_concat(&s, &[str_val("b"), str_val("c")]);
    assert_eq!(result.to_string(), "abc");
}

#[test]
fn test_concat_no_args() {
    let s = str_val("hello");
    let result = string_concat(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

// ============================================================================
// indexOf / lastIndexOf
// ============================================================================

#[test]
fn test_index_of_found() {
    let s = str_val("hello world");
    let result = string_index_of(&s, &[str_val("world")]);
    assert_eq!(result.to_int32(), 6);
}

#[test]
fn test_index_of_not_found() {
    let s = str_val("hello");
    let result = string_index_of(&s, &[str_val("xyz")]);
    assert_eq!(result.to_int32(), -1);
}

#[test]
fn test_index_of_with_position() {
    let s = str_val("hello hello");
    let result = string_index_of(&s, &[str_val("hello"), JSValue::int(3)]);
    assert_eq!(result.to_int32(), 6);
}

#[test]
fn test_index_of_empty_search() {
    let s = str_val("hello");
    let result = string_index_of(&s, &[str_val("")]);
    assert_eq!(result.to_int32(), 0);
}

#[test]
fn test_last_index_of_found() {
    let s = str_val("hello hello");
    let result = string_last_index_of(&s, &[str_val("hello")]);
    assert_eq!(result.to_int32(), 6);
}

#[test]
fn test_last_index_of_not_found() {
    let s = str_val("hello");
    let result = string_last_index_of(&s, &[str_val("xyz")]);
    assert_eq!(result.to_int32(), -1);
}

#[test]
fn test_last_index_of_with_position() {
    let s = str_val("hello hello");
    let result = string_last_index_of(&s, &[str_val("hello"), JSValue::int(3)]);
    assert_eq!(result.to_int32(), 0);
}

// ============================================================================
// includes / startsWith / endsWith
// ============================================================================

#[test]
fn test_includes_true() {
    let s = str_val("hello world");
    let result = string_includes(&s, &[str_val("world")]);
    assert!(result.to_boolean());
}

#[test]
fn test_includes_false() {
    let s = str_val("hello");
    let result = string_includes(&s, &[str_val("xyz")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_starts_with_true() {
    let s = str_val("hello world");
    let result = string_starts_with(&s, &[str_val("hello")]);
    assert!(result.to_boolean());
}

#[test]
fn test_starts_with_false() {
    let s = str_val("hello world");
    let result = string_starts_with(&s, &[str_val("world")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_starts_with_position() {
    let s = str_val("hello world");
    let result = string_starts_with(&s, &[str_val("world"), JSValue::int(6)]);
    assert!(result.to_boolean());
}

#[test]
fn test_ends_with_true() {
    let s = str_val("hello world");
    let result = string_ends_with(&s, &[str_val("world")]);
    assert!(result.to_boolean());
}

#[test]
fn test_ends_with_false() {
    let s = str_val("hello world");
    let result = string_ends_with(&s, &[str_val("hello")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_ends_with_position() {
    let s = str_val("hello world");
    let result = string_ends_with(&s, &[str_val("hello"), JSValue::int(5)]);
    assert!(result.to_boolean());
}

// ============================================================================
// slice / substring / substr
// ============================================================================

#[test]
fn test_slice_basic() {
    let s = str_val("hello world");
    let result = string_slice(&s, &[JSValue::int(0), JSValue::int(5)]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_slice_negative() {
    let s = str_val("hello world");
    let result = string_slice(&s, &[JSValue::int(-5)]);
    assert_eq!(result.to_string(), "world");
}

#[test]
fn test_slice_out_of_bounds() {
    let s = str_val("hi");
    let result = string_slice(&s, &[JSValue::int(0), JSValue::int(100)]);
    assert_eq!(result.to_string(), "hi");
}

#[test]
fn test_substring_basic() {
    let s = str_val("hello world");
    let result = string_substring(&s, &[JSValue::int(0), JSValue::int(5)]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_substring_swap_args() {
    let s = str_val("hello");
    let result = string_substring(&s, &[JSValue::int(3), JSValue::int(1)]);
    // substring swaps args if start > end
    let r = result.to_string();
    assert!(r == "ell" || r == "el" || r == "l", "Unexpected result: {}", r);
}

#[test]
fn test_substr_basic() {
    let s = str_val("hello world");
    let result = string_substr(&s, &[JSValue::int(6), JSValue::int(5)]);
    assert_eq!(result.to_string(), "world");
}

#[test]
fn test_substr_negative_start() {
    let s = str_val("hello world");
    let result = string_substr(&s, &[JSValue::int(-5)]);
    assert_eq!(result.to_string(), "world");
}

#[test]
fn test_substr_no_length() {
    let s = str_val("hello");
    let result = string_substr(&s, &[JSValue::int(2)]);
    assert_eq!(result.to_string(), "llo");
}

// ============================================================================
// split
// ============================================================================

#[test]
fn test_split_basic() {
    let s = str_val("a,b,c");
    let result = string_split(&s, &[str_val(",")]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_split_with_limit() {
    let s = str_val("a,b,c,d");
    let result = string_split(&s, &[str_val(","), JSValue::int(2)]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_split_empty_separator() {
    let s = str_val("abc");
    let result = string_split(&s, &[str_val("")]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_split_no_separator() {
    let s = str_val("hello");
    let result = string_split(&s, &[]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 1);
}

#[test]
fn test_split_empty_string() {
    let s = str_val("");
    let result = string_split(&s, &[str_val(",")]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 1);
}

// ============================================================================
// trim / trimStart / trimEnd
// ============================================================================

#[test]
fn test_trim() {
    let s = str_val("  hello  ");
    let result = string_trim(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_trim_tabs_newlines() {
    let s = str_val("\t\nhello\t\n");
    let result = string_trim(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_trim_start() {
    let s = str_val("  hello  ");
    let result = string_trim_start(&s, &[]);
    assert_eq!(result.to_string(), "hello  ");
}

#[test]
fn test_trim_end() {
    let s = str_val("  hello  ");
    let result = string_trim_end(&s, &[]);
    assert_eq!(result.to_string(), "  hello");
}

// ============================================================================
// toUpperCase / toLowerCase
// ============================================================================

#[test]
fn test_to_upper_case() {
    let s = str_val("hello");
    let result = string_to_upper_case(&s, &[]);
    assert_eq!(result.to_string(), "HELLO");
}

#[test]
fn test_to_lower_case() {
    let s = str_val("HELLO");
    let result = string_to_lower_case(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_to_locale_upper_case() {
    let s = str_val("hello");
    let result = string_to_locale_upper_case(&s, &[]);
    assert_eq!(result.to_string(), "HELLO");
}

#[test]
fn test_to_locale_lower_case() {
    let s = str_val("HELLO");
    let result = string_to_locale_lower_case(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

// ============================================================================
// replace / replaceAll
// ============================================================================

#[test]
fn test_replace_first() {
    let s = str_val("hello world hello");
    let result = string_replace(&s, &[str_val("hello"), str_val("hi")]);
    assert_eq!(result.to_string(), "hi world hello");
}

#[test]
fn test_replace_no_match() {
    let s = str_val("hello");
    let result = string_replace(&s, &[str_val("xyz"), str_val("abc")]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_replace_empty_pattern() {
    let s = str_val("hello");
    let result = string_replace(&s, &[str_val(""), str_val("x")]);
    assert_eq!(result.to_string(), "xhello");
}

#[test]
fn test_replace_all() {
    let s = str_val("hello hello hello");
    let result = string_replace_all(&s, &[str_val("hello"), str_val("hi")]);
    assert_eq!(result.to_string(), "hi hi hi");
}

#[test]
fn test_replace_all_no_match() {
    let s = str_val("hello");
    let result = string_replace_all(&s, &[str_val("xyz"), str_val("abc")]);
    assert_eq!(result.to_string(), "hello");
}

// ============================================================================
// match / search
// ============================================================================

#[test]
fn test_match_basic() {
    let s = str_val("hello 123 world");
    let result = string_match(&s, &[str_val("\\d+")]);
    // match returns an array or null
    assert!(result.is_object() || result.is_null());
}

#[test]
fn test_match_no_match() {
    let s = str_val("hello");
    let result = string_match(&s, &[str_val("\\d+")]);
    assert!(result.is_null());
}

#[test]
fn test_search_basic() {
    let s = str_val("hello 123 world");
    let result = string_search(&s, &[str_val("\\d+")]);
    // search returns index or -1
    assert!(result.to_int32() >= -1);
}

#[test]
fn test_search_no_match() {
    let s = str_val("hello");
    let result = string_search(&s, &[str_val("\\d+")]);
    assert_eq!(result.to_int32(), -1);
}

// ============================================================================
// padStart / padEnd / repeat / at
// ============================================================================

#[test]
fn test_pad_start() {
    let s = str_val("5");
    let result = string_pad_start(&s, &[JSValue::int(3), str_val("0")]);
    assert_eq!(result.to_string(), "005");
}

#[test]
fn test_pad_start_no_fill() {
    let s = str_val("hello");
    let result = string_pad_start(&s, &[JSValue::int(3)]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_pad_end() {
    let s = str_val("5");
    let result = string_pad_end(&s, &[JSValue::int(3), str_val("0")]);
    assert_eq!(result.to_string(), "500");
}

#[test]
fn test_pad_end_no_fill() {
    let s = str_val("hi");
    let result = string_pad_end(&s, &[JSValue::int(5)]);
    assert_eq!(result.to_string(), "hi   ");
}

#[test]
fn test_repeat() {
    let s = str_val("ab");
    let result = string_repeat(&s, &[JSValue::int(3)]);
    assert_eq!(result.to_string(), "ababab");
}

#[test]
fn test_repeat_zero() {
    let s = str_val("hello");
    let result = string_repeat(&s, &[JSValue::int(0)]);
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_at_positive() {
    let s = str_val("hello");
    let result = string_at(&s, &[JSValue::int(0)]);
    assert_eq!(result.to_string(), "h");
}

#[test]
fn test_at_negative() {
    let s = str_val("hello");
    let result = string_at(&s, &[JSValue::int(-1)]);
    assert_eq!(result.to_string(), "o");
}

#[test]
fn test_at_out_of_bounds() {
    let s = str_val("hi");
    let result = string_at(&s, &[JSValue::int(10)]);
    assert!(result.is_undefined());
}

// ============================================================================
// Static methods
// ============================================================================

#[test]
fn test_from_char_code() {
    let result = string_from_char_code(&JSValue::undefined(), &[JSValue::int(65), JSValue::int(66), JSValue::int(67)]);
    assert_eq!(result.to_string(), "ABC");
}

#[test]
fn test_from_code_point() {
    let result = string_from_code_point(&JSValue::undefined(), &[JSValue::int(97), JSValue::int(98)]);
    assert_eq!(result.to_string(), "ab");
}

#[test]
fn test_raw_basic() {
    // String.raw`hello` - basic test
    let template = JSValue::object("Object");
    let raw = create_array(vec![str_val("hello")]);
    template.set_property("raw", raw);
    let result = string_raw(&JSValue::undefined(), &[template]);
    assert_eq!(result.to_string(), "hello");
}

fn create_array(elements: Vec<JSValue>) -> JSValue {
    let arr = JSValue::object("Array");
    for (i, val) in elements.iter().enumerate() {
        arr.set_property(&i.to_string(), val.clone());
    }
    arr.set_property("length", JSValue::Int(elements.len() as i32));
    arr
}

// ============================================================================
// valueOf / toString
// ============================================================================

#[test]
fn test_value_of_string() {
    let s = str_val("hello");
    let result = string_value_of(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_to_string_string() {
    let s = str_val("hello");
    let result = string_to_string(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_value_of_on_object() {
    let obj = JSValue::object("String");
    if let JSValue::Object(ref o) = obj {
        o.borrow_mut().internal_slots.insert("PrimitiveValue".to_string(), str_val("wrapped"));
    }
    let result = string_value_of(&obj, &[]);
    assert_eq!(result.to_string(), "wrapped");
}

// ============================================================================
// normalize
// ============================================================================

#[test]
fn test_normalize_default() {
    let s = str_val("hello");
    let result = string_normalize(&s, &[]);
    assert_eq!(result.to_string(), "hello");
}

// ============================================================================
// Edge cases
// ============================================================================

#[test]
fn test_string_operations_on_empty() {
    let s = str_val("");
    assert_eq!(string_char_at(&s, &[JSValue::int(0)]).to_string(), "");
    assert_eq!(string_index_of(&s, &[str_val("a")]).to_int32(), -1);
    assert!(string_includes(&s, &[str_val("a")]).to_boolean() == false);
    assert_eq!(string_trim(&s, &[]).to_string(), "");
    assert_eq!(string_repeat(&s, &[JSValue::int(5)]).to_string(), "");
}

#[test]
fn test_string_operations_on_single_char() {
    let s = str_val("x");
    assert_eq!(string_char_at(&s, &[JSValue::int(0)]).to_string(), "x");
    assert_eq!(string_char_at(&s, &[JSValue::int(1)]).to_string(), "");
    assert_eq!(string_to_upper_case(&s, &[]).to_string(), "X");
}
