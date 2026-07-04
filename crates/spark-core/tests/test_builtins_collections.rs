//! Tests for collection builtins: Map, Set, WeakMap, WeakSet

use spark_core::builtins::map::*;
use spark_core::builtins::set::*;
use spark_core::builtins::weakmap::*;
use spark_core::builtins::weakset::*;
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
// Map tests
// ============================================================================

#[test]
fn test_map_get_missing_key() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    let result = map_get(&map, &[JSValue::string("missing")]);
    assert!(result.is_undefined());
}

#[test]
fn test_map_set_updates_existing() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::string("key"), JSValue::int(1)]);
    map_set(&map, &[JSValue::string("key"), JSValue::int(2)]);
    let val = map_get(&map, &[JSValue::string("key")]);
    assert_eq!(val.to_int32(), 2);
}

#[test]
fn test_map_size_empty() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    assert_eq!(map_size(&map, &[]).to_int32(), 0);
}

#[test]
fn test_map_size_after_inserts() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::string("a"), JSValue::int(1)]);
    map_set(&map, &[JSValue::string("b"), JSValue::int(2)]);
    assert_eq!(map_size(&map, &[]).to_int32(), 2);
}

#[test]
fn test_map_clear() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::string("a"), JSValue::int(1)]);
    map_clear(&map, &[]);
    assert_eq!(map_size(&map, &[]).to_int32(), 0);
}

#[test]
fn test_map_has() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    assert!(!map_has(&map, &[JSValue::string("x")]).to_boolean());
    map_set(&map, &[JSValue::string("x"), JSValue::int(1)]);
    assert!(map_has(&map, &[JSValue::string("x")]).to_boolean());
}

#[test]
fn test_map_delete() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::string("x"), JSValue::int(1)]);
    map_delete(&map, &[JSValue::string("x")]);
    assert!(!map_has(&map, &[JSValue::string("x")]).to_boolean());
}

#[test]
fn test_map_nan_key_same() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::float(f64::NAN), JSValue::int(1)]);
    map_set(&map, &[JSValue::float(f64::NAN), JSValue::int(2)]);
    // NaN keys should be treated as the same key
    assert_eq!(map_size(&map, &[]).to_int32(), 1);
}

#[test]
fn test_map_multiple_key_types() {
    let map = map_constructor(&JSValue::undefined(), &[]);
    map_set(&map, &[JSValue::string("str"), JSValue::int(1)]);
    map_set(&map, &[JSValue::int(42), JSValue::int(2)]);
    map_set(&map, &[JSValue::bool(true), JSValue::int(3)]);
    assert_eq!(map_size(&map, &[]).to_int32(), 3);
}

#[test]
fn test_map_on_non_map_object() {
    let not_map = JSValue::object("NotMap");
    let result = map_get(&not_map, &[JSValue::string("x")]);
    assert!(result.is_undefined());
}

#[test]
fn test_map_set_on_non_map_object() {
    let not_map = JSValue::object("NotMap");
    // Should not panic
    map_set(&not_map, &[JSValue::string("x"), JSValue::int(1)]);
}

