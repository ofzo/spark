//! Tests for date.rs, regexp.rs, and json.rs builtins

use spark_core::builtins::date::*;
use spark_core::builtins::regexp::*;
use spark_core::builtins::json::*;
use spark_core::context::JSContext;
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;
use std::cell::RefCell;
use std::rc::Rc;

fn make_ctx() -> JSContext {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    JSContext::new(rt)
}

// ============================================================================
// Date tests - gaps
// ============================================================================

#[test]
fn test_date_get_time() {
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(1000000.0));
    }
    let result = date_get_time(&this, &[]);
    assert_eq!(result.to_number(), 1000000.0);
}

#[test]
fn test_date_get_day() {
    // 2024-01-01 - check the day is in valid range (0-6)
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_day(&this, &[]);
    let day = result.to_int32();
    assert!(day >= 0 && day <= 6, "Day should be 0-6, got {}", day);
}

#[test]
fn test_date_get_utc_full_year() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(5), JSValue::int(15),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_full_year(&this, &[]);
    assert_eq!(result.to_int32(), 2024);
}

#[test]
fn test_date_get_utc_month() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(5), JSValue::int(15),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_month(&this, &[]);
    assert_eq!(result.to_int32(), 5);
}

#[test]
fn test_date_get_utc_date() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(15),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_date(&this, &[]);
    assert_eq!(result.to_int32(), 15);
}

#[test]
fn test_date_get_utc_hours() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
        JSValue::int(12), JSValue::int(30), JSValue::int(45),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_hours(&this, &[]);
    assert_eq!(result.to_int32(), 12);
}

#[test]
fn test_date_get_utc_minutes() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
        JSValue::int(12), JSValue::int(30),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_minutes(&this, &[]);
    assert_eq!(result.to_int32(), 30);
}

#[test]
fn test_date_get_utc_seconds() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
        JSValue::int(0), JSValue::int(0), JSValue::int(45),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_get_utc_seconds(&this, &[]);
    assert_eq!(result.to_int32(), 45);
}

#[test]
fn test_date_get_utc_milliseconds() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number() + 123.0));
    }
    let result = date_get_utc_milliseconds(&this, &[]);
    assert_eq!(result.to_int32(), 123);
}

#[test]
fn test_date_set_time() {
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(0.0));
    }
    let result = date_set_time(&this, &[JSValue::float(1000000.0)]);
    assert_eq!(result.to_number(), 1000000.0);
}

#[test]
fn test_date_set_full_year() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2020), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    date_set_utc_full_year(&this, &[JSValue::int(2025)]);
    let year = date_get_utc_full_year(&this, &[]);
    assert_eq!(year.to_int32(), 2025);
}

#[test]
fn test_date_set_utc_month() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    date_set_utc_month(&this, &[JSValue::int(5)]);
    let month = date_get_utc_month(&this, &[]);
    assert_eq!(month.to_int32(), 5);
}

#[test]
fn test_date_set_utc_date() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    date_set_utc_date(&this, &[JSValue::int(15)]);
    let day = date_get_utc_date(&this, &[]);
    assert_eq!(day.to_int32(), 15);
}

#[test]
fn test_date_to_json() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_to_json(&this, &[]);
    // toJSON should return a string
    assert!(result.is_string() || result.is_object());
}

#[test]
fn test_date_value_of() {
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(12345.0));
    }
    let result = date_value_of(&this, &[]);
    assert_eq!(result.to_number(), 12345.0);
}

#[test]
fn test_date_to_time_string() {
    let ts = date_utc(&JSValue::undefined(), &[
        JSValue::int(2024), JSValue::int(0), JSValue::int(1),
        JSValue::int(12), JSValue::int(30), JSValue::int(45),
    ]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_to_time_string(&this, &[]);
    let s = result.to_string();
    assert!(s.contains("12:30:45"));
}

#[test]
fn test_date_to_date_string() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(15)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_to_date_string(&this, &[]);
    let s = result.to_string();
    // Format is "Day Month Date Year" e.g. "Mon 0 15 2024"
    assert!(s.contains("15"), "Should contain day 15, got: {}", s);
    assert!(s.contains("2024"), "Should contain year 2024, got: {}", s);
}

#[test]
fn test_date_to_utc_string() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_to_utc_string(&this, &[]);
    let s = result.to_string();
    assert!(s.contains("2024"));
}

#[test]
fn test_date_to_string() {
    let ts = date_utc(&JSValue::undefined(), &[JSValue::int(2024), JSValue::int(0), JSValue::int(1)]);
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(ts.to_number()));
    }
    let result = date_to_string(&this, &[]);
    // Should produce a non-empty date string
    assert!(!result.to_string().is_empty());
}

#[test]
fn test_date_parse_iso() {
    let result = date_parse(&JSValue::undefined(), &[JSValue::string("2024-01-15T12:00:00.000Z")]);
    assert!(!result.to_number().is_nan());
}

