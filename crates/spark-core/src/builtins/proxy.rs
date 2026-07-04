#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Proxy built-in.
//!
//! Implements the JavaScript Proxy constructor and all traps.
//! A Proxy wraps a target object and intercepts operations via a handler.

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Proxy constructor
// ============================================================================

/// Proxy(target, handler) constructor.
pub fn proxy_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let handler = args.get(1).cloned().unwrap_or(JSValue::undefined());

    // Validate: target must be an object or function
    if !target.is_object() {
        return JSValue::undefined(); // TypeError in a real engine
    }
    // Validate: handler must be an object
    if !handler.is_object() {
        return JSValue::undefined(); // TypeError in a real engine
    }

    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Proxy".to_string(),
    };
    obj.internal_slots.insert("target".to_string(), target);
    obj.internal_slots.insert("handler".to_string(), handler);

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// Trap helpers
// ============================================================================

/// Get a trap function from the handler.
fn get_trap(handler: &JSValue, trap_name: &str) -> Option<JSValue> {
    handler.get_property(trap_name)
}

/// Call a trap function with the given args. Returns Some(result) or None on error.
fn call_trap(trap: &JSValue, args: &[JSValue]) -> Option<JSValue> {
    if let JSValue::Function(f) = trap {
        let func = f.borrow();
        if let FunctionBody::Native(native_fn) = &func.body {
            Some(native_fn(&JSValue::undefined(), args))
        } else {
            None
        }
    } else {
        None
    }
}

/// Get the target of a proxy.
fn get_target(proxy: &JSValue) -> JSValue {
    match proxy {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            // Check internal_slots first, then properties
            borrow
                .internal_slots
                .get("target")
                .cloned()
                .or_else(|| borrow.properties.get("target").cloned())
                .unwrap_or(JSValue::undefined())
        }
        _ => JSValue::undefined(),
    }
}

/// Get the handler of a proxy.
fn get_handler(proxy: &JSValue) -> JSValue {
    match proxy {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            // Check internal_slots first, then properties
            borrow
                .internal_slots
                .get("handler")
                .cloned()
                .or_else(|| borrow.properties.get("handler").cloned())
                .unwrap_or(JSValue::undefined())
        }
        _ => JSValue::undefined(),
    }
}

// ============================================================================
// Proxy traps
// ============================================================================

/// Proxy get trap handler - intercepts property reads.
pub fn proxy_get_handler(proxy: &JSValue, property: &JSValue) -> JSValue {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "get") {
        let result = call_trap(&trap, &[target.clone(), property.clone(), proxy.clone()]);
        if let Some(val) = result {
            return val;
        }
    }

    // Fallback to target
    let prop_name = property.to_string();
    target.get_property(&prop_name).unwrap_or(JSValue::undefined())
}

/// Proxy set trap handler - intercepts property writes.
pub fn proxy_set_handler(proxy: &JSValue, property: &JSValue, value: &JSValue) -> bool {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "set") {
        let result = call_trap(&trap, &[target.clone(), property.clone(), value.clone(), proxy.clone()]);
        if let Some(val) = result {
            return val.to_boolean();
        }
    }

    // Fallback: set on target
    let prop_name = property.to_string();
    target.set_property(&prop_name, value.clone());
    true
}

/// Proxy has trap handler - intercepts `in` operator.
pub fn proxy_has_handler(proxy: &JSValue, property: &JSValue) -> bool {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "has") {
        let result = call_trap(&trap, &[target.clone(), property.clone()]);
        if let Some(val) = result {
            return val.to_boolean();
        }
    }

    // Fallback to target
    let prop_name = property.to_string();
    target.has_property(&prop_name)
}

/// Proxy deleteProperty trap handler.
pub fn proxy_delete_handler(proxy: &JSValue, property: &JSValue) -> bool {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "deleteProperty") {
        let result = call_trap(&trap, &[target, property.clone()]);
        if let Some(val) = result {
            return val.to_boolean();
        }
    }

    // Fallback
    true
}

/// Proxy apply trap handler - intercepts function calls.
pub fn proxy_apply_handler(proxy: &JSValue, this: &JSValue, args: &[JSValue]) -> JSValue {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "apply") {
        // Create args array
        let args_arr = JSValue::object("Array");
        for (i, arg) in args.iter().enumerate() {
            args_arr.set_property(&i.to_string(), arg.clone());
        }
        args_arr.set_property("length", JSValue::Int(args.len() as i32));

        let result = call_trap(&trap, &[target, this.clone(), args_arr]);
        if let Some(val) = result {
            return val;
        }
    }

    JSValue::undefined()
}

/// Proxy construct trap handler - intercepts `new` operator.
pub fn proxy_construct_handler(proxy: &JSValue, args: &[JSValue]) -> JSValue {
    let handler = get_handler(proxy);
    let target = get_target(proxy);

    if let Some(trap) = get_trap(&handler, "construct") {
        let result = call_trap(&trap, &[target, JSValue::undefined()]);
        if let Some(val) = result {
            if val.is_object() {
                return val;
            }
        }
    }

    // Fallback: create target object
    JSValue::object("Object")
}

// ============================================================================
// Proxy.revocable
// ============================================================================

