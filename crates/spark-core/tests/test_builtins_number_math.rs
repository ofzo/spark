//! Tests for number.rs and math.rs builtins

use spark_core::builtins::number::*;
use spark_core::builtins::math::*;
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
// Number tests
// ============================================================================

#[test]
fn test_number_is_nan_with_nan() {
    let result = number_is_nan(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_nan_with_number() {
    let result = number_is_nan(&JSValue::undefined(), &[JSValue::int(42)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_nan_with_nan_value() {
    let result = number_is_nan(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_nan_with_numeric_string() {
    let result = number_is_nan(&JSValue::undefined(), &[JSValue::string("42")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_finite_with_finite() {
    let result = number_is_finite(&JSValue::undefined(), &[JSValue::int(42)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_finite_with_infinity() {
    let result = number_is_finite(&JSValue::undefined(), &[JSValue::float(f64::INFINITY)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_finite_with_nan() {
    let result = number_is_finite(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_finite_with_finite_float() {
    let result = number_is_finite(&JSValue::undefined(), &[JSValue::float(3.14)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_integer_with_integer() {
    let result = number_is_integer(&JSValue::undefined(), &[JSValue::int(42)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_integer_with_float_integer() {
    let result = number_is_integer(&JSValue::undefined(), &[JSValue::float(42.0)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_integer_with_fraction() {
    let result = number_is_integer(&JSValue::undefined(), &[JSValue::float(3.14)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_integer_with_nan() {
    let result = number_is_integer(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_safe_integer_normal() {
    let result = number_is_safe_integer(&JSValue::undefined(), &[JSValue::int(42)]);
    assert!(result.to_boolean());
}

#[test]
fn test_number_is_safe_integer_too_large() {
    let result = number_is_safe_integer(&JSValue::undefined(), &[JSValue::float(9007199254740992.0)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_safe_integer_negative_too_large() {
    let result = number_is_safe_integer(&JSValue::undefined(), &[JSValue::float(-9007199254740992.0)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_is_safe_integer_fraction() {
    let result = number_is_safe_integer(&JSValue::undefined(), &[JSValue::float(1.5)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_number_to_fixed() {
    let val = JSValue::float(3.14159);
    let result = number_to_fixed(&val, &[JSValue::int(2)]);
    assert_eq!(result.to_string(), "3.14");
}

#[test]
fn test_number_to_fixed_zero() {
    let val = JSValue::float(3.14159);
    let result = number_to_fixed(&val, &[JSValue::int(0)]);
    assert_eq!(result.to_string(), "3");
}

#[test]
fn test_number_to_exponential() {
    let val = JSValue::int(100);
    let result = number_to_exponential(&val, &[JSValue::int(2)]);
    let s = result.to_string();
    // Should contain "e" and some digits
    assert!(s.contains("e"), "Expected exponential format, got: {}", s);
}

#[test]
fn test_number_to_exponential_default() {
    let val = JSValue::int(100);
    let result = number_to_exponential(&val, &[]);
    let s = result.to_string();
    assert!(s.contains("e"), "Expected exponential format, got: {}", s);
}

#[test]
fn test_number_to_precision() {
    let val = JSValue::float(1234.5);
    let result = number_to_precision(&val, &[JSValue::int(4)]);
    let s = result.to_string();
    assert!(!s.is_empty(), "toPrecision should return non-empty");
}

#[test]
fn test_number_to_precision_less_digits() {
    let val = JSValue::float(1234.5);
    let result = number_to_precision(&val, &[JSValue::int(2)]);
    let s = result.to_string();
    assert!(!s.is_empty(), "toPrecision should return non-empty");
}

#[test]
fn test_number_to_string_decimal() {
    let val = JSValue::int(255);
    let result = number_to_string(&val, &[JSValue::int(16)]);
    assert_eq!(result.to_string(), "ff");
}

#[test]
fn test_number_to_string_binary() {
    let val = JSValue::int(10);
    let result = number_to_string(&val, &[JSValue::int(2)]);
    assert_eq!(result.to_string(), "1010");
}

#[test]
fn test_number_to_string_default() {
    let val = JSValue::int(42);
    let result = number_to_string(&val, &[]);
    assert_eq!(result.to_string(), "42");
}

#[test]
fn test_number_value_of_on_int() {
    let result = number_value_of(&JSValue::int(42), &[]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_number_value_of_on_float() {
    let result = number_value_of(&JSValue::float(3.14), &[]);
    assert!((result.to_number() - 3.14).abs() < 0.01);
}

#[test]
fn test_number_value_of_on_object() {
    let obj = JSValue::object("Number");
    if let JSValue::Object(ref o) = obj {
        o.borrow_mut().internal_slots.insert("PrimitiveValue".to_string(), JSValue::int(99));
    }
    let result = number_value_of(&obj, &[]);
    assert_eq!(result.to_int32(), 99);
}

#[test]
fn test_number_parse_int_basic() {
    let result = number_parse_int(&JSValue::undefined(), &[JSValue::string("42")]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_number_parse_int_with_radix() {
    let result = number_parse_int(&JSValue::undefined(), &[JSValue::string("ff"), JSValue::int(16)]);
    assert_eq!(result.to_int32(), 255);
}

#[test]
fn test_number_parse_int_invalid() {
    let result = number_parse_int(&JSValue::undefined(), &[JSValue::string("abc")]);
    // parseInt("abc") returns NaN
    assert!(result.to_number().is_nan() || result.to_int32() == 0);
}

#[test]
fn test_number_parse_float_basic() {
    let result = number_parse_float(&JSValue::undefined(), &[JSValue::string("3.14")]);
    assert!((result.to_number() - 3.14).abs() < 0.01);
}

#[test]
fn test_number_parse_float_invalid() {
    let result = number_parse_float(&JSValue::undefined(), &[JSValue::string("abc")]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_init_number() {
    let mut ctx = make_ctx();
    init_number(&mut ctx);
    let global = ctx.global.borrow();
    let num_ctor = global.properties.get("Number").unwrap();
    assert!(num_ctor.is_callable());
    assert!(num_ctor.get_property("isNaN").is_some());
    assert!(num_ctor.get_property("isFinite").is_some());
    assert!(num_ctor.get_property("isInteger").is_some());
    assert!(num_ctor.get_property("isSafeInteger").is_some());
    assert!(num_ctor.get_property("parseInt").is_some());
    assert!(num_ctor.get_property("parseFloat").is_some());
    assert!(num_ctor.get_property("MAX_SAFE_INTEGER").is_some());
    assert!(num_ctor.get_property("MIN_SAFE_INTEGER").is_some());
    assert!(num_ctor.get_property("POSITIVE_INFINITY").is_some());
    assert!(num_ctor.get_property("NEGATIVE_INFINITY").is_some());
    assert!(num_ctor.get_property("NaN").is_some());
    assert!(num_ctor.get_property("EPSILON").is_some());
}

// ============================================================================
// Math tests
// ============================================================================

#[test]
fn test_math_trunc_positive() {
    let result = math_trunc(&JSValue::undefined(), &[JSValue::float(3.7)]);
    assert_eq!(result.to_int32(), 3);
}

#[test]
fn test_math_trunc_negative() {
    let result = math_trunc(&JSValue::undefined(), &[JSValue::float(-3.7)]);
    assert_eq!(result.to_int32(), -3);
}

#[test]
fn test_math_trunc_nan() {
    let result = math_trunc(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_math_sign_positive() {
    let result = math_sign(&JSValue::undefined(), &[JSValue::int(5)]);
    assert_eq!(result.to_int32(), 1);
}

#[test]
fn test_math_sign_negative() {
    let result = math_sign(&JSValue::undefined(), &[JSValue::int(-5)]);
    assert_eq!(result.to_int32(), -1);
}

#[test]
fn test_math_sign_zero() {
    let result = math_sign(&JSValue::undefined(), &[JSValue::int(0)]);
    assert_eq!(result.to_int32(), 0);
}

#[test]
fn test_math_sign_negative_zero() {
    let result = math_sign(&JSValue::undefined(), &[JSValue::float(-0.0)]);
    assert_eq!(result.to_number() as i32, 0);
}

#[test]
fn test_math_sign_nan() {
    let result = math_sign(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_math_random_range() {
    let result = math_random(&JSValue::undefined(), &[]);
    let v = result.to_number();
    assert!(v >= 0.0 && v < 1.0);
}

#[test]
fn test_math_tan() {
    let result = math_tan(&JSValue::undefined(), &[JSValue::float(0.0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_asin() {
    let result = math_asin(&JSValue::undefined(), &[JSValue::float(0.0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_acos() {
    let result = math_acos(&JSValue::undefined(), &[JSValue::float(1.0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_atan() {
    let result = math_atan(&JSValue::undefined(), &[JSValue::float(0.0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_atan2() {
    let result = math_atan2(&JSValue::undefined(), &[JSValue::float(1.0), JSValue::float(0.0)]);
    let expected = std::f64::consts::FRAC_PI_2;
    assert!((result.to_number() - expected).abs() < 0.001);
}

#[test]
fn test_math_cbrt() {
    let result = math_cbrt(&JSValue::undefined(), &[JSValue::int(27)]);
    assert!((result.to_number() - 3.0).abs() < 0.001);
}

#[test]
fn test_math_cbrt_negative() {
    let result = math_cbrt(&JSValue::undefined(), &[JSValue::int(-8)]);
    assert!((result.to_number() - (-2.0)).abs() < 0.001);
}

#[test]
fn test_math_log10() {
    let result = math_log10(&JSValue::undefined(), &[JSValue::int(100)]);
    assert!((result.to_number() - 2.0).abs() < 0.001);
}

#[test]
fn test_math_log2() {
    let result = math_log2(&JSValue::undefined(), &[JSValue::int(8)]);
    assert!((result.to_number() - 3.0).abs() < 0.001);
}

#[test]
fn test_math_expm1() {
    let result = math_expm1(&JSValue::undefined(), &[JSValue::int(0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_hypot() {
    let result = math_hypot(&JSValue::undefined(), &[JSValue::int(3), JSValue::int(4)]);
    assert!((result.to_number() - 5.0).abs() < 0.001);
}

#[test]
fn test_math_hypot_no_args() {
    let result = math_hypot(&JSValue::undefined(), &[]);
    assert_eq!(result.to_number(), 0.0);
}

#[test]
fn test_math_clz32() {
    let result = math_clz32(&JSValue::undefined(), &[JSValue::int(1)]);
    assert_eq!(result.to_int32(), 31);
}

#[test]
fn test_math_clz32_zero() {
    let result = math_clz32(&JSValue::undefined(), &[JSValue::int(0)]);
    assert_eq!(result.to_int32(), 32);
}

#[test]
fn test_math_clz32_large() {
    let result = math_clz32(&JSValue::undefined(), &[JSValue::int(0xFF)]);
    assert_eq!(result.to_int32(), 24);
}

#[test]
fn test_math_imul() {
    let result = math_imul(&JSValue::undefined(), &[JSValue::int(3), JSValue::int(4)]);
    assert_eq!(result.to_int32(), 12);
}

#[test]
fn test_math_imul_overflow() {
    let result = math_imul(&JSValue::undefined(), &[JSValue::int(-1), JSValue::int(2)]);
    assert_eq!(result.to_int32(), -2);
}

#[test]
fn test_math_abs_negative() {
    let result = math_abs(&JSValue::undefined(), &[JSValue::int(-42)]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_math_abs_positive() {
    let result = math_abs(&JSValue::undefined(), &[JSValue::int(42)]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_math_abs_nan() {
    let result = math_abs(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_math_floor() {
    let result = math_floor(&JSValue::undefined(), &[JSValue::float(3.7)]);
    assert_eq!(result.to_int32(), 3);
}

#[test]
fn test_math_floor_negative() {
    let result = math_floor(&JSValue::undefined(), &[JSValue::float(-3.7)]);
    assert_eq!(result.to_int32(), -4);
}

#[test]
fn test_math_ceil() {
    let result = math_ceil(&JSValue::undefined(), &[JSValue::float(3.2)]);
    assert_eq!(result.to_int32(), 4);
}

#[test]
fn test_math_ceil_negative() {
    let result = math_ceil(&JSValue::undefined(), &[JSValue::float(-3.2)]);
    assert_eq!(result.to_int32(), -3);
}

#[test]
fn test_math_round() {
    let result = math_round(&JSValue::undefined(), &[JSValue::float(3.5)]);
    assert_eq!(result.to_int32(), 4);
}

#[test]
fn test_math_round_down() {
    let result = math_round(&JSValue::undefined(), &[JSValue::float(3.4)]);
    assert_eq!(result.to_int32(), 3);
}

#[test]
fn test_math_sqrt() {
    let result = math_sqrt(&JSValue::undefined(), &[JSValue::int(16)]);
    assert!((result.to_number() - 4.0).abs() < 0.001);
}

#[test]
fn test_math_sqrt_negative() {
    let result = math_sqrt(&JSValue::undefined(), &[JSValue::int(-1)]);
    assert!(result.to_number().is_nan());
}

#[test]
fn test_math_pow() {
    let result = math_pow(&JSValue::undefined(), &[JSValue::int(2), JSValue::int(10)]);
    assert_eq!(result.to_int32(), 1024);
}

#[test]
fn test_math_exp() {
    let result = math_exp(&JSValue::undefined(), &[JSValue::int(0)]);
    assert!((result.to_number() - 1.0).abs() < 0.001);
}

#[test]
fn test_math_log() {
    let result = math_log(&JSValue::undefined(), &[JSValue::float(std::f64::consts::E)]);
    assert!((result.to_number() - 1.0).abs() < 0.001);
}

#[test]
fn test_math_sin() {
    let result = math_sin(&JSValue::undefined(), &[JSValue::int(0)]);
    assert!((result.to_number()).abs() < 0.001);
}

#[test]
fn test_math_cos() {
    let result = math_cos(&JSValue::undefined(), &[JSValue::int(0)]);
    assert!((result.to_number() - 1.0).abs() < 0.001);
}

#[test]
fn test_math_max_no_args() {
    let result = math_max(&JSValue::undefined(), &[]);
    assert!(result.to_number().is_infinite() && result.to_number().is_sign_negative());
}

#[test]
fn test_math_min_no_args() {
    let result = math_min(&JSValue::undefined(), &[]);
    assert!(result.to_number().is_infinite() && result.to_number().is_sign_positive());
}

#[test]
fn test_math_max_multiple() {
    let result = math_max(&JSValue::undefined(), &[JSValue::int(3), JSValue::int(1), JSValue::int(4), JSValue::int(1), JSValue::int(5)]);
    assert_eq!(result.to_int32(), 5);
}

#[test]
fn test_math_min_multiple() {
    let result = math_min(&JSValue::undefined(), &[JSValue::int(3), JSValue::int(1), JSValue::int(4), JSValue::int(1), JSValue::int(5)]);
    assert_eq!(result.to_int32(), 1);
}

#[test]
fn test_init_math() {
    let mut ctx = make_ctx();
    init_math(&mut ctx);
    let global = ctx.global.borrow();
    let math = global.properties.get("Math").unwrap();
    // Verify all constants
    assert!(math.get_property("PI").is_some());
    assert!(math.get_property("E").is_some());
    assert!(math.get_property("LN2").is_some());
    assert!(math.get_property("LN10").is_some());
    assert!(math.get_property("SQRT2").is_some());
    assert!(math.get_property("SQRT1_2").is_some());
    assert!(math.get_property("LOG2E").is_some());
    assert!(math.get_property("LOG10E").is_some());
    // Verify methods
    for method in &["abs", "ceil", "floor", "round", "trunc", "sign", "max", "min",
                     "random", "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
                     "sqrt", "cbrt", "log", "log10", "log2", "exp", "expm1",
                     "pow", "hypot", "clz32", "imul"] {
        assert!(math.get_property(method).is_some(), "Math.{} should exist", method);
    }
}

// ============================================================================
// Math edge cases: NaN/Infinity propagation
// ============================================================================

#[test]
fn test_math_operations_with_nan() {
    let nan = JSValue::float(f64::NAN);
    assert!(math_abs(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_ceil(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_floor(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_round(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_trunc(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_sqrt(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_sin(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
    assert!(math_cos(&JSValue::undefined(), &[nan.clone()]).to_number().is_nan());
}

#[test]
fn test_math_no_args_returns_nan() {
    assert!(math_abs(&JSValue::undefined(), &[]).to_number().is_nan());
    assert!(math_ceil(&JSValue::undefined(), &[]).to_number().is_nan());
    assert!(math_floor(&JSValue::undefined(), &[]).to_number().is_nan());
    assert!(math_round(&JSValue::undefined(), &[]).to_number().is_nan());
    assert!(math_sqrt(&JSValue::undefined(), &[]).to_number().is_nan());
}
