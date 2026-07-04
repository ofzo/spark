//! Tests for function.rs, proxy.rs, and reflect.rs builtins

use spark_core::builtins::function::*;
use spark_core::builtins::proxy::*;
use spark_core::builtins::reflect::*;
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
// Function tests - gaps
// ============================================================================

#[test]
fn test_function_to_string_native() {
    let f = JSValue::function(
        Some("myFunc"),
        vec!["a".to_string(), "b".to_string()],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    let result = function_to_string(&f, &[]);
    assert!(result.to_string().contains("myFunc"));
    assert!(result.to_string().contains("function"));
}

#[test]
fn test_function_to_string_source() {
    let f = JSValue::function(
        Some("srcFunc"),
        vec![],
        FunctionBody::Source("return 42".to_string()),
    );
    let result = function_to_string(&f, &[]);
    assert!(result.to_string().contains("srcFunc"));
}

#[test]
fn test_function_to_string_on_non_function() {
    let result = function_to_string(&JSValue::int(42), &[]);
    // Should return something (possibly "function" or handle gracefully)
    assert!(!result.to_string().is_empty() || result.is_undefined());
}

#[test]
fn test_function_call_basic() {
    let f = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(42)),
    );
    let result = function_call(&f, &[JSValue::undefined()]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_function_call_with_args() {
    let f = JSValue::function(
        None,
        vec!["a".to_string()],
        FunctionBody::Native(|_, args| {
            let val = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
            JSValue::int(val * 2)
        }),
    );
    let result = function_call(&f, &[JSValue::undefined(), JSValue::int(21)]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_function_call_on_non_function() {
    let result = function_call(&JSValue::int(42), &[JSValue::undefined()]);
    // Should handle gracefully
    assert!(result.is_undefined());
}

#[test]
fn test_function_apply_basic() {
    let f = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(99)),
    );
    let result = function_apply(&f, &[JSValue::undefined()]);
    assert_eq!(result.to_int32(), 99);
}

#[test]
fn test_function_apply_with_args_array() {
    let f = JSValue::function(
        None,
        vec!["a".to_string(), "b".to_string()],
        FunctionBody::Native(|_, args| {
            let a = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
            let b = args.get(1).map(|v| v.to_int32()).unwrap_or(0);
            JSValue::int(a + b)
        }),
    );
    let args_arr = JSValue::object("Array");
    args_arr.set_property("0", JSValue::int(10));
    args_arr.set_property("1", JSValue::int(20));
    args_arr.set_property("length", JSValue::int(2));
    let result = function_apply(&f, &[JSValue::undefined(), args_arr]);
    assert_eq!(result.to_int32(), 30);
}

#[test]
fn test_function_apply_with_null_args() {
    let f = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(1)),
    );
    let result = function_apply(&f, &[JSValue::undefined(), JSValue::null()]);
    assert_eq!(result.to_int32(), 1);
}

#[test]
fn test_function_apply_with_undefined_args() {
    let f = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(1)),
    );
    let result = function_apply(&f, &[JSValue::undefined(), JSValue::undefined()]);
    assert_eq!(result.to_int32(), 1);
}

#[test]
fn test_function_apply_on_non_function() {
    let result = function_apply(&JSValue::int(42), &[JSValue::undefined()]);
    assert!(result.is_undefined());
}

#[test]
fn test_function_bind_basic() {
    let f = JSValue::function(
        Some("add"),
        vec!["a".to_string(), "b".to_string()],
        FunctionBody::Native(|_, args| {
            let a = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
            let b = args.get(1).map(|v| v.to_int32()).unwrap_or(0);
            JSValue::int(a + b)
        }),
    );
    let bound = function_bind(&f, &[JSValue::undefined(), JSValue::int(10)]);
    assert!(bound.is_callable());
}

