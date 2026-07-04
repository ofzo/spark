#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Math built-in.
//!
//! Implements the JavaScript Math object.

use crate::value::{JSValue, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Math static methods
// ============================================================================

/// Math.abs(x)
pub fn math_abs(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.abs())
}

/// Math.ceil(x)
pub fn math_ceil(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.ceil())
}

/// Math.floor(x)
pub fn math_floor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.floor())
}

/// Math.round(x)
pub fn math_round(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.round())
}

/// Math.trunc(x)
pub fn math_trunc(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.trunc())
}

/// Math.sign(x)
pub fn math_sign(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    if x.is_nan() {
        JSValue::float(f64::NAN)
    } else if x > 0.0 {
        JSValue::float(1.0)
    } else if x < 0.0 {
        JSValue::float(-1.0)
    } else {
        // +0, -0
        JSValue::float(x)
    }
}

/// Math.max(value1, value2, ...)
pub fn math_max(_this: &JSValue, args: &[JSValue]) -> JSValue {
    if args.is_empty() {
        return JSValue::float(f64::NEG_INFINITY);
    }
    let mut result = f64::NEG_INFINITY;
    for arg in args {
        let n = arg.to_number();
        if n.is_nan() {
            return JSValue::float(f64::NAN);
        }
        if n > result {
            result = n;
        }
    }
    JSValue::float(result)
}

/// Math.min(value1, value2, ...)
pub fn math_min(_this: &JSValue, args: &[JSValue]) -> JSValue {
    if args.is_empty() {
        return JSValue::float(f64::INFINITY);
    }
    let mut result = f64::INFINITY;
    for arg in args {
        let n = arg.to_number();
        if n.is_nan() {
            return JSValue::float(f64::NAN);
        }
        if n < result {
            result = n;
        }
    }
    JSValue::float(result)
}

/// Math.random() - Returns a pseudo-random number between 0 (inclusive) and 1 (exclusive).
pub fn math_random(_this: &JSValue, _args: &[JSValue]) -> JSValue {
    // Simple LCG pseudo-random (not cryptographically secure)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    // Use host clock if available, otherwise fall back to SystemTime
    let nanos = if let Some(clock) = crate::interpreter::get_clock() {
        (clock.now_ms() * 1000.0) as u32
    } else {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    };
    nanos.hash(&mut hasher);
    let hash = hasher.finish();
    JSValue::float((hash as f64) / (u64::MAX as f64))
}

/// Math.sin(x)
pub fn math_sin(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.sin())
}

/// Math.cos(x)
pub fn math_cos(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.cos())
}

/// Math.tan(x)
pub fn math_tan(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.tan())
}

/// Math.asin(x)
pub fn math_asin(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.asin())
}

/// Math.acos(x)
pub fn math_acos(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.acos())
}

/// Math.atan(x)
pub fn math_atan(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.atan())
}

/// Math.atan2(y, x)
pub fn math_atan2(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let y = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    let x = args.get(1).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(y.atan2(x))
}

/// Math.sqrt(x)
pub fn math_sqrt(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.sqrt())
}

/// Math.cbrt(x)
pub fn math_cbrt(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.cbrt())
}

/// Math.log(x) - Natural logarithm (ln)
pub fn math_log(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.ln())
}

/// Math.log10(x)
pub fn math_log10(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.log10())
}

/// Math.log2(x)
pub fn math_log2(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.log2())
}

/// Math.exp(x)
pub fn math_exp(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.exp())
}

/// Math.expm1(x)
pub fn math_expm1(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(x.exp_m1())
}

/// Math.pow(base, exponent)
pub fn math_pow(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let base = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    let exp = args.get(1).map(|v| v.to_number()).unwrap_or(f64::NAN);
    JSValue::float(base.powf(exp))
}

/// Math.hypot(value1, value2, ...)
pub fn math_hypot(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let mut sum = 0.0_f64;
    for arg in args {
        let n = arg.to_number();
        if n.is_infinite() {
            return JSValue::float(f64::INFINITY);
        }
        if n.is_nan() {
            return JSValue::float(f64::NAN);
        }
        sum += n * n;
    }
    JSValue::float(sum.sqrt())
}

/// Math.clz32(x)
pub fn math_clz32(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_uint32()).unwrap_or(0);
    JSValue::int(x.leading_zeros() as i32)
}

/// Math.imul(x, y)
pub fn math_imul(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let x = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
    let y = args.get(1).map(|v| v.to_int32()).unwrap_or(0);
    JSValue::int(x.wrapping_mul(y))
}

// ============================================================================
// Math constants
// ============================================================================

