#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Promise built-in.
//!
//! Implements the JavaScript Promise constructor and its methods.
//! Note: Since this is a synchronous interpreter without an event loop,
//! promises are represented with a state machine but cannot actually
//! resolve asynchronously. The API surface is complete.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Promise state constants
// ============================================================================

const STATE_PENDING: i32 = 0;
const STATE_FULFILLED: i32 = 1;
const STATE_REJECTED: i32 = 2;

// ============================================================================
// Promise helpers
// ============================================================================

fn get_promise_state(this: &JSValue) -> i32 {
    this.get_property("__state")
        .map(|v| v.to_int32())
        .unwrap_or(STATE_PENDING)
}

fn get_promise_result(this: &JSValue) -> JSValue {
    this.get_property("__result")
        .unwrap_or(JSValue::undefined())
}

fn set_promise_state(this: &JSValue, state: i32) {
    this.set_property("__state", JSValue::Int(state));
}

fn set_promise_result(this: &JSValue, result: JSValue) {
    this.set_property("__result", result);
}

fn get_promise_fulfill_reactions(this: &JSValue) -> Vec<JSValue> {
    // Reactions are now stored as a JS array in __reactions property
    Vec::new()
}

fn add_promise_reaction(this: &JSValue, reaction: JSValue) {
    if let Some(reactions) = this.get_property("__reactions") {
        let len = reactions.get_property("length")
            .map(|v| v.to_number() as usize)
            .unwrap_or(0);
        reactions.set_property(&len.to_string(), reaction);
        reactions.set_property("length", JSValue::Int((len + 1) as i32));
    }
}

// ============================================================================
// Promise constructor
// ============================================================================

