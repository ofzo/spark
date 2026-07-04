//! Tests for object.rs and array.rs builtins

use spark_core::builtins::object::*;
use spark_core::builtins::array::*;
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
// Object tests - untested methods
// ============================================================================

#[test]
fn test_object_constructor_with_string() {
    let result = object_constructor(&JSValue::undefined(), &[JSValue::string("hello")]);
    // Object("hello") returns a string wrapper object
    assert!(result.is_object());
}

#[test]
fn test_object_constructor_with_object() {
    let obj = JSValue::object("X");
    let result = object_constructor(&JSValue::undefined(), &[obj.clone()]);
    // Object(obj) returns the same object
    match (&result, &obj) {
        (JSValue::Object(a), JSValue::Object(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("Expected same object"),
    }
}

#[test]
fn test_object_constructor_no_args() {
    let result = object_constructor(&JSValue::undefined(), &[]);
    assert!(result.is_object());
}

#[test]
fn test_object_constructor_with_null() {
    let result = object_constructor(&JSValue::undefined(), &[JSValue::null()]);
    assert!(result.is_object());
}

#[test]
fn test_object_constructor_with_undefined() {
    let result = object_constructor(&JSValue::undefined(), &[JSValue::undefined()]);
    assert!(result.is_object());
}

#[test]
fn test_object_constructor_with_number() {
    let result = object_constructor(&JSValue::undefined(), &[JSValue::int(42)]);
    assert!(result.is_object());
}

#[test]
fn test_object_constructor_with_bool() {
    let result = object_constructor(&JSValue::undefined(), &[JSValue::bool(true)]);
    assert!(result.is_object());
}

#[test]
fn test_object_is_nan() {
    let result = object_is(&JSValue::undefined(), &[
        JSValue::float(f64::NAN),
        JSValue::float(f64::NAN),
    ]);
    assert!(result.to_boolean());
}

#[test]
fn test_object_is_positive_negative_zero() {
    let result = object_is(&JSValue::undefined(), &[
        JSValue::float(0.0),
        JSValue::float(-0.0),
    ]);
    assert!(!result.to_boolean());
}

#[test]
fn test_object_is_same_value() {
    let result = object_is(&JSValue::undefined(), &[
        JSValue::int(42),
        JSValue::int(42),
    ]);
    assert!(result.to_boolean());
}

#[test]
fn test_object_is_different_values() {
    let result = object_is(&JSValue::undefined(), &[
        JSValue::int(1),
        JSValue::int(2),
    ]);
    assert!(!result.to_boolean());
}

#[test]
fn test_object_assign_multiple_sources() {
    let target = JSValue::object("Object");
    target.set_property("a", JSValue::int(1));

    let source1 = JSValue::object("Object");
    source1.set_property("b", JSValue::int(2));

    let source2 = JSValue::object("Object");
    source2.set_property("c", JSValue::int(3));

    object_assign(&JSValue::undefined(), &[target.clone(), source1, source2]);
    assert_eq!(target.get_property("a").unwrap().to_int32(), 1);
    assert_eq!(target.get_property("b").unwrap().to_int32(), 2);
    assert_eq!(target.get_property("c").unwrap().to_int32(), 3);
}

#[test]
fn test_object_keys_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("a", JSValue::int(1));
    obj.set_property("b", JSValue::int(2));
    let keys = object_keys(&JSValue::undefined(), &[obj]);
    assert!(keys.is_object());
}

#[test]
fn test_object_values_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(10));
    let vals = object_values(&JSValue::undefined(), &[obj]);
    assert!(vals.is_object());
}

#[test]
fn test_object_entries_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("k", JSValue::string("v"));
    let entries = object_entries(&JSValue::undefined(), &[obj]);
    assert!(entries.is_object());
}

