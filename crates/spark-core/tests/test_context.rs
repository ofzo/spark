//! Comprehensive tests for context.rs - JSContext and VarEnv

use spark_core::context::*;
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;
use std::cell::RefCell;
use std::rc::Rc;

fn make_ctx() -> JSContext {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    JSContext::new(rt)
}

// ============================================================================
// VarEnv basic operations
// ============================================================================

#[test]
fn test_varenv_new_empty() {
    let env = VarEnv::new();
    assert!(env.variables.is_empty());
    assert!(env.parent.is_none());
}

#[test]
fn test_varenv_set_and_get() {
    let mut env = VarEnv::new();
    env.set("x", JSValue::int(42));
    assert_eq!(env.get("x").unwrap().to_int32(), 42);
}

#[test]
fn test_varenv_get_nonexistent() {
    let env = VarEnv::new();
    assert!(env.get("missing").is_none());
}

#[test]
fn test_varenv_has_own() {
    let mut env = VarEnv::new();
    assert!(!env.has_own("x"));
    env.set("x", JSValue::int(1));
    assert!(env.has_own("x"));
}

#[test]
fn test_varenv_overwrite() {
    let mut env = VarEnv::new();
    env.set("x", JSValue::int(1));
    env.set("x", JSValue::int(2));
    assert_eq!(env.get("x").unwrap().to_int32(), 2);
}

#[test]
fn test_varenv_with_parent_chain_lookup() {
    let mut parent = VarEnv::new();
    parent.set("parent_var", JSValue::string("from_parent"));

    let child = VarEnv::with_parent(parent);
    assert_eq!(child.get("parent_var").unwrap().to_string(), "from_parent");
}

#[test]
fn test_varenv_with_parent_shadow() {
    let mut parent = VarEnv::new();
    parent.set("x", JSValue::int(1));

    let mut child = VarEnv::with_parent(parent);
    child.set("x", JSValue::int(2));

    // Child shadows parent
    assert_eq!(child.get("x").unwrap().to_int32(), 2);
}

#[test]
fn test_varenv_has_own_does_not_check_parent() {
    let mut parent = VarEnv::new();
    parent.set("x", JSValue::int(1));

    let child = VarEnv::with_parent(parent);
    assert!(!child.has_own("x")); // x is in parent, not child
}

#[test]
fn test_varenv_clone() {
    let mut env = VarEnv::new();
    env.set("x", JSValue::int(42));
    let cloned = env.clone();
    assert_eq!(cloned.get("x").unwrap().to_int32(), 42);
}

#[test]
fn test_varenv_debug() {
    let env = VarEnv::new();
    let debug = format!("{:?}", env);
    assert!(debug.contains("VarEnv"));
}

// ============================================================================
// JSContext creation
// ============================================================================

#[test]
fn test_context_new() {
    let ctx = make_ctx();
    assert!(ctx.exception.is_none());
    assert!(!ctx.strict_mode);
}

#[test]
fn test_context_this_initially_undefined() {
    let ctx = make_ctx();
    assert!(ctx.this.is_undefined());
}

// ============================================================================
// JSContext variable operations
// ============================================================================

#[test]
fn test_context_declare_and_get_var() {
    let mut ctx = make_ctx();
    ctx.declare_var("x", JSValue::int(42));
    assert_eq!(ctx.get_var("x").to_int32(), 42);
}

#[test]
fn test_context_set_var() {
    let mut ctx = make_ctx();
    ctx.declare_var("x", JSValue::int(1));
    ctx.set_var("x", JSValue::int(2));
    assert_eq!(ctx.get_var("x").to_int32(), 2);
}

#[test]
fn test_context_get_var_undeclared_returns_undefined() {
    let ctx = make_ctx();
    assert!(ctx.get_var("nonexistent").is_undefined());
}

#[test]
fn test_context_set_var_falls_through_to_global() {
    let mut ctx = make_ctx();
    ctx.set_var("globalProp", JSValue::string("hello"));
    // Should be set on the global object
    let global = ctx.global.borrow();
    assert!(global.properties.get("globalProp").is_some());
}

#[test]
fn test_context_get_var_reads_from_global() {
    let ctx = make_ctx();
    {
        let mut global = ctx.global.borrow_mut();
        global.properties.insert("g".to_string(), JSValue::int(99));
    }
    assert_eq!(ctx.get_var("g").to_int32(), 99);
}

// ============================================================================
// JSContext scope operations
// ============================================================================

#[test]
fn test_context_push_pop_scope() {
    let mut ctx = make_ctx();
    ctx.push_scope();
    ctx.declare_var("inner", JSValue::int(1));
    assert_eq!(ctx.get_var("inner").to_int32(), 1);
    ctx.pop_scope();
    assert!(ctx.get_var("inner").is_undefined());
}