/// Promise(executor) constructor.
pub fn promise_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let promise = JSValue::object("Promise");
    // Use properties instead of internal_slots so JS code can access them
    promise.set_property("__state", JSValue::Int(STATE_PENDING));
    promise.set_property("__result", JSValue::undefined());
    promise.set_property("__reactions", crate::builtins::array::create_array(vec![]));

    // If an executor function is provided, call it with resolve and reject
    if let Some(executor) = args.get(0) {
        if executor.is_callable() {
            // Create resolve and reject functions
            let resolve = JSValue::function(
                Some("resolve"),
                vec!["value".to_string()],
                FunctionBody::Closure(Rc::new({
                    let promise_clone = promise.clone();
                    move |_this: &JSValue, args: &[JSValue]| {
                        let val = args.get(0).cloned().unwrap_or(JSValue::undefined());
                        let state = promise_clone.get_property("__state")
                            .map(|v| v.to_int32())
                            .unwrap_or(STATE_PENDING);
                        if state == STATE_PENDING {
                            promise_clone.set_property("__state", JSValue::Int(STATE_FULFILLED));
                            promise_clone.set_property("__result", val.clone());
                            // Process reactions
                            if let Some(reactions_val) = promise_clone.get_property("__reactions") {
                                let len = reactions_val.get_property("length")
                                    .map(|v| v.to_number() as usize)
                                    .unwrap_or(0);
                                for i in 0..len {
                                    if let Some(reaction) = reactions_val.get_property(&i.to_string()) {
                                        if let Some(on_fulfilled) = reaction.get_property("onFulfilled") {
                                            if on_fulfilled.is_callable() {
                                                // Can't call bytecode from native - skip
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        JSValue::undefined()
                    }
                })),
            );

            let reject = JSValue::function(
                Some("reject"),
                vec!["reason".to_string()],
                FunctionBody::Closure(Rc::new({
                    let promise_clone = promise.clone();
                    move |_this: &JSValue, args: &[JSValue]| {
                        let reason = args.get(0).cloned().unwrap_or(JSValue::undefined());
                        let state = promise_clone.get_property("__state")
                            .map(|v| v.to_int32())
                            .unwrap_or(STATE_PENDING);
                        if state == STATE_PENDING {
                            promise_clone.set_property("__state", JSValue::Int(STATE_REJECTED));
                            promise_clone.set_property("__result", reason);
                        }
                        JSValue::undefined()
                    }
                })),
            );

            // Call executor - handles all function types
            if let JSValue::Function(f) = executor {
                let func = f.borrow();
                match &func.body {
                    FunctionBody::Native(native_fn) => {
                        native_fn(&JSValue::undefined(), &[resolve, reject]);
                    }
                    FunctionBody::Closure(closure_fn) => {
                        closure_fn(&JSValue::undefined(), &[resolve, reject]);
                    }
                    _ => {}
                }
            }
        }
    }

    promise
}

// ============================================================================
// Promise.prototype methods
// ============================================================================

/// Promise.prototype.then(onFulfilled, onRejected)
pub fn promise_then(this: &JSValue, args: &[JSValue]) -> JSValue {
    let state = get_promise_state(this);
    let result = get_promise_result(this);

    let on_fulfilled = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let on_rejected = args.get(1).cloned().unwrap_or(JSValue::undefined());

    // Create a new promise for the chaining
    let new_promise = promise_constructor(&JSValue::undefined(), &[]);

    // Helper to call any function type
    fn call_any(func: &JSValue, this: &JSValue, args: &[JSValue]) -> JSValue {
        match func {
            JSValue::Function(f) => {
                let borrow = f.borrow();
                match &borrow.body {
                    FunctionBody::Native(native_fn) => native_fn(this, args),
                    FunctionBody::Closure(closure_fn) => closure_fn(this, args),
                    _ => JSValue::undefined(),
                }
            }
            _ => JSValue::undefined(),
        }
    }

    if state == STATE_FULFILLED {
        if on_fulfilled.is_callable() {
            let value = call_any(&on_fulfilled, &JSValue::undefined(), &[result.clone()]);
            if get_promise_state(&new_promise) == STATE_PENDING {
                set_promise_state(&new_promise, STATE_FULFILLED);
                set_promise_result(&new_promise, value);
            }
        } else {
            set_promise_state(&new_promise, STATE_FULFILLED);
            set_promise_result(&new_promise, result);
        }
    } else if state == STATE_REJECTED {
        if on_rejected.is_callable() {
            let value = call_any(&on_rejected, &JSValue::undefined(), &[result.clone()]);
            set_promise_state(&new_promise, STATE_FULFILLED);
            set_promise_result(&new_promise, value);
        } else {
            set_promise_state(&new_promise, STATE_REJECTED);
            set_promise_result(&new_promise, result);
        }
    } else {
        // Still pending - add reactions
        let reaction = JSValue::object("Reaction");
        reaction.set_property("onFulfilled", on_fulfilled);
        reaction.set_property("onRejected", on_rejected);
        reaction.set_property("promise", new_promise.clone());
        add_promise_reaction(this, reaction);
    }

    new_promise
}

/// Promise.prototype.catch(onRejected)
pub fn promise_catch(this: &JSValue, args: &[JSValue]) -> JSValue {
    let on_rejected = args.get(0).cloned().unwrap_or(JSValue::undefined());
    promise_then(this, &[JSValue::undefined(), on_rejected])
}

/// Promise.prototype.finally(onFinally)
pub fn promise_finally(this: &JSValue, args: &[JSValue]) -> JSValue {
    let on_finally = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let state = get_promise_state(this);
    let result = get_promise_result(this);

    if state == STATE_PENDING {
        // Add as both fulfilled and rejected reaction
        let reaction = JSValue::object("Reaction");
        reaction.set_property("onFulfilled", on_finally.clone());
        reaction.set_property("onRejected", on_finally);
        reaction.set_property("promise", this.clone());
        add_promise_reaction(this, reaction);
        this.clone()
    } else {
        // Immediately invoke onFinally
        if on_finally.is_callable() {
            if let JSValue::Function(f) = &on_finally {
                let func = f.borrow();
                if let FunctionBody::Native(native_fn) = &func.body {
                    native_fn(&JSValue::undefined(), &[]);
                }
            }
        }
        // Return a new promise that resolves with the same value
        let new_promise = promise_constructor(&JSValue::undefined(), &[]);
        set_promise_state(&new_promise, state);
        set_promise_result(&new_promise, result);
        new_promise
    }
}

// ============================================================================
// Static methods
// ============================================================================

/// Promise.resolve(value)
pub fn promise_resolve(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let promise = promise_constructor(&JSValue::undefined(), &[]);
    promise.set_property("__state", JSValue::Int(STATE_FULFILLED));
    promise.set_property("__result", value);
    promise
}

/// Promise.reject(reason)
pub fn promise_reject(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let reason = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let promise = promise_constructor(&JSValue::undefined(), &[]);
    promise.set_property("__state", JSValue::Int(STATE_REJECTED));
    promise.set_property("__result", reason);
    promise
}

/// Promise.all(iterable)
pub fn promise_all(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let iterable = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let result_promise = promise_constructor(&JSValue::undefined(), &[]);

    let mut values = Vec::new();
    let mut all_fulfilled = true;

    // Iterate over the iterable
    if let Some(length_val) = iterable.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let item = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            let state = item.get_property("__state").map(|v| v.to_int32());
            match state {
                Some(s) if s == STATE_FULFILLED => {
                    let result = item.get_property("__result").unwrap_or(JSValue::undefined());
                    values.push(result);
                }
                Some(s) if s == STATE_REJECTED => {
                    let reason = item.get_property("__result").unwrap_or(JSValue::undefined());
                    set_promise_state(&result_promise, STATE_REJECTED);
                    set_promise_result(&result_promise, reason);
                    return result_promise;
                }
                _ => {
                    all_fulfilled = false;
                    values.push(JSValue::undefined());
                }
            }
        }
    }

    if all_fulfilled {
        // Create an array of resolved values
        let arr = JSValue::object("Array");
        for (i, val) in values.iter().enumerate() {
            arr.set_property(&i.to_string(), val.clone());
        }
        arr.set_property("length", JSValue::Int(values.len() as i32));
        set_promise_state(&result_promise, STATE_FULFILLED);
        set_promise_result(&result_promise, arr);
    }

    result_promise
}

/// Promise.allSettled(iterable)
pub fn promise_all_settled(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let iterable = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let result_promise = promise_constructor(&JSValue::undefined(), &[]);

    let mut results = Vec::new();

    if let Some(length_val) = iterable.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let item = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            let state = item.get_property("__state").map(|v| v.to_int32());
            let result_val = item.get_property("__result").unwrap_or(JSValue::undefined());

            let descriptor = JSValue::object("Object");
            match state {
                Some(s) if s == STATE_FULFILLED => {
                    descriptor.set_property("status", JSValue::string("fulfilled"));
                    descriptor.set_property("value", result_val);
                }
                Some(s) if s == STATE_REJECTED => {
                    descriptor.set_property("status", JSValue::string("rejected"));
                    descriptor.set_property("reason", result_val);
                }
                _ => {
                    descriptor.set_property("status", JSValue::string("fulfilled"));
                    descriptor.set_property("value", item.clone());
                }
            }
            results.push(descriptor);
        }
    }

    let arr = JSValue::object("Array");
    for (i, val) in results.iter().enumerate() {
        arr.set_property(&i.to_string(), val.clone());
    }
    arr.set_property("length", JSValue::Int(results.len() as i32));
    set_promise_state(&result_promise, STATE_FULFILLED);
    set_promise_result(&result_promise, arr);
    result_promise
}

/// Promise.race(iterable)
pub fn promise_race(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let iterable = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let result_promise = promise_constructor(&JSValue::undefined(), &[]);

    if let Some(length_val) = iterable.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let item = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            let state = item.get_property("__state").map(|v| v.to_int32());
            if let Some(s) = state {
                if s == STATE_FULFILLED || s == STATE_REJECTED {
                    let result_val = item.get_property("__result").unwrap_or(JSValue::undefined());
                    set_promise_state(&result_promise, s);
                    set_promise_result(&result_promise, result_val);
                    return result_promise;
                }
            }
        }
    }

    result_promise
}