#[test]
fn test_map_has_on_non_map_object() {
    let not_map = JSValue::object("NotMap");
    let result = map_has(&not_map, &[JSValue::string("x")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_map_delete_on_non_map_object() {
    let not_map = JSValue::object("NotMap");
    // Should not panic
    map_delete(&not_map, &[JSValue::string("x")]);
}

#[test]
fn test_init_map() {
    let mut ctx = make_ctx();
    init_map(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("Map").is_some());
}

// ============================================================================
// Set tests
// ============================================================================

#[test]
fn test_set_size_empty() {
    let set = set_constructor(&JSValue::undefined(), &[]);
    assert_eq!(set_size(&set, &[]).to_int32(), 0);
}

#[test]
fn test_set_size_after_adds() {
    let set = set_constructor(&JSValue::undefined(), &[]);
    set_add(&set, &[JSValue::int(1)]);
    set_add(&set, &[JSValue::int(2)]);
    assert_eq!(set_size(&set, &[]).to_int32(), 2);
}

#[test]
fn test_set_clear() {
    let set = set_constructor(&JSValue::undefined(), &[]);
    set_add(&set, &[JSValue::int(1)]);
    set_clear(&set, &[]);
    assert_eq!(set_size(&set, &[]).to_int32(), 0);
}

#[test]
fn test_set_delete_nonexistent() {
    let set = set_constructor(&JSValue::undefined(), &[]);
    // Deleting from empty set should not panic
    set_delete(&set, &[JSValue::int(99)]);
}

#[test]
fn test_set_nan_same() {
    let set = set_constructor(&JSValue::undefined(), &[]);
    set_add(&set, &[JSValue::float(f64::NAN)]);
    set_add(&set, &[JSValue::float(f64::NAN)]);
    assert_eq!(set_size(&set, &[]).to_int32(), 1);
}

#[test]
fn test_set_on_non_set_object() {
    let not_set = JSValue::object("NotSet");
    set_add(&not_set, &[JSValue::int(1)]);
    assert!(!set_has(&not_set, &[JSValue::int(1)]).to_boolean());
}

#[test]
fn test_set_has_on_non_set_object() {
    let not_set = JSValue::object("NotSet");
    assert!(!set_has(&not_set, &[JSValue::int(1)]).to_boolean());
}

#[test]
fn test_set_delete_on_non_set_object() {
    let not_set = JSValue::object("NotSet");
    set_delete(&not_set, &[JSValue::int(1)]);
}

#[test]
fn test_init_set() {
    let mut ctx = make_ctx();
    init_set(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("Set").is_some());
}

// ============================================================================
// WeakMap tests
// ============================================================================

#[test]
fn test_weakmap_get_missing() {
    let wm = weakmap_constructor(&JSValue::undefined(), &[]);
    let key = JSValue::object("Key");
    let result = weakmap_get(&wm, &[key]);
    assert!(result.is_undefined());
}

#[test]
fn test_weakmap_set_updates() {
    let wm = weakmap_constructor(&JSValue::undefined(), &[]);
    let key = JSValue::object("Key");
    weakmap_set(&wm, &[key.clone(), JSValue::int(1)]);
    weakmap_set(&wm, &[key.clone(), JSValue::int(2)]);
    let val = weakmap_get(&wm, &[key]);
    assert_eq!(val.to_int32(), 2);
}

#[test]
fn test_weakmap_has_on_non_weakmap() {
    let not_wm = JSValue::object("NotWeakMap");
    let result = weakmap_has(&not_wm, &[JSValue::object("Key")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_weakmap_get_on_non_weakmap() {
    let not_wm = JSValue::object("NotWeakMap");
    let result = weakmap_get(&not_wm, &[JSValue::object("Key")]);
    assert!(result.is_undefined());
}

#[test]
fn test_weakmap_set_on_non_weakmap() {
    let not_wm = JSValue::object("NotWeakMap");
    // Should not panic
    weakmap_set(&not_wm, &[JSValue::object("Key"), JSValue::int(1)]);
}

#[test]
fn test_weakmap_delete_on_non_weakmap() {
    let not_wm = JSValue::object("NotWeakMap");
    weakmap_delete(&not_wm, &[JSValue::object("Key")]);
}

#[test]
fn test_weakmap_multiple_keys() {
    let wm = weakmap_constructor(&JSValue::undefined(), &[]);
    let k1 = JSValue::object("K1");
    let k2 = JSValue::object("K2");
    weakmap_set(&wm, &[k1.clone(), JSValue::int(10)]);
    weakmap_set(&wm, &[k2.clone(), JSValue::int(20)]);
    assert_eq!(weakmap_get(&wm, &[k1]).to_int32(), 10);
    assert_eq!(weakmap_get(&wm, &[k2]).to_int32(), 20);
}

#[test]
fn test_init_weakmap() {
    let mut ctx = make_ctx();
    init_weakmap(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("WeakMap").is_some());
}

// ============================================================================
// WeakSet tests
// ============================================================================

#[test]
fn test_weakset_has_empty() {
    let ws = weakset_constructor(&JSValue::undefined(), &[]);
    let obj = JSValue::object("X");
    assert!(!weakset_has(&ws, &[obj]).to_boolean());
}

#[test]
fn test_weakset_add_returns_this() {
    let ws = weakset_constructor(&JSValue::undefined(), &[]);
    let obj = JSValue::object("X");
    let result = weakset_add(&ws, &[obj]);
    // add should return the WeakSet itself for chaining
    match (&ws, &result) {
        (JSValue::Object(a), JSValue::Object(b)) => {
            assert!(Rc::ptr_eq(a, b));
        }
        _ => panic!("Expected same object"),
    }
}

#[test]
fn test_weakset_delete_nonexistent() {
    let ws = weakset_constructor(&JSValue::undefined(), &[]);
    let obj = JSValue::object("X");
    weakset_delete(&ws, &[obj]);
    // Should not panic
}

#[test]
fn test_weakset_on_non_weakset() {
    let not_ws = JSValue::object("NotWeakSet");
    weakset_add(&not_ws, &[JSValue::object("X")]);
    assert!(!weakset_has(&not_ws, &[JSValue::object("X")]).to_boolean());
}

#[test]
fn test_weakset_has_on_non_weakset() {
    let not_ws = JSValue::object("NotWeakSet");
    assert!(!weakset_has(&not_ws, &[JSValue::object("X")]).to_boolean());
}

#[test]
fn test_weakset_delete_on_non_weakset() {
    let not_ws = JSValue::object("NotWeakSet");
    weakset_delete(&not_ws, &[JSValue::object("X")]);
}

#[test]
fn test_init_weakset() {
    let mut ctx = make_ctx();
    init_weakset(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("WeakSet").is_some());
}
