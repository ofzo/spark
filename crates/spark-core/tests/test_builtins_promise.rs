//! Tests for promise.rs builtin - covering gaps in existing tests

use spark_core::builtins::promise::*;
use spark_core::context::JSContext;
use spark_core::runtime::JSRuntime;
use spark_core::value::{JSValue, FunctionBody};
use std::cell::RefCell;
use std::rc::Rc;

fn make_ctx() -> JSContext {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    JSContext::new(rt)
}

// ============================================================================
// Promise constructor with executor
// ============================================================================

#[test]
fn test_promise_constructor_with_executor_resolve() {
    let this = JSValue::undefined();
    let executor = JSValue::function(
        Some("executor"),
        vec!["resolve".to_string(), "reject".to_string()],
        FunctionBody::Native(|_this, args| {
            let resolve = args.get(0).unwrap();
            // Call resolve with a value
            match resolve {
                JSValue::Function(f) => {
                    let borrow = f.borrow();
                    if let FunctionBody::Closure(closure_fn) = &borrow.body {
                        closure_fn(&JSValue::undefined(), &[JSValue::int(42)]);
                    }
                }
                _ => {}
            }
            JSValue::undefined()
        }),
    );
    let p = promise_constructor(&this, &[executor]);
    let state = p.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
    assert_eq!(p.get_property("__result").unwrap().to_int32(), 42);
}

#[test]
fn test_promise_constructor_with_executor_reject() {
    let this = JSValue::undefined();
    let executor = JSValue::function(
        Some("executor"),
        vec!["resolve".to_string(), "reject".to_string()],
        FunctionBody::Native(|_this, args| {
            let reject = args.get(1).unwrap();
            match reject {
                JSValue::Function(f) => {
                    let borrow = f.borrow();
                    if let FunctionBody::Closure(closure_fn) = &borrow.body {
                        closure_fn(&JSValue::undefined(), &[JSValue::string("error")]);
                    }
                }
                _ => {}
            }
            JSValue::undefined()
        }),
    );
    let p = promise_constructor(&this, &[executor]);
    let state = p.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(2)); // REJECTED
    assert_eq!(p.get_property("__result").unwrap().to_string(), "error");
}

#[test]
fn test_promise_constructor_with_non_callable() {
    let this = JSValue::undefined();
    let p = promise_constructor(&this, &[JSValue::int(42)]);
    // Should still create a pending promise
    let state = p.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(0)); // PENDING
}

#[test]
fn test_promise_constructor_no_args() {
    let this = JSValue::undefined();
    let p = promise_constructor(&this, &[]);
    let state = p.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(0)); // PENDING
}

// ============================================================================
// Promise.prototype.finally
// ============================================================================

#[test]
fn test_promise_finally_on_fulfilled() {
    let this = JSValue::undefined();
    let p = promise_resolve(&this, &[JSValue::int(42)]);

    let on_finally = JSValue::function(
        Some("cleanup"),
        vec![],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );

    let result = promise_finally(&p, &[on_finally]);
    // Should return a new promise with the same value
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
    assert_eq!(result.get_property("__result").unwrap().to_int32(), 42);
}

#[test]
fn test_promise_finally_on_rejected() {
    let this = JSValue::undefined();
    let p = promise_reject(&this, &[JSValue::string("error")]);

    let on_finally = JSValue::function(
        Some("cleanup"),
        vec![],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );

    let result = promise_finally(&p, &[on_finally]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(2)); // REJECTED
    assert_eq!(result.get_property("__result").unwrap().to_string(), "error");
}

#[test]
fn test_promise_finally_on_pending() {
    let this = JSValue::undefined();
    let p = promise_constructor(&this, &[]);

    let on_finally = JSValue::function(
        Some("cleanup"),
        vec![],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );

    let result = promise_finally(&p, &[on_finally]);
    // Should return the same promise (pending)
    match (&p, &result) {
        (JSValue::Object(a), JSValue::Object(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("Expected same promise"),
    }
}

#[test]
fn test_promise_finally_no_args() {
    let this = JSValue::undefined();
    let p = promise_resolve(&this, &[JSValue::int(1)]);
    let result = promise_finally(&p, &[]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1));
}

// ============================================================================
// Promise.allSettled
// ============================================================================

#[test]
fn test_promise_all_settled_all_fulfilled() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_resolve(&this, &[JSValue::int(1)]));
    arr.set_property("1", promise_resolve(&this, &[JSValue::int(2)]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_all_settled(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
}

#[test]
fn test_promise_all_settled_mixed() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_resolve(&this, &[JSValue::int(1)]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("err")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_all_settled(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    // allSettled always fulfills
    assert_eq!(state, Some(1));
}

#[test]
fn test_promise_all_settled_all_rejected() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_reject(&this, &[JSValue::string("err1")]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("err2")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_all_settled(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // still fulfilled
}

#[test]
fn test_promise_all_settled_empty() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));

    let result = promise_all_settled(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1));
}

// ============================================================================
// Promise.race
// ============================================================================

#[test]
fn test_promise_race_fulfilled_first() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_resolve(&this, &[JSValue::int(1)]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("err")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_race(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
    assert_eq!(result.get_property("__result").unwrap().to_int32(), 1);
}

#[test]
fn test_promise_race_rejected_first() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_reject(&this, &[JSValue::string("fail")]));
    arr.set_property("1", promise_resolve(&this, &[JSValue::int(1)]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_race(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(2)); // REJECTED
    assert_eq!(result.get_property("__result").unwrap().to_string(), "fail");
}

#[test]
fn test_promise_race_all_pending() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_constructor(&this, &[]));
    arr.set_property("1", promise_constructor(&this, &[]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_race(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(0)); // PENDING
}

#[test]
fn test_promise_race_empty() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));

    let result = promise_race(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(0)); // PENDING
}