/// Promise.any(iterable)
pub fn promise_any(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let iterable = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let result_promise = promise_constructor(&JSValue::undefined(), &[]);

    let mut errors = Vec::new();
    let mut all_rejected = true;

    if let Some(length_val) = iterable.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let item = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            let state = item.get_property("__state").map(|v| v.to_int32());
            let result_val = item.get_property("__result").unwrap_or(JSValue::undefined());

            match state {
                Some(s) if s == STATE_FULFILLED => {
                    set_promise_state(&result_promise, STATE_FULFILLED);
                    set_promise_result(&result_promise, result_val);
                    return result_promise;
                }
                Some(s) if s == STATE_REJECTED => {
                    errors.push(result_val);
                }
                _ => {
                    all_rejected = false;
                }
            }
        }
    }

    if all_rejected {
        // Reject with an AggregateError
        let aggregate_error = JSValue::object("AggregateError");
        aggregate_error.set_property("message", JSValue::string("All promises were rejected"));
        aggregate_error.set_property("name", JSValue::string("AggregateError"));
        let errors_arr = JSValue::object("Array");
        for (i, err) in errors.iter().enumerate() {
            errors_arr.set_property(&i.to_string(), err.clone());
        }
        errors_arr.set_property("length", JSValue::Int(errors.len() as i32));
        aggregate_error.set_property("errors", errors_arr);
        set_promise_state(&result_promise, STATE_REJECTED);
        set_promise_result(&result_promise, aggregate_error);
    }

    result_promise
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Promise constructor and prototype.
pub fn init_promise(ctx: &mut JSContext) {
    let promise_ctor = JSValue::function(
        Some("Promise"),
        vec!["executor".to_string()],
        FunctionBody::Native(promise_constructor),
    );

    // Create Promise.prototype
    let prototype = JSValue::object("Promise");

    // Use JS-implemented methods for then/catch/finally
    // These can call bytecode callbacks (arrow functions, etc.)
    let js_methods = super::js_builtins::compile_promise_js_methods();
    for (name, func) in js_methods {
        prototype.set_property(&name, func);
    }

    promise_ctor.set_property("prototype", prototype);

    // Static methods
    promise_ctor.set_property(
        "resolve",
        JSValue::function(
            Some("resolve"),
            vec!["value".to_string()],
            FunctionBody::Native(promise_resolve),
        ),
    );
    promise_ctor.set_property(
        "reject",
        JSValue::function(
            Some("reject"),
            vec!["reason".to_string()],
            FunctionBody::Native(promise_reject),
        ),
    );
    promise_ctor.set_property(
        "all",
        JSValue::function(
            Some("all"),
            vec!["iterable".to_string()],
            FunctionBody::Native(promise_all),
        ),
    );
    promise_ctor.set_property(
        "allSettled",
        JSValue::function(
            Some("allSettled"),
            vec!["iterable".to_string()],
            FunctionBody::Native(promise_all_settled),
        ),
    );
    promise_ctor.set_property(
        "race",
        JSValue::function(
            Some("race"),
            vec!["iterable".to_string()],
            FunctionBody::Native(promise_race),
        ),
    );
    promise_ctor.set_property(
        "any",
        JSValue::function(
            Some("any"),
            vec!["iterable".to_string()],
            FunctionBody::Native(promise_any),
        ),
    );

    ctx.global
        .borrow_mut()
        .properties
        .insert("Promise".to_string(), promise_ctor);
}