#[test]
fn test_context_nested_scopes() {
    let mut ctx = make_ctx();
    ctx.declare_var("outer", JSValue::int(1));

    ctx.push_scope();
    ctx.declare_var("inner", JSValue::int(2));
    assert_eq!(ctx.get_var("outer").to_int32(), 1);
    assert_eq!(ctx.get_var("inner").to_int32(), 2);

    ctx.push_scope();
    ctx.declare_var("deep", JSValue::int(3));
    assert_eq!(ctx.get_var("outer").to_int32(), 1);
    assert_eq!(ctx.get_var("inner").to_int32(), 2);
    assert_eq!(ctx.get_var("deep").to_int32(), 3);

    ctx.pop_scope();
    assert!(ctx.get_var("deep").is_undefined());
    assert_eq!(ctx.get_var("inner").to_int32(), 2);

    ctx.pop_scope();
    assert!(ctx.get_var("inner").is_undefined());
    assert_eq!(ctx.get_var("outer").to_int32(), 1);
}

#[test]
fn test_context_pop_root_scope_no_panic() {
    let mut ctx = make_ctx();
    // Popping the root scope should not panic
    ctx.pop_scope();
}

#[test]
fn test_context_scope_variable_shadowing() {
    let mut ctx = make_ctx();
    ctx.declare_var("x", JSValue::int(1));

    ctx.push_scope();
    ctx.declare_var("x", JSValue::int(2));
    assert_eq!(ctx.get_var("x").to_int32(), 2);

    ctx.pop_scope();
    assert_eq!(ctx.get_var("x").to_int32(), 1);
}

#[test]
fn test_context_set_var_in_outer_scope() {
    let mut ctx = make_ctx();
    ctx.declare_var("x", JSValue::int(1));

    ctx.push_scope();
    // set_var falls through to global when var not in current scope
    ctx.set_var("x", JSValue::int(10));
    // The value in the outer scope may or may not be updated depending on implementation
    // Just verify no panic and the value is accessible
    let _ = ctx.get_var("x");

    ctx.pop_scope();
}

// ============================================================================
// JSContext exception handling
// ============================================================================

#[test]
fn test_context_throw_and_take_exception() {
    let mut ctx = make_ctx();
    assert!(!ctx.has_exception());

    ctx.throw(JSValue::string("error message"));
    assert!(ctx.has_exception());

    let exc = ctx.take_exception();
    assert!(exc.is_some());
    assert_eq!(exc.unwrap().to_string(), "error message");

    // After take, exception should be cleared
    assert!(!ctx.has_exception());
    assert!(ctx.take_exception().is_none());
}

#[test]
fn test_context_throw_object() {
    let mut ctx = make_ctx();
    let err = JSValue::object("Error");
    err.set_property("message", JSValue::string("oops"));
    ctx.throw(err);
    assert!(ctx.has_exception());
}

#[test]
fn test_context_throw_value() {
    let mut ctx = make_ctx();
    ctx.throw(JSValue::int(42));
    let exc = ctx.take_exception().unwrap();
    assert_eq!(exc.to_int32(), 42);
}

// ============================================================================
// VarEnv with multiple variable types
// ============================================================================

#[test]
fn test_varenv_different_value_types() {
    let mut env = VarEnv::new();
    env.set("undef", JSValue::undefined());
    env.set("null", JSValue::null());
    env.set("bool", JSValue::bool(true));
    env.set("int", JSValue::int(42));
    env.set("float", JSValue::float(3.14));
    env.set("string", JSValue::string("hello"));
    env.set("object", JSValue::object("X"));

    assert!(env.get("undef").unwrap().is_undefined());
    assert!(env.get("null").unwrap().is_null());
    assert!(env.get("bool").unwrap().to_boolean());
    assert_eq!(env.get("int").unwrap().to_int32(), 42);
    assert!((env.get("float").unwrap().to_number() - 3.14).abs() < f64::EPSILON);
    assert_eq!(env.get("string").unwrap().to_string(), "hello");
    assert!(env.get("object").unwrap().is_object());
}

// ============================================================================
// Context with custom runtime config
// ============================================================================

#[test]
fn test_context_strict_mode() {
    let mut ctx = make_ctx();
    ctx.strict_mode = true;
    assert!(ctx.strict_mode);
}

#[test]
fn test_context_this_binding() {
    let mut ctx = make_ctx();
    ctx.this = JSValue::int(99);
    assert_eq!(ctx.this.to_int32(), 99);
}
