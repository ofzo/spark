#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Error built-in.
//!
//! Implements the JavaScript Error constructor and its methods.
//! Supports Error, TypeError, RangeError, ReferenceError, SyntaxError,
//! URIError, and EvalError.

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Helper: create an error object
// ============================================================================

fn create_error_object(class_name: &str, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: class_name.to_string(),
    };
    obj.properties.insert("message".to_string(), JSValue::string(&message));
    obj.properties.insert("name".to_string(), JSValue::string(class_name));
    obj.properties.insert(
        "stack".to_string(),
        JSValue::string(&format!("{}: {}", class_name, message)),
    );
    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// Error constructor
// ============================================================================

/// Error(message) constructor.
pub fn error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("Error"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("Error: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// TypeError constructor
// ============================================================================

/// TypeError(message) constructor.
pub fn type_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("TypeError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("TypeError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// RangeError constructor
// ============================================================================

/// RangeError(message) constructor.
pub fn range_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("RangeError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("RangeError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// ReferenceError constructor
// ============================================================================

/// ReferenceError(message) constructor.
pub fn reference_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("ReferenceError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("ReferenceError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// SyntaxError constructor
// ============================================================================

/// SyntaxError(message) constructor.
pub fn syntax_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("SyntaxError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("SyntaxError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// URIError constructor
// ============================================================================

/// URIError(message) constructor.
pub fn uri_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("URIError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("URIError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// EvalError constructor
// ============================================================================

/// EvalError(message) constructor.
pub fn eval_error_constructor(this: &JSValue, args: &[JSValue]) -> JSValue {
    let message = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if let JSValue::Object(obj) = this {
        let mut borrow = obj.borrow_mut();
        borrow.properties.insert("message".to_string(), JSValue::string(&message));
        borrow.properties.insert("name".to_string(), JSValue::string("EvalError"));
        borrow.properties.insert("stack".to_string(), JSValue::string(&format!("EvalError: {}", message)));
    }
    JSValue::Undefined
}

// ============================================================================
// Error.prototype methods
// ============================================================================

/// Error.prototype.toString()
pub fn error_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let obj = match this {
        JSValue::Object(o) => o.clone(),
        _ => return JSValue::string("Error"),
    };
    let borrow = obj.borrow();
    let name = borrow
        .internal_slots
        .get("name")
        .or_else(|| borrow.properties.get("name"))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "Error".to_string());
    let message = borrow
        .internal_slots
        .get("message")
        .or_else(|| borrow.properties.get("message"))
        .map(|v| v.to_string())
        .unwrap_or_default();
    if message.is_empty() {
        JSValue::string(&name)
    } else {
        JSValue::string(&format!("{}: {}", name, message))
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Error constructor and prototype, including all error subclasses.
pub fn init_error(ctx: &mut JSContext) {
    // Create the base Error constructor
    let error_constructor_fn = JSValue::function(
        Some("Error"),
        vec!["message".to_string()],
        FunctionBody::Native(error_constructor),
    );

    // Create Error.prototype
    let error_prototype = JSValue::object("Error");
    error_prototype.set_property(
        "toString",
        JSValue::function(Some("toString"), vec![], FunctionBody::Native(error_to_string)),
    );
    error_prototype.set_property("message", JSValue::string(""));
    error_prototype.set_property("name", JSValue::string("Error"));
    error_prototype.set_property(
        "stack",
        JSValue::string(""),
    );

    // Set constructor and prototype linkage
    error_constructor_fn.set_property("prototype", error_prototype.clone());
    error_constructor_fn.set_property(
        "name",
        JSValue::string("Error"),
    );

    // Install Error on global
    ctx.global
        .borrow_mut()
        .properties
        .insert("Error".to_string(), error_constructor_fn);

    // Helper to create an error subclass
    fn make_error_subclass(
        name: &str,
        constructor_fn: fn(&JSValue, &[JSValue]) -> JSValue,
    ) -> JSValue {
        let ctor = JSValue::function(
            Some(name),
            vec!["message".to_string()],
            FunctionBody::Native(constructor_fn),
        );

        let proto = JSValue::object(name);
        proto.set_property(
            "toString",
            JSValue::function(
                Some("toString"),
                vec![],
                FunctionBody::Native(error_to_string),
            ),
        );
        proto.set_property("message", JSValue::string(""));
        proto.set_property("name", JSValue::string(name));

        ctor.set_property("prototype", proto);
        ctor.set_property("name", JSValue::string(name));

        ctor
    }

    // Install error subclasses
    let subclasses: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("TypeError", type_error_constructor),
        ("RangeError", range_error_constructor),
        ("ReferenceError", reference_error_constructor),
        ("SyntaxError", syntax_error_constructor),
        ("URIError", uri_error_constructor),
        ("EvalError", eval_error_constructor),
    ];

    for &(name, ctor_fn) in subclasses {
        let subclass = make_error_subclass(name, ctor_fn);
        ctx.global
            .borrow_mut()
            .properties
            .insert(name.to_string(), subclass);
    }
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
    fn test_error_constructor() {
        // Simulate `new Error("something failed")` - handle_new creates the object
        let obj = JSValue::object("Error");
        error_constructor(&obj, &[JSValue::string("something failed")]);
        match &obj {
            JSValue::Object(o) => {
                let borrow = o.borrow();
                assert_eq!(
                    borrow.properties.get("message").unwrap().to_string(),
                    "something failed"
                );
                assert_eq!(
                    borrow.properties.get("name").unwrap().to_string(),
                    "Error"
                );
            }
            _ => panic!("Expected Error object"),
        }
    }

    #[test]
    fn test_type_error_constructor() {
        let obj = JSValue::object("TypeError");
        type_error_constructor(&obj, &[JSValue::string("bad type")]);
        match &obj {
            JSValue::Object(o) => {
                let borrow = o.borrow();
                assert_eq!(
                    borrow.properties.get("message").unwrap().to_string(),
                    "bad type"
                );
                assert_eq!(
                    borrow.properties.get("name").unwrap().to_string(),
                    "TypeError"
                );
            }
            _ => panic!("Expected TypeError object"),
        }
    }

    #[test]
    fn test_error_to_string() {
        let obj = JSValue::object("Error");
        error_constructor(&obj, &[JSValue::string("oops")]);
        let result = error_to_string(&obj, &[]);
        assert_eq!(result.to_string(), "Error: oops");
    }

    #[test]
    fn test_error_to_string_no_message() {
        let this = JSValue::undefined();
        let error_obj = error_constructor(&this, &[]);
        let result = error_to_string(&error_obj, &[]);
        assert_eq!(result.to_string(), "Error");
    }

    #[test]
    fn test_init_error() {
        let mut ctx = make_ctx();
        init_error(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Error").is_some());
        assert!(global.properties.get("TypeError").is_some());
        assert!(global.properties.get("RangeError").is_some());
        assert!(global.properties.get("ReferenceError").is_some());
        assert!(global.properties.get("SyntaxError").is_some());
        assert!(global.properties.get("URIError").is_some());
        assert!(global.properties.get("EvalError").is_some());
    }
}