// ============================================================================
// Tests
// ============================================================================

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
    fn test_promise_constructor() {
        let this = JSValue::undefined();
        let p = promise_constructor(&this, &[]);
        assert_eq!(get_promise_state(&p), STATE_PENDING);
    }

    #[test]
    fn test_promise_resolve() {
        let this = JSValue::undefined();
        let p = promise_resolve(&this, &[JSValue::int(42)]);
        assert_eq!(get_promise_state(&p), STATE_FULFILLED);
        assert_eq!(get_promise_result(&p).to_int32(), 42);
    }

    #[test]
    fn test_promise_reject() {
        let this = JSValue::undefined();
        let p = promise_reject(&this, &[JSValue::string("error")]);
        assert_eq!(get_promise_state(&p), STATE_REJECTED);
        assert_eq!(get_promise_result(&p).to_string(), "error");
    }

    #[test]
    fn test_promise_then_on_fulfilled() {
        let this = JSValue::undefined();
        let p = promise_resolve(&this, &[JSValue::int(5)]);

        // Create a then callback that doubles the value
        let callback = JSValue::function(
            Some("double"),
            vec!["val".to_string()],
            FunctionBody::Native(|_this, args| {
                let val = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::Float(val * 2.0)
            }),
        );

        let new_p = promise_then(&p, &[callback, JSValue::undefined()]);
        assert_eq!(get_promise_state(&new_p), STATE_FULFILLED);
        assert_eq!(get_promise_result(&new_p).to_number(), 10.0);
    }

    #[test]
    fn test_promise_catch_on_rejected() {
        let this = JSValue::undefined();
        let p = promise_reject(&this, &[JSValue::string("fail")]);

        let callback = JSValue::function(
            Some("handleError"),
            vec!["reason".to_string()],
            FunctionBody::Native(|_this, args| {
                let reason = args.get(0).map(|v| v.to_string()).unwrap_or_default();
                JSValue::string(&format!("handled: {}", reason))
            }),
        );

        let new_p = promise_catch(&p, &[callback]);
        assert_eq!(get_promise_state(&new_p), STATE_FULFILLED);
        assert_eq!(get_promise_result(&new_p).to_string(), "handled: fail");
    }

    #[test]
    fn test_promise_all() {
        let this = JSValue::undefined();
        let arr = JSValue::object("Array");
        arr.set_property("0", promise_resolve(&this, &[JSValue::int(1)]));
        arr.set_property("1", promise_resolve(&this, &[JSValue::int(2)]));
        arr.set_property("length", JSValue::int(2));

        let result = promise_all(&this, &[arr]);
        assert_eq!(get_promise_state(&result), STATE_FULFILLED);
    }

    #[test]
    fn test_init_promise() {
        let mut ctx = make_ctx();
        init_promise(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Promise").is_some());
    }
}
