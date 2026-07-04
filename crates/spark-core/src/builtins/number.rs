#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Number built-in.
//!
//! Implements the JavaScript Number constructor and its methods.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Number constructor
// ============================================================================

/// Number constructor - `new Number(value)` or `Number(value)`
pub fn number_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
    JSValue::float(val)
}

// ============================================================================
// Number static methods
// ============================================================================

/// Number.isNaN(value)
pub fn number_is_nan(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.get(0) {
        Some(v) => {
            // Number.isNaN only returns true for actual NaN number values
            if !v.is_number() {
                return JSValue::bool(false);
            }
            JSValue::bool(v.to_number().is_nan())
        }
        None => JSValue::bool(false),
    }
}

/// Number.isFinite(value)
pub fn number_is_finite(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.get(0) {
        Some(v) => {
            if !v.is_number() {
                return JSValue::bool(false);
            }
            let n = v.to_number();
            JSValue::bool(!n.is_nan() && !n.is_infinite())
        }
        None => JSValue::bool(false),
    }
}

/// Number.isInteger(value)
pub fn number_is_integer(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.get(0) {
        Some(v) => {
            if !v.is_number() {
                return JSValue::bool(false);
            }
            let n = v.to_number();
            if n.is_nan() || n.is_infinite() {
                return JSValue::bool(false);
            }
            JSValue::bool(n.fract() == 0.0)
        }
        None => JSValue::bool(false),
    }
}

/// Number.isSafeInteger(value)
pub fn number_is_safe_integer(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.get(0) {
        Some(v) => {
            if !v.is_number() {
                return JSValue::bool(false);
            }
            let n = v.to_number();
            if n.is_nan() || n.is_infinite() {
                return JSValue::bool(false);
            }
            if n.fract() != 0.0 {
                return JSValue::bool(false);
            }
            JSValue::bool(n.abs() <= 9007199254740991.0_f64)
        }
        None => JSValue::bool(false),
    }
}

/// Number.parseInt(string, radix)
pub fn number_parse_int(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let radix = args.get(1).map(|v| v.to_int32()).unwrap_or(0);

    let s = s.trim();

    if s.is_empty() {
        return JSValue::float(f64::NAN);
    }

    // Determine effective radix and strip prefix
    let (effective_radix, s) = if radix == 0 {
        if s.starts_with("0x") || s.starts_with("0X") {
            (16, &s[2..])
        } else {
            (10, s)
        }
    } else if radix < 2 || radix > 36 {
        return JSValue::float(f64::NAN)
    } else {
        let s = if radix == 16 && (s.starts_with("0x") || s.starts_with("0X")) {
            &s[2..]
        } else {
            s
        };
        (radix as u32, s)
    };

    // Handle sign
    let (s, negative) = if s.starts_with('-') {
        (&s[1..], true)
    } else if s.starts_with('+') {
        (&s[1..], false)
    } else {
        (s, false)
    };

    // Skip leading zeros
    let s = s.trim_start_matches('0');

    // Parse the number
    let mut result: f64 = 0.0;
    for ch in s.chars() {
        let digit = match ch {
            '0'..='9' => ch as u32 - '0' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 10,
            'A'..='Z' => ch as u32 - 'A' as u32 + 10,
            _ => break,
        };
        if digit >= effective_radix {
            break;
        }
        result = result * (effective_radix as f64) + (digit as f64);
    }

    if negative {
        result = -result;
    }

    JSValue::float(result)
}

/// Number.parseFloat(string)
pub fn number_parse_float(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let s = s.trim();

    if s.is_empty() {
        return JSValue::float(f64::NAN);
    }

    match s.parse::<f64>() {
        Ok(n) => JSValue::float(n),
        Err(_) => {
            // Try to parse as much as possible
            let mut end = 0;
            let chars: Vec<char> = s.chars().collect();
            let mut has_digit = false;

            for (i, &ch) in chars.iter().enumerate() {
                match ch {
                    '0'..='9' => {
                        has_digit = true;
                        end = i + 1;
                    }
                    '.' if i == 0 || has_digit => {
                        end = i + 1;
                    }
                    'e' | 'E' => {
                        end = i + 1;
                        // Check for optional sign after e
                        if end < chars.len() && (chars[end] == '+' || chars[end] == '-') {
                            end += 1;
                        }
                    }
                    _ => break,
                }
            }

            if end > 0 {
                let num_str: String = chars[..end].iter().collect();
                match num_str.parse::<f64>() {
                    Ok(n) => JSValue::float(n),
                    Err(_) => JSValue::float(f64::NAN),
                }
            } else {
                JSValue::float(f64::NAN)
            }
        }
    }
}

// ============================================================================
// Number prototype methods
// ============================================================================

