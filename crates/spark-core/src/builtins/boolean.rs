#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Boolean built-in.
//!
//! Implements the JavaScript Boolean constructor and its methods.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Boolean constructor
// ============================================================================

/// Boolean constructor - `new Boolean(value)` or `Boolean(value)`
pub fn boolean_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).map(|v| v.to_boolean()).unwrap_or(false);
    JSValue::bool(val)
}

// ============================================================================
// Boolean.prototype methods
// ============================================================================

/// Boolean.prototype.valueOf()
pub fn boolean_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Bool(b) => JSValue::bool(*b),
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            if let Some(val) = borrow.internal_slots.get("PrimitiveValue") {
                val.clone()
            } else {
                JSValue::undefined()
            }
        }
        _ => JSValue::undefined(),
    }
}

/// Boolean.prototype.toString()
pub fn boolean_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Bool(b) => JSValue::string(if *b { "true" } else { "false" }),
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            if let Some(val) = borrow.internal_slots.get("PrimitiveValue") {
                match val {
                    JSValue::Bool(b) => JSValue::string(if *b { "true" } else { "false" }),
                    _ => JSValue::string("undefined"),
                }
            } else {
                JSValue::string("undefined")
            }
        }
        _ => JSValue::string("undefined"),
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Boolean constructor and prototype.
pub fn init_boolean(ctx: &mut JSContext) {
    // Create the Boolean constructor function
    let constructor = JSValue::function(
        Some("Boolean"),
        vec!["value".to_string()],
        FunctionBody::Native(boolean_constructor),
    );

    // Create Boolean.prototype
    let prototype = JSValue::object("Boolean");
    prototype.set_property("valueOf", JSValue::function(
        Some("valueOf"),
        vec![],
        FunctionBody::Native(boolean_value_of),
    ));
    prototype.set_property("toString", JSValue::function(
        Some("toString"),
        vec![],
        FunctionBody::Native(boolean_to_string),
    ));

    // Set Boolean.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Set Boolean on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("Boolean".to_string(), constructor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_boolean_constructor() {
        let this = JSValue::undefined();
        let result = boolean_constructor(&this, &[JSValue::int(1)]);
        assert_eq!(result, JSValue::bool(true));

        let result = boolean_constructor(&this, &[JSValue::int(0)]);
        assert_eq!(result, JSValue::bool(false));

        let result = boolean_constructor(&this, &[]);
        assert_eq!(result, JSValue::bool(false));
    }

    #[test]
    fn test_boolean_value_of() {
        let this = JSValue::bool(true);
        let result = boolean_value_of(&this, &[]);
        assert_eq!(result, JSValue::bool(true));

        let this = JSValue::bool(false);
        let result = boolean_value_of(&this, &[]);
        assert_eq!(result, JSValue::bool(false));
    }

    #[test]
    fn test_boolean_to_string() {
        let this = JSValue::bool(true);
        let result = boolean_to_string(&this, &[]);
        assert_eq!(result.to_string(), "true");

        let this = JSValue::bool(false);
        let result = boolean_to_string(&this, &[]);
        assert_eq!(result.to_string(), "false");
    }

    #[test]
    fn test_init_boolean() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        init_boolean(&mut ctx);
        assert!(ctx.global.borrow().properties.get("Boolean").is_some());
    }
}
