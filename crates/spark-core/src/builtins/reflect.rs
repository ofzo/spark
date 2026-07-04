#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Reflect built-in.
//!
//! Implements the JavaScript Reflect object with all its static methods.
//! Reflect provides methods that mirror the proxy trap API and offer
//! standard object operations.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Reflect.get(target, propertyKey [, receiver])
// ============================================================================

/// Reflect.get(target, propertyKey [, receiver])
pub fn reflect_get(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let _receiver = args.get(2).cloned();

    let prop_name = property_key.to_string();
    match &target {
        JSValue::Object(obj) => {
            // Check if target has its own property first
            let borrow = obj.borrow();
            if let Some(val) = borrow.properties.get(&prop_name) {
                return val.clone();
            }
            // Check prototype chain
            drop(borrow);
            target.get_property(&prop_name).unwrap_or(JSValue::undefined())
        }
        JSValue::Function(f) => {
            let borrow = f.borrow();
            if let Some(val) = borrow.closure.get(&prop_name) {
                return val.borrow().clone();
            }
            // Check function's special properties
            drop(borrow);
            target.get_property(&prop_name).unwrap_or(JSValue::undefined())
        }
        _ => {
            // For primitives, create wrapper and get property
            match target.to_object() {
                Ok(obj) => obj.get_property(&prop_name).unwrap_or(JSValue::undefined()),
                Err(_) => JSValue::undefined(),
            }
        }
    }
}

// ============================================================================
// Reflect.set(target, propertyKey, value [, receiver])
// ============================================================================

/// Reflect.set(target, propertyKey, value [, receiver])
pub fn reflect_set(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let value = args.get(2).cloned().unwrap_or(JSValue::undefined());

    let prop_name = property_key.to_string();
    target.set_property(&prop_name, value);
    JSValue::bool(true)
}

// ============================================================================
// Reflect.has(target, propertyKey)
// ============================================================================

/// Reflect.has(target, propertyKey)
pub fn reflect_has(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let prop_name = property_key.to_string();
    JSValue::bool(target.has_property(&prop_name))
}

// ============================================================================
// Reflect.deleteProperty(target, propertyKey)
// ============================================================================

/// Reflect.deleteProperty(target, propertyKey)
pub fn reflect_delete_property(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let prop_name = property_key.to_string();

    match &target {
        JSValue::Object(obj) => {
            let had_prop = obj.borrow().properties.contains_key(&prop_name);
            if had_prop {
                obj.borrow_mut().properties.remove(&prop_name);
                JSValue::bool(true)
            } else {
                JSValue::bool(true) // Deleting non-existent is success
            }
        }
        JSValue::Function(f) => {
            let had_prop = f.borrow().closure.contains_key(&prop_name);
            if had_prop {
                f.borrow_mut().closure.remove(&prop_name);
                JSValue::bool(true)
            } else {
                JSValue::bool(true)
            }
        }
        _ => JSValue::bool(true),
    }
}

// ============================================================================
// Reflect.apply(target, thisArgument, argumentsList)
// ============================================================================

/// Reflect.apply(target, thisArgument, argumentsList)
pub fn reflect_apply(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let this_arg = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let arguments_list = args.get(2).cloned().unwrap_or(JSValue::undefined());

    // Convert arguments list to a Vec
    let mut call_args = Vec::new();
    if let Some(length_val) = arguments_list.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let arg = arguments_list.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            call_args.push(arg);
        }
    }

    match &target {
        JSValue::Function(f) => {
            let func_ref = f.clone();
            let body_clone = {
                let func_borrow = func_ref.borrow();
                func_borrow.body.clone()
            };
            match body_clone {
                FunctionBody::Native(native_fn) => {
                    native_fn(&this_arg, &call_args)
                }
                _ => JSValue::undefined(),
            }
        }
        _ => JSValue::undefined(),
    }
}

// ============================================================================
// Reflect.construct(target, argumentsList [, newTarget])
// ============================================================================

/// Reflect.construct(target, argumentsList [, newTarget])
pub fn reflect_construct(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let arguments_list = args.get(1).cloned().unwrap_or(JSValue::undefined());

    // Convert arguments list
    let mut call_args = Vec::new();
    if let Some(length_val) = arguments_list.get_property("length") {
        let length = length_val.to_uint32();
        for i in 0..length {
            let arg = arguments_list.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
            call_args.push(arg);
        }
    }

    match &target {
        JSValue::Function(f) => {
            let func_ref = f.clone();
            let body_clone = {
                let func_borrow = func_ref.borrow();
                func_borrow.body.clone()
            };
            match body_clone {
                FunctionBody::Native(native_fn) => {
                    // Create a new object as `this` for the constructor
                    let obj = JSValue::object("Object");
                    let result = native_fn(&obj, &call_args);
                    // If constructor returns an object, use that; otherwise use the new object
                    if result.is_object() {
                        result
                    } else {
                        obj
                    }
                }
                _ => JSValue::object("Object"),
            }
        }
        _ => JSValue::object("Object"),
    }
}