#[test]
fn test_object_get_own_property_names() {
    let obj = JSValue::object("Object");
    obj.set_property("a", JSValue::int(1));
    obj.set_property("b", JSValue::int(2));
    let names = object_get_own_property_names(&JSValue::undefined(), &[obj]);
    assert!(names.is_object());
}

#[test]
fn test_object_get_prototype_of() {
    let proto = JSValue::object("Proto");
    let obj = JSValue::object("Child");
    if let JSValue::Object(ref o) = obj {
        if let JSValue::Object(ref p) = proto {
            o.borrow_mut().prototype = Some(p.clone());
        }
    }
    let result = object_get_prototype_of(&JSValue::undefined(), &[obj]);
    match (&result, &proto) {
        (JSValue::Object(a), JSValue::Object(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("Expected prototype"),
    }
}

#[test]
fn test_object_set_prototype_of() {
    let obj = JSValue::object("Child");
    let proto = JSValue::object("Proto");
    proto.set_property("inherited", JSValue::int(42));
    object_set_prototype_of(&JSValue::undefined(), &[obj.clone(), proto]);
    assert_eq!(obj.get_property("inherited").unwrap().to_int32(), 42);
}

#[test]
fn test_object_create_with_null_proto() {
    let result = object_create(&JSValue::undefined(), &[JSValue::null()]);
    assert!(result.is_object());
    if let JSValue::Object(ref o) = result {
        assert!(o.borrow().prototype.is_none());
    }
}

#[test]
fn test_object_define_property_basic() {
    let obj = JSValue::object("Object");
    let desc = JSValue::object("Object");
    desc.set_property("value", JSValue::int(42));
    desc.set_property("writable", JSValue::bool(true));
    desc.set_property("enumerable", JSValue::bool(true));
    desc.set_property("configurable", JSValue::bool(true));
    object_define_property(&JSValue::undefined(), &[obj.clone(), JSValue::string("x"), desc]);
    assert_eq!(obj.get_property("x").unwrap().to_int32(), 42);
}

#[test]
fn test_object_freeze_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    object_freeze(&JSValue::undefined(), &[obj.clone()]);
    // After freeze, trying to add new properties should fail
    // (behavior depends on implementation)
}

#[test]
fn test_object_seal_basic() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    object_seal(&JSValue::undefined(), &[obj.clone()]);
}

#[test]
fn test_object_prevent_extensions_basic() {
    let obj = JSValue::object("Object");
    object_prevent_extensions(&JSValue::undefined(), &[obj.clone()]);
}

#[test]
fn test_object_is_frozen() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    object_freeze(&JSValue::undefined(), &[obj.clone()]);
    let result = object_is_frozen(&JSValue::undefined(), &[obj]);
    assert!(result.to_boolean());
}

#[test]
fn test_object_is_sealed() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    object_seal(&JSValue::undefined(), &[obj.clone()]);
    let result = object_is_sealed(&JSValue::undefined(), &[obj]);
    assert!(result.to_boolean());
}

#[test]
fn test_object_is_extensible_default() {
    let obj = JSValue::object("Object");
    let result = object_is_extensible(&JSValue::undefined(), &[obj]);
    // Default: objects are extensible
    assert!(result.to_boolean());
}

#[test]
fn test_object_has_own() {
    let obj = JSValue::object("Object");
    obj.set_property("x", JSValue::int(1));
    let result = object_has_own(&JSValue::undefined(), &[obj, JSValue::string("x")]);
    assert!(result.to_boolean());
}

#[test]
fn test_object_has_own_missing() {
    let obj = JSValue::object("Object");
    let result = object_has_own(&JSValue::undefined(), &[obj, JSValue::string("missing")]);
    assert!(!result.to_boolean());
}

#[test]
fn test_object_from_entries() {
    let arr = JSValue::object("Array");
    let entry0 = JSValue::object("Array");
    entry0.set_property("0", JSValue::string("a"));
    entry0.set_property("1", JSValue::int(1));
    entry0.set_property("length", JSValue::int(2));
    arr.set_property("0", entry0);
    arr.set_property("length", JSValue::int(1));

    let result = object_from_entries(&JSValue::undefined(), &[arr]);
    assert!(result.is_object());
}