/// Proxy.revocable(target, handler) - Returns {proxy, revoke}
pub fn proxy_revocable(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let handler = args.get(1).cloned().unwrap_or(JSValue::undefined());

    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let revoke = JSValue::function(
        Some("revoke"),
        vec![],
        FunctionBody::Closure(Rc::new({
            let proxy_clone = proxy.clone();
            move |_this: &JSValue, _args: &[JSValue]| {
                // Nullify target and handler
                if let JSValue::Object(obj) = &proxy_clone {
                    obj.borrow_mut()
                        .internal_slots
                        .insert("target".to_string(), JSValue::null());
                    obj.borrow_mut()
                        .internal_slots
                        .insert("handler".to_string(), JSValue::null());
                }
                JSValue::undefined()
            }
        })),
    );

    let result = JSValue::object("Object");
    result.set_property("proxy", proxy);
    result.set_property("revoke", revoke);
    result
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Proxy constructor.
pub fn init_proxy(ctx: &mut JSContext) {
    let proxy_ctor = JSValue::function(
        Some("Proxy"),
        vec!["target".to_string(), "handler".to_string()],
        FunctionBody::Native(proxy_constructor),
    );

    proxy_ctor.set_property(
        "revocable",
        JSValue::function(
            Some("revocable"),
            vec!["target".to_string(), "handler".to_string()],
            FunctionBody::Native(proxy_revocable),
        ),
    );

    ctx.global
        .borrow_mut()
        .properties
        .insert("Proxy".to_string(), proxy_ctor);
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
    fn test_proxy_constructor() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        let handler = JSValue::object("Object");
        let proxy = proxy_constructor(&this, &[target, handler]);
        match &proxy {
            JSValue::Object(obj) => {
                assert_eq!(obj.borrow().class_name, "Proxy");
                assert!(obj.borrow().internal_slots.get("target").is_some());
                assert!(obj.borrow().internal_slots.get("handler").is_some());
            }
            _ => panic!("Expected Proxy object"),
        }
    }

    #[test]
    fn test_proxy_get_trap() {
        let target = JSValue::object("Object");
        target.set_property("x", JSValue::int(42));

        let handler = JSValue::object("Object");
        handler.set_property(
            "get",
            JSValue::function(
                Some("get"),
                vec!["target".to_string(), "prop".to_string(), "receiver".to_string()],
                FunctionBody::Native(|_this, args| {
                    let target = &args[0];
                    let prop = &args[1];
                    let prop_name = prop.to_string();
                    target.get_property(&prop_name).unwrap_or(JSValue::undefined())
                }),
            ),
        );

        let proxy = JSValue::object("Proxy");
        proxy.set_property("target", target);
        proxy.set_property("handler", handler);

        let result = proxy_get_handler(&proxy, &JSValue::string("x"));
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_proxy_set_trap() {
        let target = JSValue::object("Object");

        let handler = JSValue::object("Object");
        handler.set_property(
            "set",
            JSValue::function(
                Some("set"),
                vec![],
                FunctionBody::Native(|_this, args| {
                    let target = &args[0];
                    let prop = &args[1];
                    let val = &args[2];
                    let prop_name = prop.to_string();
                    target.set_property(&prop_name, val.clone());
                    JSValue::bool(true)
                }),
            ),
        );

        let proxy = JSValue::object("Proxy");
        proxy.set_property("target", target.clone());
        proxy.set_property("handler", handler);

        let result = proxy_set_handler(&proxy, &JSValue::string("x"), &JSValue::int(99));
        assert!(result);
        assert_eq!(target.get_property("x").unwrap().to_int32(), 99);
    }

    #[test]
    fn test_proxy_has_trap() {
        let target = JSValue::object("Object");
        target.set_property("x", JSValue::int(1));

        let handler = JSValue::object("Object");
        handler.set_property(
            "has",
            JSValue::function(
                Some("has"),
                vec![],
                FunctionBody::Native(|_this, args| {
                    let prop = &args[1];
                    let prop_name = prop.to_string();
                    JSValue::bool(prop_name == "x")
                }),
            ),
        );

        let proxy = JSValue::object("Proxy");
        proxy.set_property("target", target);
        proxy.set_property("handler", handler);

        assert!(proxy_has_handler(&proxy, &JSValue::string("x")));
        assert!(!proxy_has_handler(&proxy, &JSValue::string("y")));
    }

    #[test]
    fn test_proxy_delete_trap() {
        let target = JSValue::object("Object");

        let handler = JSValue::object("Object");
        handler.set_property(
            "deleteProperty",
            JSValue::function(
                Some("deleteProperty"),
                vec![],
                FunctionBody::Native(|_this, _args| JSValue::bool(true)),
            ),
        );

        let proxy = JSValue::object("Proxy");
        proxy.set_property("target", target);
        proxy.set_property("handler", handler);

        assert!(proxy_delete_handler(&proxy, &JSValue::string("x")));
    }

    #[test]
    fn test_proxy_revocable() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        let handler = JSValue::object("Object");

        let result = proxy_revocable(&this, &[target, handler]);
        assert!(result.get_property("proxy").is_some());
        assert!(result.get_property("revoke").is_some());
    }

    #[test]
    fn test_init_proxy() {
        let mut ctx = make_ctx();
        init_proxy(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Proxy").is_some());
    }
}