/// Get the primitive number value from this.
fn get_this_number(this: &JSValue) -> f64 {
    match this {
        JSValue::Int(i) => *i as f64,
        JSValue::Float(f) => *f,
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            if let Some(val) = borrow.internal_slots.get("PrimitiveValue") {
                val.to_number()
            } else {
                f64::NAN
            }
        }
        _ => this.to_number(),
    }
}

/// Number.prototype.toFixed(fractionDigits)
pub fn number_to_fixed(this: &JSValue, args: &[JSValue]) -> JSValue {
    let n = get_this_number(this);
    let digits = args.get(0).map(|v| v.to_int32()).unwrap_or(0).max(0).min(100) as usize;

    if n.is_nan() {
        return JSValue::string("NaN");
    }
    if n.is_infinite() {
        return JSValue::string(if n > 0.0 { "Infinity" } else { "-Infinity" });
    }

    // Use Rust's formatting with fixed precision
    let formatted = format!("{:.*}", digits, n);

    // Ensure there's a decimal point for small digit counts (JS semantics)
    if digits == 0 && !formatted.contains('.') {
        return JSValue::string(&formatted);
    }

    JSValue::string(&formatted)
}

/// Number.prototype.toExponential(fractionDigits)
pub fn number_to_exponential(this: &JSValue, args: &[JSValue]) -> JSValue {
    let n = get_this_number(this);
    let digits = args.get(0).map(|v| v.to_int32()).unwrap_or(0).max(0).min(100) as usize;

    if n.is_nan() {
        return JSValue::string("NaN");
    }
    if n.is_infinite() {
        return JSValue::string(if n > 0.0 { "Infinity" } else { "-Infinity" });
    }

    let formatted = format!("{:.*e}", digits, n);
    JSValue::string(&formatted)
}

/// Number.prototype.toPrecision(precision)
pub fn number_to_precision(this: &JSValue, args: &[JSValue]) -> JSValue {
    let n = get_this_number(this);

    match args.get(0) {
        Some(precision_arg) => {
            let p = precision_arg.to_int32() as usize;
            if p < 1 || p > 100 {
                return JSValue::undefined();
            }

            if n.is_nan() {
                return JSValue::string("NaN");
            }
            if n.is_infinite() {
                return JSValue::string(if n > 0.0 { "Infinity" } else { "-Infinity" });
            }

            let formatted = format!("{:.*}", p - 1, n);
            JSValue::string(&formatted)
        }
        None => {
            // No precision: use to string (either fixed or exponential)
            if n.is_nan() {
                return JSValue::string("NaN");
            }
            if n.is_infinite() {
                return JSValue::string(if n > 0.0 { "Infinity" } else { "-Infinity" });
            }
            let formatted = format!("{}", n);
            JSValue::string(&formatted)
        }
    }
}

/// Number.prototype.toString(radix)
pub fn number_to_string(this: &JSValue, args: &[JSValue]) -> JSValue {
    let n = get_this_number(this);
    let radix = args.get(0).map(|v| v.to_int32()).unwrap_or(10);

    if n.is_nan() {
        return JSValue::string("NaN");
    }
    if n.is_infinite() {
        return JSValue::string(if n > 0.0 { "Infinity" } else { "-Infinity" });
    }

    if radix == 10 {
        let formatted = format_number(n);
        return JSValue::string(&formatted);
    }

    // For non-10 radix, use integer conversion
    let int_val = n as i64;
    let formatted = match radix {
        2 => format!("{:b}", int_val),
        8 => format!("{:o}", int_val),
        16 => format!("{:x}", int_val),
        _ => {
            // General radix conversion
            if int_val == 0 {
                return JSValue::string("0");
            }
            let negative = int_val < 0;
            let mut val = int_val.unsigned_abs();
            let base = radix as u64;
            let mut digits = Vec::new();
            while val > 0 {
                let digit = (val % base) as u8;
                let ch = if digit < 10 {
                    (b'0' + digit) as char
                } else {
                    (b'a' + digit - 10) as char
                };
                digits.push(ch);
                val /= base;
            }
            digits.reverse();
            let s: String = digits.iter().collect();
            if negative {
                format!("-{}", s)
            } else {
                s
            }
        }
    };
    JSValue::string(&formatted)
}

/// Number.prototype.valueOf()
pub fn number_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::float(get_this_number(this))
}