#[test]
fn test_init_object() {
    let mut ctx = make_ctx();
    init_object(&mut ctx);
    let global = ctx.global.borrow();
    let obj_ctor = global.properties.get("Object").unwrap();
    assert!(obj_ctor.is_callable());
    assert!(obj_ctor.get_property("keys").is_some());
    assert!(obj_ctor.get_property("values").is_some());
    assert!(obj_ctor.get_property("entries").is_some());
    assert!(obj_ctor.get_property("assign").is_some());
    assert!(obj_ctor.get_property("create").is_some());
    assert!(obj_ctor.get_property("defineProperty").is_some());
    assert!(obj_ctor.get_property("getOwnPropertyDescriptor").is_some());
    assert!(obj_ctor.get_property("getOwnPropertyNames").is_some());
    assert!(obj_ctor.get_property("getPrototypeOf").is_some());
    assert!(obj_ctor.get_property("setPrototypeOf").is_some());
    assert!(obj_ctor.get_property("is").is_some());
    assert!(obj_ctor.get_property("freeze").is_some());
    assert!(obj_ctor.get_property("seal").is_some());
    assert!(obj_ctor.get_property("preventExtensions").is_some());
    assert!(obj_ctor.get_property("isFrozen").is_some());
    assert!(obj_ctor.get_property("isSealed").is_some());
    assert!(obj_ctor.get_property("isExtensible").is_some());
    assert!(obj_ctor.get_property("fromEntries").is_some());
    assert!(obj_ctor.get_property("hasOwn").is_some());
    assert!(obj_ctor.get_property("groupBy").is_some());
}

// ============================================================================
// Array tests - untested methods
// ============================================================================

#[test]
fn test_array_helper_create_and_length() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    assert_eq!(array_length(&arr), 3);
}

#[test]
fn test_array_helper_get() {
    let arr = create_array(vec![JSValue::int(10), JSValue::int(20)]);
    assert_eq!(array_get(&arr, 0).to_int32(), 10);
    assert_eq!(array_get(&arr, 1).to_int32(), 20);
}

#[test]
fn test_array_helper_set() {
    let arr = create_array(vec![JSValue::int(0)]);
    array_set(&arr, 0, JSValue::int(99));
    assert_eq!(array_get(&arr, 0).to_int32(), 99);
}

#[test]
fn test_array_helper_delete() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    array_delete(&arr, 0);
    // After delete, element should be undefined
    assert!(array_get(&arr, 0).is_undefined());
}

#[test]
fn test_array_constructor_no_args() {
    let arr = array_constructor(&JSValue::undefined(), &[]);
    assert!(arr.is_object());
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 0);
}

#[test]
fn test_array_constructor_with_length() {
    let arr = array_constructor(&JSValue::undefined(), &[JSValue::int(5)]);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 5);
}