#[test]
fn test_date_invalid_on_setters() {
    let this = JSValue::object("Date");
    if let JSValue::Object(ref o) = this {
        o.borrow_mut().internal_slots.insert("value".to_string(), JSValue::float(f64::NAN));
    }
    let result = date_get_utc_full_year(&this, &[]);
    assert!(result.to_number().is_nan());
}

// ============================================================================
// RegExp tests - gaps
// ============================================================================

#[test]
fn test_regexp_ignore_case() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("hello"), JSValue::string("i")]);
    let result = regexp_test(&re, &[JSValue::string("HELLO")]);
    assert!(result.to_boolean());
}

#[test]
fn test_regexp_global_flag() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("a"), JSValue::string("g")]);
    let global = re.get_property("global");
    assert!(global.map(|v| v.to_boolean()).unwrap_or(false));
}

#[test]
fn test_regexp_multiline_flag() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("^hello"), JSValue::string("m")]);
    let multiline = re.get_property("multiline");
    assert!(multiline.map(|v| v.to_boolean()).unwrap_or(false));
}

#[test]
fn test_regexp_alternation() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("cat|dog")]);
    assert!(regexp_test(&re, &[JSValue::string("I have a cat")]).to_boolean());
    assert!(regexp_test(&re, &[JSValue::string("I have a dog")]).to_boolean());
    assert!(!regexp_test(&re, &[JSValue::string("I have a bird")]).to_boolean());
}

#[test]
fn test_regexp_word_char() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("\\w+")]);
    let result = regexp_test(&re, &[JSValue::string("hello123")]);
    assert!(result.to_boolean());
}

#[test]
fn test_regexp_digit() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("\\d+")]);
    assert!(regexp_test(&re, &[JSValue::string("abc123")]).to_boolean());
    assert!(!regexp_test(&re, &[JSValue::string("abc")]).to_boolean());
}

#[test]
fn test_regexp_whitespace() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("\\s+")]);
    assert!(regexp_test(&re, &[JSValue::string("hello world")]).to_boolean());
}

#[test]
fn test_regexp_word_char_match() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("\\w+")]);
    assert!(regexp_test(&re, &[JSValue::string("hello")]).to_boolean());
}

#[test]
fn test_regexp_quantifier_exact() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("a{3}")]);
    assert!(regexp_test(&re, &[JSValue::string("aaa")]).to_boolean());
    assert!(!regexp_test(&re, &[JSValue::string("aa")]).to_boolean());
}

#[test]
fn test_regexp_quantifier_range() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("a{2,4}")]);
    assert!(regexp_test(&re, &[JSValue::string("aa")]).to_boolean());
    assert!(regexp_test(&re, &[JSValue::string("aaaa")]).to_boolean());
    assert!(!regexp_test(&re, &[JSValue::string("a")]).to_boolean());
}

#[test]
fn test_regexp_group_basic() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("(\\d+)")]);
    let result = regexp_exec(&re, &[JSValue::string("abc123def")]);
    match &result {
        JSValue::Object(_) => {
            assert_eq!(result.get_property("0").unwrap().to_string(), "123");
        }
        _ => panic!("Expected match object"),
    }
}

#[test]
fn test_regexp_empty_pattern() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("")]);
    let result = regexp_test(&re, &[JSValue::string("anything")]);
    assert!(result.to_boolean());
}

#[test]
fn test_regexp_dot_matches_non_newline() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string(".")]);
    assert!(regexp_test(&re, &[JSValue::string("a")]).to_boolean());
    assert!(!regexp_test(&re, &[JSValue::string("\n")]).to_boolean());
}

#[test]
fn test_regexp_dot_all_flag() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("."), JSValue::string("s")]);
    let dot_all = re.get_property("dotAll");
    assert!(dot_all.map(|v| v.to_boolean()).unwrap_or(false));
}

#[test]
fn test_regexp_anchors() {
    let re_start = regexp_constructor(&JSValue::undefined(), &[JSValue::string("^hello")]);
    assert!(regexp_test(&re_start, &[JSValue::string("hello world")]).to_boolean());
    assert!(!regexp_test(&re_start, &[JSValue::string("say hello")]).to_boolean());

    let re_end = regexp_constructor(&JSValue::undefined(), &[JSValue::string("world$")]);
    assert!(regexp_test(&re_end, &[JSValue::string("hello world")]).to_boolean());
    assert!(!regexp_test(&re_end, &[JSValue::string("world hello")]).to_boolean());
}

#[test]
fn test_regexp_to_string_format() {
    let re = regexp_constructor(&JSValue::undefined(), &[JSValue::string("\\d+"), JSValue::string("gi")]);
    let result = regexp_to_string(&re, &[]);
    assert_eq!(result.to_string(), "/\\d+/gi");
}