/// Helper to format a number like JavaScript does
fn format_number(n: f64) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let s = format!("{}", n);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Number constructor and prototype.
pub fn init_number(ctx: &mut JSContext) {
    // Create the Number constructor function
    let constructor = JSValue::function(
        Some("Number"),
        vec!["value".to_string()],
        FunctionBody::Native(number_constructor),
    );

    // Create Number.prototype
    let prototype = JSValue::object("Number");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("toFixed", number_to_fixed),
        ("toExponential", number_to_exponential),
        ("toPrecision", number_to_precision),
        ("toString", number_to_string),
        ("valueOf", number_value_of),
    ];

    for &(name, func) in methods {
        prototype.set_property(name, JSValue::function(
            Some(name),
            vec![],
            FunctionBody::Native(func),
        ));
    }

    // Set Number.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Add static methods to constructor
    constructor.set_property("isNaN", JSValue::function(
        Some("isNaN"),
        vec!["value".to_string()],
        FunctionBody::Native(number_is_nan),
    ));
    constructor.set_property("isFinite", JSValue::function(
        Some("isFinite"),
        vec!["value".to_string()],
        FunctionBody::Native(number_is_finite),
    ));
    constructor.set_property("isInteger", JSValue::function(
        Some("isInteger"),
        vec!["value".to_string()],
        FunctionBody::Native(number_is_integer),
    ));
    constructor.set_property("isSafeInteger", JSValue::function(
        Some("isSafeInteger"),
        vec!["value".to_string()],
        FunctionBody::Native(number_is_safe_integer),
    ));
    constructor.set_property("parseInt", JSValue::function(
        Some("parseInt"),
        vec!["string".to_string(), "radix".to_string()],
        FunctionBody::Native(number_parse_int),
    ));
    constructor.set_property("parseFloat", JSValue::function(
        Some("parseFloat"),
        vec!["string".to_string()],
        FunctionBody::Native(number_parse_float),
    ));

    // Add constants
    constructor.set_property("NaN", JSValue::float(f64::NAN));
    constructor.set_property("POSITIVE_INFINITY", JSValue::float(f64::INFINITY));
    constructor.set_property("NEGATIVE_INFINITY", JSValue::float(f64::NEG_INFINITY));
    constructor.set_property("MAX_VALUE", JSValue::float(f64::MAX));
    constructor.set_property("MIN_VALUE", JSValue::float(f64::MIN_POSITIVE));
    constructor.set_property("MAX_SAFE_INTEGER", JSValue::float(9007199254740991.0_f64));
    constructor.set_property("MIN_SAFE_INTEGER", JSValue::float(-9007199254740991.0_f64));
    constructor.set_property("EPSILON", JSValue::float(f64::EPSILON));

    // Set Number on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("Number".to_string(), constructor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_number_is_nan() {
        let this = JSValue::undefined();
        assert!(number_is_nan(&this, &[JSValue::float(f64::NAN)]).to_boolean());
        assert!(!number_is_nan(&this, &[JSValue::int(1)]).to_boolean());
        assert!(!number_is_nan(&this, &[JSValue::string("NaN")]).to_boolean());
    }

    #[test]
    fn test_number_is_finite() {
        let this = JSValue::undefined();
        assert!(number_is_finite(&this, &[JSValue::int(1)]).to_boolean());
        assert!(!number_is_finite(&this, &[JSValue::float(f64::INFINITY)]).to_boolean());
        assert!(!number_is_finite(&this, &[JSValue::float(f64::NAN)]).to_boolean());
    }

    #[test]
    fn test_number_is_integer() {
        let this = JSValue::undefined();
        assert!(number_is_integer(&this, &[JSValue::int(1)]).to_boolean());
        assert!(!number_is_integer(&this, &[JSValue::float(1.5)]).to_boolean());
        assert!(!number_is_integer(&this, &[JSValue::float(f64::NAN)]).to_boolean());
    }

    #[test]
    fn test_to_fixed() {
        let this = JSValue::float(1.234);
        let result = number_to_fixed(&this, &[JSValue::int(2)]);
        assert_eq!(result.to_string(), "1.23");

        let this = JSValue::float(1.0);
        let result = number_to_fixed(&this, &[JSValue::int(0)]);
        assert_eq!(result.to_string(), "1");
    }

    #[test]
    fn test_to_exponential() {
        let this = JSValue::float(12345.0);
        let result = number_to_exponential(&this, &[JSValue::int(2)]);
        assert_eq!(result.to_string(), "1.23e4");
    }

    #[test]
    fn test_parse_int() {
        let this = JSValue::undefined();
        let result = number_parse_int(&this, &[JSValue::string("42")]);
        assert_eq!(result.to_number(), 42.0);

        let result = number_parse_int(&this, &[JSValue::string("0xFF"), JSValue::int(16)]);
        assert_eq!(result.to_number(), 255.0);
    }

    #[test]
    fn test_parse_float() {
        let this = JSValue::undefined();
        let result = number_parse_float(&this, &[JSValue::string("3.14")]);
        assert!((result.to_number() - 3.14).abs() < 1e-10);

        let result = number_parse_float(&this, &[JSValue::string("  42  ")]);
        assert_eq!(result.to_number(), 42.0);
    }
}