#[test]
fn test_array_constructor_with_elements() {
    let arr = array_constructor(&JSValue::undefined(), &[JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_array_is_array_true() {
    let arr = create_array(vec![]);
    let result = array_is_array(&JSValue::undefined(), &[arr]);
    assert!(result.to_boolean());
}

#[test]
fn test_array_is_array_false() {
    let obj = JSValue::object("Object");
    let result = array_is_array(&JSValue::undefined(), &[obj]);
    assert!(!result.to_boolean());
}

#[test]
fn test_array_of() {
    let result = array_of(&JSValue::undefined(), &[JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    assert!(result.is_object());
    assert_eq!(result.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_array_from() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    let result = array_from(&JSValue::undefined(), &[arr]);
    assert!(result.is_object());
}

#[test]
fn test_array_push_basic() {
    let arr = create_array(vec![]);
    array_push(&arr, &[JSValue::int(1)]);
    array_push(&arr, &[JSValue::int(2)]);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_array_pop_basic() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    let popped = array_pop(&arr, &[]);
    assert_eq!(popped.to_int32(), 2);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 1);
}

#[test]
fn test_array_pop_empty() {
    let arr = create_array(vec![]);
    let popped = array_pop(&arr, &[]);
    assert!(popped.is_undefined());
}

#[test]
fn test_array_shift_basic() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let shifted = array_shift(&arr, &[]);
    assert_eq!(shifted.to_int32(), 1);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_array_shift_empty() {
    let arr = create_array(vec![]);
    let shifted = array_shift(&arr, &[]);
    assert!(shifted.is_undefined());
}

#[test]
fn test_array_unshift_basic() {
    let arr = create_array(vec![JSValue::int(3)]);
    array_unshift(&arr, &[JSValue::int(1), JSValue::int(2)]);
    assert_eq!(arr.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_array_includes_found() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let result = array_includes(&arr, &[JSValue::int(2)]);
    assert!(result.to_boolean());
}

#[test]
fn test_array_includes_not_found() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    let result = array_includes(&arr, &[JSValue::int(99)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_array_includes_nan() {
    let arr = create_array(vec![JSValue::float(f64::NAN)]);
    let result = array_includes(&arr, &[JSValue::float(f64::NAN)]);
    assert!(result.to_boolean()); // includes uses SameValueZero
}

#[test]
fn test_array_index_of() {
    let arr = create_array(vec![JSValue::int(10), JSValue::int(20), JSValue::int(30)]);
    let result = array_index_of(&arr, &[JSValue::int(20)]);
    assert_eq!(result.to_int32(), 1);
}

#[test]
fn test_array_index_of_not_found() {
    let arr = create_array(vec![JSValue::int(10)]);
    let result = array_index_of(&arr, &[JSValue::int(99)]);
    assert_eq!(result.to_int32(), -1);
}

#[test]
fn test_array_last_index_of() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(1)]);
    let result = array_last_index_of(&arr, &[JSValue::int(1)]);
    assert_eq!(result.to_int32(), 2);
}

#[test]
fn test_array_reverse() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let reversed = array_reverse(&arr, &[]);
    assert_eq!(reversed.get_property("0").unwrap().to_int32(), 3);
    assert_eq!(reversed.get_property("1").unwrap().to_int32(), 2);
    assert_eq!(reversed.get_property("2").unwrap().to_int32(), 1);
}

#[test]
fn test_array_slice_basic() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3), JSValue::int(4)]);
    let sliced = array_slice(&arr, &[JSValue::int(1), JSValue::int(3)]);
    assert_eq!(sliced.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_array_slice_negative_start() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let sliced = array_slice(&arr, &[JSValue::int(-2)]);
    assert_eq!(sliced.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_array_splice_remove() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let removed = array_splice(&arr, &[JSValue::int(1), JSValue::int(1)]);
    assert_eq!(removed.get_property("0").unwrap().to_int32(), 2);
}

#[test]
fn test_array_concat() {
    let arr1 = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    let arr2 = create_array(vec![JSValue::int(3), JSValue::int(4)]);
    let result = array_concat(&arr1, &[arr2]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 4);
}

#[test]
fn test_array_join_default() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let result = array_join(&arr, &[]);
    assert_eq!(result.to_string(), "1,2,3");
}

#[test]
fn test_array_join_custom_separator() {
    let arr = create_array(vec![JSValue::string("a"), JSValue::string("b")]);
    let result = array_join(&arr, &[JSValue::string("-")]);
    assert_eq!(result.to_string(), "a-b");
}

#[test]
fn test_array_join_empty() {
    let arr = create_array(vec![]);
    let result = array_join(&arr, &[]);
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_array_flat() {
    let inner = create_array(vec![JSValue::int(2), JSValue::int(3)]);
    let outer = create_array(vec![JSValue::int(1), inner]);
    let result = array_flat(&outer, &[]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 3);
}

#[test]
fn test_array_fill() {
    let arr = create_array(vec![JSValue::int(0), JSValue::int(0), JSValue::int(0)]);
    let result = array_fill(&arr, &[JSValue::int(7)]);
    assert_eq!(result.get_property("0").unwrap().to_int32(), 7);
    assert_eq!(result.get_property("1").unwrap().to_int32(), 7);
    assert_eq!(result.get_property("2").unwrap().to_int32(), 7);
}

#[test]
fn test_array_copy_within() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3), JSValue::int(4), JSValue::int(5)]);
    array_copy_within(&arr, &[JSValue::int(0), JSValue::int(3)]);
    assert_eq!(arr.get_property("0").unwrap().to_int32(), 4);
    assert_eq!(arr.get_property("1").unwrap().to_int32(), 5);
}