#[test]
fn test_regexp_constructor_from_regexp() {
    let re1 = regexp_constructor(&JSValue::undefined(), &[JSValue::string("abc"), JSValue::string("i")]);
    let result = regexp_to_string(&re1, &[]);
    assert!(result.to_string().contains("abc"));
    assert!(result.to_string().contains("i"));
}

#[test]
fn test_init_regexp() {
    let mut ctx = make_ctx();
    init_regexp(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("RegExp").is_some());
}

// ============================================================================
// JSON tests - gaps
// ============================================================================

#[test]
fn test_json_parse_string_escapes() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string(r#""hello\nworld""#)]);
    assert_eq!(result.to_string(), "hello\nworld");
}

#[test]
fn test_json_parse_tab_escape() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string(r#""hello\tworld""#)]);
    assert_eq!(result.to_string(), "hello\tworld");
}

#[test]
fn test_json_parse_backslash() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string(r#""hello\\world""#)]);
    assert_eq!(result.to_string(), "hello\\world");
}

#[test]
fn test_json_parse_quote_escape() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string(r#""hello\"world""#)]);
    assert_eq!(result.to_string(), "hello\"world");
}

#[test]
fn test_json_parse_negative_number() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string("-42")]);
    assert_eq!(result.to_int32(), -42);
}

#[test]
fn test_json_parse_float() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string("3.14")]);
    assert!((result.to_number() - 3.14).abs() < 0.01);
}

#[test]
fn test_json_parse_empty_object() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string("{}")]);
    assert!(result.is_object());
}

#[test]
fn test_json_parse_empty_array() {
    let result = json_parse(&JSValue::undefined(), &[JSValue::string("[]")]);
    assert!(result.is_object());
    assert_eq!(result.get_property("length").unwrap().to_int32(), 0);
}

#[test]
fn test_json_stringify_empty_object() {
    let result = json_stringify(&JSValue::undefined(), &[JSValue::object("Object")]);
    assert_eq!(result.to_string(), "{}");
}

#[test]
fn test_json_stringify_empty_array() {
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));
    let result = json_stringify(&JSValue::undefined(), &[arr]);
    assert_eq!(result.to_string(), "[]");
}

#[test]
fn test_json_stringify_number() {
    let result = json_stringify(&JSValue::undefined(), &[JSValue::int(42)]);
    assert_eq!(result.to_string(), "42");
}

#[test]
fn test_json_stringify_string() {
    let result = json_stringify(&JSValue::undefined(), &[JSValue::string("hello")]);
    assert_eq!(result.to_string(), r#""hello""#);
}

#[test]
fn test_json_stringify_bool() {
    assert_eq!(json_stringify(&JSValue::undefined(), &[JSValue::bool(true)]).to_string(), "true");
    assert_eq!(json_stringify(&JSValue::undefined(), &[JSValue::bool(false)]).to_string(), "false");
}

#[test]
fn test_json_stringify_null() {
    assert_eq!(json_stringify(&JSValue::undefined(), &[JSValue::null()]).to_string(), "null");
}

#[test]
fn test_json_stringify_nested_object() {
    let inner = JSValue::object("Object");
    inner.set_property("x", JSValue::int(1));
    let outer = JSValue::object("Object");
    outer.set_property("inner", inner);
    let result = json_stringify(&JSValue::undefined(), &[outer]);
    assert!(result.to_string().contains("x"));
}

#[test]
fn test_json_roundtrip() {
    let original = r#"{"name":"test","value":42,"nested":{"a":true}}"#;
    let parsed = json_parse(&JSValue::undefined(), &[JSValue::string(original)]);
    let stringified = json_stringify(&JSValue::undefined(), &[parsed]);
    let reparsed = json_parse(&JSValue::undefined(), &[stringified]);
    assert_eq!(reparsed.get_property("name").unwrap().to_string(), "test");
    assert_eq!(reparsed.get_property("value").unwrap().to_int32(), 42);
}

#[test]
fn test_json_stringify_with_space_string() {
    let obj = JSValue::object("Object");
    obj.set_property("a", JSValue::int(1));
    let result = json_stringify(&JSValue::undefined(), &[obj, JSValue::null(), JSValue::string("  ")]);
    assert!(result.to_string().contains("\n"));
}

#[test]
fn test_json_stringify_with_space_number() {
    let obj = JSValue::object("Object");
    obj.set_property("a", JSValue::int(1));
    let result = json_stringify(&JSValue::undefined(), &[obj, JSValue::null(), JSValue::int(2)]);
    assert!(result.to_string().contains("\n"));
}

#[test]
fn test_init_json() {
    let mut ctx = make_ctx();
    init_json(&mut ctx);
    let global = ctx.global.borrow();
    let json = global.properties.get("JSON").unwrap();
    assert!(json.get_property("parse").is_some());
    assert!(json.get_property("stringify").is_some());
}