// ============================================================================
// Reflect.getPrototypeOf(target)
// ============================================================================

/// Reflect.getPrototypeOf(target)
pub fn reflect_get_prototype_of(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    match &target {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            match &borrow.prototype {
                Some(proto) => JSValue::Object(proto.clone()),
                None => JSValue::null(),
            }
        }
        _ => JSValue::null(),
    }
}

// ============================================================================
// Reflect.setPrototypeOf(target, proto)
// ============================================================================

/// Reflect.setPrototypeOf(target, proto)
pub fn reflect_set_prototype_of(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let proto = args.get(1).cloned().unwrap_or(JSValue::null());

    match &target {
        JSValue::Object(obj) => {
            let proto_ref = match &proto {
                JSValue::Object(o) => Some(o.clone()),
                JSValue::Null => None,
                _ => return JSValue::bool(false),
            };
            obj.borrow_mut().prototype = proto_ref;
            JSValue::bool(true)
        }
        _ => JSValue::bool(false),
    }
}

// ============================================================================
// Reflect.defineProperty(target, propertyKey, attributes)
// ============================================================================

/// Reflect.defineProperty(target, propertyKey, attributes)
pub fn reflect_define_property(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let attributes = args.get(2).cloned().unwrap_or(JSValue::undefined());

    let prop_name = property_key.to_string();

    match &target {
        JSValue::Object(obj) => {
            // Get the value from attributes
            let value = attributes.get_property("value").unwrap_or(JSValue::undefined());
            obj.borrow_mut()
                .properties
                .insert(prop_name, value);
            JSValue::bool(true)
        }
        JSValue::Function(f) => {
            let value = attributes.get_property("value").unwrap_or(JSValue::undefined());
            f.borrow_mut()
                .closure
                .insert(prop_name, Rc::new(RefCell::new(value)));
            JSValue::bool(true)
        }
        _ => JSValue::bool(false),
    }
}

// ============================================================================
// Reflect.getOwnPropertyDescriptor(target, propertyKey)
// ============================================================================

/// Reflect.getOwnPropertyDescriptor(target, propertyKey)
pub fn reflect_get_own_property_descriptor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let property_key = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let prop_name = property_key.to_string();

    match &target {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            if let Some(value) = borrow.properties.get(&prop_name) {
                let descriptor = JSValue::object("Object");
                descriptor.set_property("value", value.clone());
                descriptor.set_property("writable", JSValue::bool(true));
                descriptor.set_property("enumerable", JSValue::bool(true));
                descriptor.set_property("configurable", JSValue::bool(true));
                descriptor
            } else {
                JSValue::undefined()
            }
        }
        _ => JSValue::undefined(),
    }
}

// ============================================================================
// Reflect.ownKeys(target)
// ============================================================================

/// Reflect.ownKeys(target) - Returns an array of all own property keys.
pub fn reflect_own_keys(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());

    match &target {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            let keys: Vec<String> = borrow.properties.keys().cloned().collect();
            let arr = JSValue::object("Array");
            for (i, key) in keys.iter().enumerate() {
                arr.set_property(&i.to_string(), JSValue::string(key));
            }
            arr.set_property("length", JSValue::Int(keys.len() as i32));
            arr
        }
        JSValue::Function(f) => {
            let borrow = f.borrow();
            let keys: Vec<String> = borrow.closure.keys().cloned().collect();
            let arr = JSValue::object("Array");
            for (i, key) in keys.iter().enumerate() {
                arr.set_property(&i.to_string(), JSValue::string(key));
            }
            arr.set_property("length", JSValue::Int(keys.len() as i32));
            arr
        }
        _ => {
            let arr = JSValue::object("Array");
            arr.set_property("length", JSValue::int(0));
            arr
        }
    }
}

// ============================================================================
// Reflect.isExtensible(target)
// ============================================================================

/// Reflect.isExtensible(target)
pub fn reflect_is_extensible(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    // Objects are extensible by default
    match &target {
        JSValue::Object(_) | JSValue::Function(_) => {
            let extensible = target
                .get_property("__extensible__")
                .map(|v| v.to_boolean())
                .unwrap_or(true);
            JSValue::bool(extensible)
        }
        _ => JSValue::bool(false),
    }
}

// ============================================================================
// Reflect.preventExtensions(target)
// ============================================================================