fn create_math_object() -> JSValue {
    let math = JSValue::object("Math");

    // Add methods
    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("abs", math_abs),
        ("ceil", math_ceil),
        ("floor", math_floor),
        ("round", math_round),
        ("trunc", math_trunc),
        ("sign", math_sign),
        ("max", math_max),
        ("min", math_min),
        ("random", math_random),
        ("sin", math_sin),
        ("cos", math_cos),
        ("tan", math_tan),
        ("asin", math_asin),
        ("acos", math_acos),
        ("atan", math_atan),
        ("atan2", math_atan2),
        ("sqrt", math_sqrt),
        ("cbrt", math_cbrt),
        ("log", math_log),
        ("log10", math_log10),
        ("log2", math_log2),
        ("exp", math_exp),
        ("expm1", math_expm1),
        ("pow", math_pow),
        ("hypot", math_hypot),
        ("clz32", math_clz32),
        ("imul", math_imul),
    ];

    for &(name, func) in methods {
        let f = JSValue::function(
            Some(name),
            vec![],
            FunctionBody::Native(func),
        );
        math.set_property(name, f);
    }

    // Add constants
    math.set_property("E", JSValue::float(std::f64::consts::E));
    math.set_property("LN10", JSValue::float(std::f64::consts::LN_10));
    math.set_property("LN2", JSValue::float(std::f64::consts::LN_2));
    math.set_property("LOG2E", JSValue::float(std::f64::consts::LOG2_E));
    math.set_property("LOG10E", JSValue::float(std::f64::consts::LOG10_E));
    math.set_property("PI", JSValue::float(std::f64::consts::PI));
    math.set_property("SQRT1_2", JSValue::float(std::f64::consts::FRAC_1_SQRT_2));
    math.set_property("SQRT2", JSValue::float(std::f64::consts::SQRT_2));

    math
}

/// Initialize the Math object.
pub fn init_math(ctx: &mut JSContext) {
    let math = create_math_object();
    ctx.global
        .borrow_mut()
        .properties
        .insert("Math".to_string(), math);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    fn make_ctx() -> JSContext {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        JSContext::new(rt)
    }

    #[test]
    fn test_math_abs() {
        let this = JSValue::undefined();
        assert_eq!(math_abs(&this, &[JSValue::int(-5)]).to_number(), 5.0);
        assert_eq!(math_abs(&this, &[JSValue::int(5)]).to_number(), 5.0);
        assert_eq!(math_abs(&this, &[JSValue::float(-3.14)]).to_number(), 3.14);
    }

    #[test]
    fn test_math_floor() {
        let this = JSValue::undefined();
        assert_eq!(math_floor(&this, &[JSValue::float(4.7)]).to_number(), 4.0);
        assert_eq!(math_floor(&this, &[JSValue::float(-4.7)]).to_number(), -5.0);
    }

    #[test]
    fn test_math_ceil() {
        let this = JSValue::undefined();
        assert_eq!(math_ceil(&this, &[JSValue::float(4.2)]).to_number(), 5.0);
        assert_eq!(math_ceil(&this, &[JSValue::float(-4.2)]).to_number(), -4.0);
    }

    #[test]
    fn test_math_round() {
        let this = JSValue::undefined();
        assert_eq!(math_round(&this, &[JSValue::float(4.5)]).to_number(), 5.0);
        assert_eq!(math_round(&this, &[JSValue::float(4.4)]).to_number(), 4.0);
    }

    #[test]
    fn test_math_max_min() {
        let this = JSValue::undefined();
        assert_eq!(math_max(&this, &[JSValue::int(1), JSValue::int(2), JSValue::int(3)]).to_number(), 3.0);
        assert_eq!(math_min(&this, &[JSValue::int(1), JSValue::int(2), JSValue::int(3)]).to_number(), 1.0);
        assert_eq!(math_max(&this, &[]).to_number(), f64::NEG_INFINITY);
        assert_eq!(math_min(&this, &[]).to_number(), f64::INFINITY);
    }

    #[test]
    fn test_math_sqrt_pow() {
        let this = JSValue::undefined();
        assert_eq!(math_sqrt(&this, &[JSValue::float(9.0)]).to_number(), 3.0);
        assert_eq!(math_pow(&this, &[JSValue::int(2), JSValue::int(10)]).to_number(), 1024.0);
    }

    #[test]
    fn test_math_trig() {
        let this = JSValue::undefined();
        let sin_zero = math_sin(&this, &[JSValue::float(0.0)]).to_number();
        assert!((sin_zero - 0.0).abs() < 1e-10);
        let cos_zero = math_cos(&this, &[JSValue::float(0.0)]).to_number();
        assert!((cos_zero - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_math_log_exp() {
        let this = JSValue::undefined();
        let log1 = math_log(&this, &[JSValue::float(1.0)]).to_number();
        assert!((log1 - 0.0).abs() < 1e-10);
        let exp0 = math_exp(&this, &[JSValue::float(0.0)]).to_number();
        assert!((exp0 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_init_math() {
        let mut ctx = make_ctx();
        init_math(&mut ctx);
        let math = ctx.global.borrow().properties.get("Math").cloned();
        assert!(math.is_some());
        let math = math.unwrap();
        assert!(math.get_property("PI").is_some());
        assert!(math.get_property("abs").is_some());
    }
}