#[test]
fn test_array_sort_basic() {
    let arr = create_array(vec![JSValue::int(3), JSValue::int(1), JSValue::int(2)]);
    let result = array_sort(&arr, &[]);
    assert_eq!(result.get_property("0").unwrap().to_int32(), 1);
    assert_eq!(result.get_property("1").unwrap().to_int32(), 2);
    assert_eq!(result.get_property("2").unwrap().to_int32(), 3);
}

#[test]
fn test_array_at_positive() {
    let arr = create_array(vec![JSValue::int(10), JSValue::int(20), JSValue::int(30)]);
    let result = array_at(&arr, &[JSValue::int(1)]);
    assert_eq!(result.to_int32(), 20);
}

#[test]
fn test_array_at_negative() {
    let arr = create_array(vec![JSValue::int(10), JSValue::int(20), JSValue::int(30)]);
    let result = array_at(&arr, &[JSValue::int(-1)]);
    assert_eq!(result.to_int32(), 30);
}

#[test]
fn test_array_to_reversed() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let result = array_to_reversed(&arr, &[]);
    assert_eq!(result.get_property("0").unwrap().to_int32(), 3);
    assert_eq!(result.get_property("2").unwrap().to_int32(), 1);
}

#[test]
fn test_array_to_sorted() {
    let arr = create_array(vec![JSValue::int(3), JSValue::int(1), JSValue::int(2)]);
    let result = array_to_sorted(&arr, &[]);
    assert_eq!(result.get_property("0").unwrap().to_int32(), 1);
    assert_eq!(result.get_property("1").unwrap().to_int32(), 2);
    assert_eq!(result.get_property("2").unwrap().to_int32(), 3);
}

#[test]
fn test_array_to_spliced() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let result = array_to_spliced(&arr, &[JSValue::int(1), JSValue::int(1)]);
    assert_eq!(result.get_property("length").unwrap().to_int32(), 2);
}

#[test]
fn test_array_with() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
    let result = array_with(&arr, &[JSValue::int(1), JSValue::int(99)]);
    assert_eq!(result.get_property("1").unwrap().to_int32(), 99);
}

#[test]
fn test_array_to_string() {
    let arr = create_array(vec![JSValue::int(1), JSValue::int(2)]);
    let result = array_to_string(&arr, &[]);
    assert_eq!(result.to_string(), "1,2");
}

#[test]
fn test_array_to_string_empty() {
    let arr = create_array(vec![]);
    let result = array_to_string(&arr, &[]);
    assert_eq!(result.to_string(), "");
}

#[test]
fn test_init_array() {
    let mut ctx = make_ctx();
    init_array(&mut ctx);
    let global = ctx.global.borrow();
    let arr_ctor = global.properties.get("Array").unwrap();
    assert!(arr_ctor.is_callable());
    assert!(arr_ctor.get_property("isArray").is_some());
    assert!(arr_ctor.get_property("from").is_some());
    assert!(arr_ctor.get_property("of").is_some());
}