/// Reflect.preventExtensions(target)
pub fn reflect_prevent_extensions(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = args.get(0).cloned().unwrap_or(JSValue::undefined());
    match &target {
        JSValue::Object(_) => {
            target.set_property("__extensible__", JSValue::bool(false));
            JSValue::bool(true)
        }
        _ => JSValue::bool(false),
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Reflect object with all its methods.
pub fn init_reflect(ctx: &mut JSContext) {
    let reflect = JSValue::object("Reflect");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("get", reflect_get),
        ("set", reflect_set),
        ("has", reflect_has),
        ("deleteProperty", reflect_delete_property),
        ("apply", reflect_apply),
        ("construct", reflect_construct),
        ("getPrototypeOf", reflect_get_prototype_of),
        ("setPrototypeOf", reflect_set_prototype_of),
        ("defineProperty", reflect_define_property),
        ("getOwnPropertyDescriptor", reflect_get_own_property_descriptor),
        ("ownKeys", reflect_own_keys),
        ("isExtensible", reflect_is_extensible),
        ("preventExtensions", reflect_prevent_extensions),
    ];

    for &(name, func) in methods {
        reflect.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    ctx.global
        .borrow_mut()
        .properties
        .insert("Reflect".to_string(), reflect);
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
    fn test_reflect_get() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        target.set_property("x", JSValue::int(42));
        let result = reflect_get(&this, &[target, JSValue::string("x")]);
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_reflect_set() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        let result = reflect_set(&this, &[target.clone(), JSValue::string("x"), JSValue::int(99)]);
        assert!(result.to_boolean());
        assert_eq!(target.get_property("x").unwrap().to_int32(), 99);
    }

    #[test]
    fn test_reflect_has() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        target.set_property("x", JSValue::int(1));
        assert!(reflect_has(&this, &[target.clone(), JSValue::string("x")]).to_boolean());
        assert!(!reflect_has(&this, &[target, JSValue::string("y")]).to_boolean());
    }

    #[test]
    fn test_reflect_delete_property() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        target.set_property("x", JSValue::int(1));
        let result = reflect_delete_property(&this, &[target.clone(), JSValue::string("x")]);
        assert!(result.to_boolean());
        assert!(!target.has_property("x"));
    }

    #[test]
    fn test_reflect_apply() {
        let this = JSValue::undefined();
        let func = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::Float(a + b)
            }),
        );
        let args_arr = JSValue::object("Array");
        args_arr.set_property("0", JSValue::int(3));
        args_arr.set_property("1", JSValue::int(4));
        args_arr.set_property("length", JSValue::int(2));

        let result = reflect_apply(&this, &[func, JSValue::undefined(), args_arr]);
        assert_eq!(result.to_number(), 7.0);
    }

    #[test]
    fn test_reflect_construct() {
        let this = JSValue::undefined();
        let constructor = JSValue::function(
            Some("MyClass"),
            vec!["x".to_string()],
            FunctionBody::Native(|_this, args| {
                let x = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
                let obj = JSValue::object("MyClass");
                obj.set_property("x", JSValue::Int(x));
                obj
            }),
        );
        let args_arr = JSValue::object("Array");
        args_arr.set_property("0", JSValue::int(10));
        args_arr.set_property("length", JSValue::int(1));

        let result = reflect_construct(&this, &[constructor, args_arr]);
        assert!(result.is_object());
        assert_eq!(result.get_property("x").unwrap().to_int32(), 10);
    }

    #[test]
    fn test_reflect_get_prototype_of() {
        let this = JSValue::undefined();
        let proto = JSValue::object("Proto");
        let target = JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
            prototype: match &proto {
                JSValue::Object(o) => Some(o.clone()),
                _ => None,
            },
            internal_slots: std::collections::HashMap::new(),
            class_name: "Object".to_string(),
        })));

        let result = reflect_get_prototype_of(&this, &[target]);
        match (&result, &proto) {
            (JSValue::Object(a), JSValue::Object(b)) => assert!(Rc::ptr_eq(a, b)),
            _ => panic!("Expected same prototype"),
        }
    }

    #[test]
    fn test_reflect_own_keys() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        target.set_property("a", JSValue::int(1));
        target.set_property("b", JSValue::int(2));

        let keys = reflect_own_keys(&this, &[target]);
        let length = keys.get_property("length").unwrap().to_int32();
        assert_eq!(length, 2);
    }

    #[test]
    fn test_reflect_define_property() {
        let this = JSValue::undefined();
        let target = JSValue::object("Object");
        let attrs = JSValue::object("Object");
        attrs.set_property("value", JSValue::int(42));

        let result = reflect_define_property(&this, &[target.clone(), JSValue::string("x"), attrs]);
        assert!(result.to_boolean());
        assert_eq!(target.get_property("x").unwrap().to_int32(), 42);
    }

    #[test]
    fn test_init_reflect() {
        let mut ctx = make_ctx();
        init_reflect(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Reflect").is_some());
    }
}