// ============================================================================
// Promise.any
// ============================================================================

#[test]
fn test_promise_any_first_fulfilled() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_resolve(&this, &[JSValue::int(42)]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("err")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_any(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
    assert_eq!(result.get_property("__result").unwrap().to_int32(), 42);
}

#[test]
fn test_promise_any_all_rejected() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_reject(&this, &[JSValue::string("err1")]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("err2")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_any(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(2)); // REJECTED with AggregateError
    let err = result.get_property("__result").unwrap();
    assert!(err.is_object());
}

#[test]
fn test_promise_any_empty() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));

    let result = promise_any(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    // Empty iterable should reject with AggregateError
    assert_eq!(state, Some(2));
}

#[test]
fn test_promise_any_second_fulfilled() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_reject(&this, &[JSValue::string("err")]));
    arr.set_property("1", promise_resolve(&this, &[JSValue::int(99)]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_any(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
    assert_eq!(result.get_property("__result").unwrap().to_int32(), 99);
}

// ============================================================================
// Promise.all edge cases
// ============================================================================

#[test]
fn test_promise_all_with_rejected() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("0", promise_resolve(&this, &[JSValue::int(1)]));
    arr.set_property("1", promise_reject(&this, &[JSValue::string("fail")]));
    arr.set_property("length", JSValue::int(2));

    let result = promise_all(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(2)); // REJECTED
    assert_eq!(result.get_property("__result").unwrap().to_string(), "fail");
}

#[test]
fn test_promise_all_empty() {
    let this = JSValue::undefined();
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));

    let result = promise_all(&this, &[arr]);
    let state = result.get_property("__state").map(|v| v.to_int32());
    assert_eq!(state, Some(1)); // FULFILLED
}

// ============================================================================
// Promise chaining
// ============================================================================

#[test]
fn test_promise_then_chaining() {
    let this = JSValue::undefined();
    let p = promise_resolve(&this, &[JSValue::int(5)]);

    let double = JSValue::function(
        None,
        vec!["v".to_string()],
        FunctionBody::Native(|_, args| {
            let v = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
            JSValue::Float(v * 2.0)
        }),
    );

    let add_one = JSValue::function(
        None,
        vec!["v".to_string()],
        FunctionBody::Native(|_, args| {
            let v = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
            JSValue::Float(v + 1.0)
        }),
    );

    let p2 = promise_then(&p, &[double, JSValue::undefined()]);
    let p3 = promise_then(&p2, &[add_one, JSValue::undefined()]);

    assert_eq!(p3.get_property("__state").map(|v| v.to_int32()), Some(1));
    assert!((p3.get_property("__result").unwrap().to_number() - 11.0).abs() < 0.01);
}

#[test]
fn test_promise_catch_chaining() {
    let this = JSValue::undefined();
    let p = promise_reject(&this, &[JSValue::string("fail")]);

    let recover = JSValue::function(
        None,
        vec!["err".to_string()],
        FunctionBody::Native(|_, _| JSValue::string("recovered")),
    );

    let p2 = promise_catch(&p, &[recover]);
    assert_eq!(p2.get_property("__state").map(|v| v.to_int32()), Some(1));
    assert_eq!(p2.get_property("__result").unwrap().to_string(), "recovered");
}

#[test]
fn test_promise_then_no_handler_fulfilled() {
    let this = JSValue::undefined();
    let p = promise_resolve(&this, &[JSValue::int(42)]);
    let p2 = promise_then(&p, &[JSValue::undefined(), JSValue::undefined()]);
    // Value should pass through
    assert_eq!(p2.get_property("__state").map(|v| v.to_int32()), Some(1));
    assert_eq!(p2.get_property("__result").unwrap().to_int32(), 42);
}

#[test]
fn test_promise_then_no_handler_rejected() {
    let this = JSValue::undefined();
    let p = promise_reject(&this, &[JSValue::string("err")]);
    let p2 = promise_then(&p, &[JSValue::undefined(), JSValue::undefined()]);
    // Rejection should propagate
    assert_eq!(p2.get_property("__state").map(|v| v.to_int32()), Some(2));
    assert_eq!(p2.get_property("__result").unwrap().to_string(), "err");
}

#[test]
fn test_promise_reject_default() {
    let this = JSValue::undefined();
    let p = promise_reject(&this, &[]);
    assert_eq!(p.get_property("__state").map(|v| v.to_int32()), Some(2));
    assert!(p.get_property("__result").unwrap().is_undefined());
}

#[test]
fn test_promise_resolve_default() {
    let this = JSValue::undefined();
    let p = promise_resolve(&this, &[]);
    assert_eq!(p.get_property("__state").map(|v| v.to_int32()), Some(1));
    assert!(p.get_property("__result").unwrap().is_undefined());
}

#[test]
fn test_init_promise_registers_statics() {
    let mut ctx = make_ctx();
    init_promise(&mut ctx);
    let global = ctx.global.borrow();
    let promise_ctor = global.properties.get("Promise").unwrap();
    assert!(promise_ctor.is_callable());
    assert!(promise_ctor.get_property("resolve").is_some());
    assert!(promise_ctor.get_property("reject").is_some());
    assert!(promise_ctor.get_property("all").is_some());
    assert!(promise_ctor.get_property("allSettled").is_some());
    assert!(promise_ctor.get_property("race").is_some());
    assert!(promise_ctor.get_property("any").is_some());
    assert!(promise_ctor.get_property("prototype").is_some());
}