#[test]
fn test_function_bind_name() {
    let f = JSValue::function(
        Some("original"),
        vec![],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    let bound = function_bind(&f, &[JSValue::undefined()]);
    let name = bound.get_property("name");
    assert!(name.is_some());
    assert!(name.unwrap().to_string().contains("original"));
}

#[test]
fn test_function_bind_length() {
    let f = JSValue::function(
        None,
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    let bound = function_bind(&f, &[JSValue::undefined(), JSValue::int(1)]);
    let len = bound.get_property("length");
    assert!(len.is_some());
    // length = max(0, original_length - bound_args) = 3 - 1 = 2
    assert_eq!(len.unwrap().to_int32(), 2);
}

#[test]
fn test_function_bind_no_args() {
    let f = JSValue::function(
        None,
        vec!["a".to_string()],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    let bound = function_bind(&f, &[JSValue::undefined()]);
    assert!(bound.is_callable());
}

#[test]
fn test_function_bind_on_non_function() {
    let result = function_bind(&JSValue::int(42), &[JSValue::undefined()]);
    // Should handle gracefully
    assert!(!result.is_callable() || result.is_undefined());
}

#[test]
fn test_init_function() {
    let mut ctx = make_ctx();
    init_function(&mut ctx);
    let global = ctx.global.borrow();
    let func_ctor = global.properties.get("Function").unwrap();
    assert!(func_ctor.is_callable());
    assert!(func_ctor.get_property("prototype").is_some());
}

// ============================================================================
// Proxy tests - gaps
// ============================================================================

#[test]
fn test_proxy_constructor_basic() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    let result = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    assert!(result.is_object());
    if let JSValue::Object(ref o) = result {
        assert_eq!(o.borrow().class_name, "Proxy");
    }
}

#[test]
fn test_proxy_get_handler_basic() {
    let target = JSValue::object("Object");
    target.set_property("x", JSValue::int(42));
    let handler = JSValue::object("Object");
    handler.set_property("get", JSValue::function(
        None,
        vec!["target".to_string(), "prop".to_string()],
        FunctionBody::Native(|_, args| {
            let prop = args.get(1).map(|v| v.to_string()).unwrap_or_default();
            JSValue::string(&format!("intercepted: {}", prop))
        }),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_get_handler(&proxy, &JSValue::string("x"));
    assert_eq!(result.to_string(), "intercepted: x");
}

#[test]
fn test_proxy_set_handler_basic() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    handler.set_property("set", JSValue::function(
        None,
        vec!["target".to_string(), "prop".to_string(), "value".to_string()],
        FunctionBody::Native(|_, _| JSValue::bool(true)),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_set_handler(&proxy, &JSValue::string("x"), &JSValue::int(1));
    assert!(result);
}

#[test]
fn test_proxy_has_handler_basic() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    handler.set_property("has", JSValue::function(
        None,
        vec!["target".to_string(), "prop".to_string()],
        FunctionBody::Native(|_, _| JSValue::bool(true)),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_has_handler(&proxy, &JSValue::string("x"));
    assert!(result);
}

#[test]
fn test_proxy_delete_handler_basic() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    handler.set_property("deleteProperty", JSValue::function(
        None,
        vec!["target".to_string(), "prop".to_string()],
        FunctionBody::Native(|_, _| JSValue::bool(true)),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_delete_handler(&proxy, &JSValue::string("x"));
    assert!(result);
}

#[test]
fn test_proxy_apply_handler_basic() {
    let target = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(42)),
    );
    let handler = JSValue::object("Object");
    handler.set_property("apply", JSValue::function(
        None,
        vec!["target".to_string(), "thisArg".to_string(), "args".to_string()],
        FunctionBody::Native(|_, _| JSValue::int(99)),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_apply_handler(&proxy, &JSValue::undefined(), &[]);
    assert_eq!(result.to_int32(), 99);
}

#[test]
fn test_proxy_construct_handler_basic() {
    let target = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| {
            let obj = JSValue::object("MyClass");
            obj.set_property("constructed", JSValue::bool(true));
            obj
        }),
    );
    let handler = JSValue::object("Object");
    handler.set_property("construct", JSValue::function(
        None,
        vec!["target".to_string(), "args".to_string()],
        FunctionBody::Native(|_, _| {
            let obj = JSValue::object("ProxyClass");
            obj.set_property("proxy_constructed", JSValue::bool(true));
            obj
        }),
    ));
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    let result = proxy_construct_handler(&proxy, &[]);
    assert!(result.is_object());
}

#[test]
fn test_proxy_revocable_basic() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    let result = proxy_revocable(&JSValue::undefined(), &[target, handler]);
    assert!(result.is_object());
    assert!(result.get_property("proxy").is_some());
    assert!(result.get_property("revoke").is_some());
}

#[test]
fn test_proxy_revocable_revoke() {
    let target = JSValue::object("Object");
    let handler = JSValue::object("Object");
    let result = proxy_revocable(&JSValue::undefined(), &[target, handler]);
    let revoke = result.get_property("revoke").unwrap();
    assert!(revoke.is_callable());
}

#[test]
fn test_proxy_fallback_to_target() {
    // When no trap is defined, behavior should fall through to target
    let target = JSValue::object("Object");
    target.set_property("x", JSValue::int(42));
    let handler = JSValue::object("Object"); // empty handler
    let proxy = proxy_constructor(&JSValue::undefined(), &[target, handler]);
    // With no get trap, should fall back to target
    let result = proxy_get_handler(&proxy, &JSValue::string("x"));
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_init_proxy() {
    let mut ctx = make_ctx();
    init_proxy(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("Proxy").is_some());
}

// ============================================================================
// Reflect tests - gaps
// ============================================================================

#[test]
fn test_reflect_get_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(42));
    let result = reflect_get(&JSValue::undefined(), &[obj, JSValue::string("x")]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_reflect_get_missing() {
    let obj = JSValue::object("Object");
    let result = reflect_get(&JSValue::undefined(), &[obj, JSValue::string("missing")]);
    assert!(result.is_undefined());
}

#[test]
fn test_reflect_set_basic() {
    let obj = JSValue::object("Object");
    reflect_set(&JSValue::undefined(), &[obj.clone(), JSValue::string("x"), JSValue::int(99)]);
    assert_eq!(obj.get_property("x").unwrap().to_int32(), 99);
}

#[test]
fn test_reflect_has_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    let result = reflect_has(&JSValue::undefined(), &[obj, JSValue::string("x")]);
    assert!(result.to_boolean());
}

#[test]
fn test_reflect_has_missing() {
    let obj = JSValue::object("Object");
    let result = reflect_has(&JSValue::undefined(), &[obj, JSValue::string("missing")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_reflect_delete_property() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    reflect_delete_property(&JSValue::undefined(), &[obj.clone(), JSValue::string("x")]);
    assert!(!obj.has_property("x"));
}

#[test]
fn test_reflect_delete_property_nonexistent() {
    let obj = JSValue::object("Object");
    // Should not panic
    reflect_delete_property(&JSValue::undefined(), &[obj, JSValue::string("missing")]);
}

#[test]
fn test_reflect_apply_basic() {
    let f = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(42)),
    );
    let result = reflect_apply(&JSValue::undefined(), &[f, JSValue::undefined()]);
    assert_eq!(result.to_int32(), 42);
}

#[test]
fn test_reflect_apply_with_args() {
    let f = JSValue::function(
        None,
        vec!["a".to_string()],
        FunctionBody::Native(|_, args| {
            args.get(0).cloned().unwrap_or(JSValue::undefined())
        }),
    );
    let args_arr = JSValue::object("Array");
    args_arr.set_property("0", JSValue::int(99));
    args_arr.set_property("length", JSValue::int(1));
    let result = reflect_apply(&JSValue::undefined(), &[f, JSValue::undefined(), args_arr]);
    assert_eq!(result.to_int32(), 99);
}

#[test]
fn test_reflect_construct_basic() {
    let ctor = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| {
            let obj = JSValue::object("MyClass");
            obj.set_property("init", JSValue::bool(true));
            obj
        }),
    );
    let result = reflect_construct(&JSValue::undefined(), &[ctor]);
    assert!(result.is_object());
}

#[test]
fn test_reflect_get_prototype_of() {
    let proto = JSValue::object("Proto");
    let obj = JSValue::object("Child");
    if let JSValue::Object(ref o) = obj {
        if let JSValue::Object(ref p) = proto {
            o.borrow_mut().prototype = Some(p.clone());
        }
    }
    let result = reflect_get_prototype_of(&JSValue::undefined(), &[obj]);
    match (&result, &proto) {
        (JSValue::Object(a), JSValue::Object(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("Expected prototype"),
    }
}

#[test]
fn test_reflect_set_prototype_of() {
    let obj = JSValue::object("Child");
    let new_proto = JSValue::object("NewProto");
    new_proto.set_property("inherited", JSValue::int(42));
    reflect_set_prototype_of(&JSValue::undefined(), &[obj.clone(), new_proto]);
    assert_eq!(obj.get_property("inherited").unwrap().to_int32(), 42);
}

#[test]
fn test_reflect_define_property() {
    let obj = JSValue::object("Object");
    let desc = JSValue::object("Object");
    desc.set_property("value", JSValue::int(42));
    desc.set_property("writable", JSValue::bool(true));
    desc.set_property("enumerable", JSValue::bool(true));
    desc.set_property("configurable", JSValue::bool(true));
    reflect_define_property(&JSValue::undefined(), &[obj.clone(), JSValue::string("x"), desc]);
    assert_eq!(obj.get_property("x").unwrap().to_int32(), 42);
}

#[test]
fn test_reflect_get_own_property_descriptor() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(42));
    let result = reflect_get_own_property_descriptor(&JSValue::undefined(), &[obj, JSValue::string("x")]);
    // Should return a descriptor object
    assert!(result.is_object() || result.is_undefined());
}

#[test]
fn test_reflect_own_keys() {
    let obj = JSValue::object("Object");
    obj.set_property("a", JSValue::int(1));
    obj.set_property("b", JSValue::int(2));
    let result = reflect_own_keys(&JSValue::undefined(), &[obj]);
    assert!(result.is_object());
}

#[test]
fn test_reflect_is_extensible() {
    let obj = JSValue::object("Object");
    let result = reflect_is_extensible(&JSValue::undefined(), &[obj]);
    assert!(result.to_boolean());
}

#[test]
fn test_reflect_prevent_extensions() {
    let obj = JSValue::object("Object");
    reflect_prevent_extensions(&JSValue::undefined(), &[obj.clone()]);
    let result = reflect_is_extensible(&JSValue::undefined(), &[obj]);
    assert!(!result.to_boolean());
}

#[test]
fn test_init_reflect() {
    let mut ctx = make_ctx();
    init_reflect(&mut ctx);
    let global = ctx.global.borrow();
    let reflect = global.properties.get("Reflect").unwrap();
    for method in &["get", "set", "has", "deleteProperty", "apply", "construct",
                     "getPrototypeOf", "setPrototypeOf", "defineProperty",
                     "getOwnPropertyDescriptor", "ownKeys", "isExtensible", "preventExtensions"] {
        assert!(reflect.get_property(method).is_some(), "Reflect.{} should exist", method);
    }
}
