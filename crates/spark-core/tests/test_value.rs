//! Comprehensive tests for value.rs - JSValue, JSObject, JSFunction, BytecodeFunction

use spark_core::value::*;

// ============================================================================
// JSValue constructors
// ============================================================================

#[test]
fn test_jsvalue_undefined() {
    let v = JSValue::undefined();
    assert!(v.is_undefined());
    assert!(!v.is_null());
    assert_eq!(v.type_of(), "undefined");
}

#[test]
fn test_jsvalue_null() {
    let v = JSValue::null();
    assert!(!v.is_undefined());
    assert!(v.is_null());
    assert_eq!(v.type_of(), "object");
}

#[test]
fn test_jsvalue_bool_true() {
    let v = JSValue::bool(true);
    assert!(!v.is_undefined());
    assert!(!v.is_null());
    assert_eq!(v.type_of(), "boolean");
    assert!(v.to_boolean());
}

#[test]
fn test_jsvalue_bool_false() {
    let v = JSValue::bool(false);
    assert!(!v.to_boolean());
}

#[test]
fn test_jsvalue_int() {
    let v = JSValue::int(42);
    assert_eq!(v.type_of(), "number");
    assert!(v.is_number());
    assert_eq!(v.to_int32(), 42);
}

#[test]
fn test_jsvalue_float() {
    let v = JSValue::float(3.14);
    assert_eq!(v.type_of(), "number");
    assert!(v.is_number());
    assert!((v.to_number() - 3.14).abs() < f64::EPSILON);
}

#[test]
fn test_jsvalue_string() {
    let v = JSValue::string("hello");
    assert_eq!(v.type_of(), "string");
    assert!(v.is_string());
    assert_eq!(v.to_string(), "hello");
}

#[test]
fn test_jsvalue_object() {
    let v = JSValue::object("MyClass");
    assert_eq!(v.type_of(), "object");
    assert!(v.is_object());
    assert!(!v.is_callable());
}

#[test]
fn test_jsvalue_function() {
    let v = JSValue::function(
        Some("test"),
        vec!["a".to_string()],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    assert_eq!(v.type_of(), "function");
    assert!(v.is_object());
    assert!(v.is_callable());
}

// ============================================================================
// to_boolean for all types
// ============================================================================

#[test]
fn test_to_boolean_undefined() {
    assert!(!JSValue::undefined().to_boolean());
}

#[test]
fn test_to_boolean_null() {
    assert!(!JSValue::null().to_boolean());
}

#[test]
fn test_to_boolean_bool_values() {
    assert!(JSValue::bool(true).to_boolean());
    assert!(!JSValue::bool(false).to_boolean());
}

#[test]
fn test_to_boolean_int_zero() {
    assert!(!JSValue::int(0).to_boolean());
}

#[test]
fn test_to_boolean_int_nonzero() {
    assert!(JSValue::int(1).to_boolean());
    assert!(JSValue::int(-1).to_boolean());
}

#[test]
fn test_to_boolean_float_nan() {
    assert!(!JSValue::float(f64::NAN).to_boolean());
}

#[test]
fn test_to_boolean_float_zero() {
    assert!(!JSValue::float(0.0).to_boolean());
}

#[test]
fn test_to_boolean_float_negative_zero() {
    assert!(!JSValue::float(-0.0).to_boolean());
}

#[test]
fn test_to_boolean_float_nonzero() {
    assert!(JSValue::float(1.5).to_boolean());
}

#[test]
fn test_to_boolean_empty_string() {
    assert!(!JSValue::string("").to_boolean());
}

#[test]
fn test_to_boolean_nonempty_string() {
    assert!(JSValue::string("0").to_boolean());
    assert!(JSValue::string("false").to_boolean());
}

#[test]
fn test_to_boolean_object() {
    assert!(JSValue::object("X").to_boolean());
}

#[test]
fn test_to_boolean_function() {
    let f = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
    assert!(f.to_boolean());
}

// ============================================================================
// type_of
// ============================================================================

#[test]
fn test_type_of_string_variant() {
    let v = JSValue::string("hello");
    assert_eq!(v.type_of(), "string");
}

#[test]
fn test_type_of_object_variant() {
    let v = JSValue::object("Foo");
    assert_eq!(v.type_of(), "object");
}

#[test]
fn test_type_of_function_variant() {
    let v = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
    assert_eq!(v.type_of(), "function");
}

// ============================================================================
// to_number conversions
// ============================================================================

#[test]
fn test_to_number_undefined() {
    assert!(JSValue::undefined().to_number().is_nan());
}

#[test]
fn test_to_number_null() {
    assert_eq!(JSValue::null().to_number(), 0.0);
}

#[test]
fn test_to_number_bool_true() {
    assert_eq!(JSValue::bool(true).to_number(), 1.0);
}

#[test]
fn test_to_number_bool_false() {
    assert_eq!(JSValue::bool(false).to_number(), 0.0);
}

#[test]
fn test_to_number_int() {
    assert_eq!(JSValue::int(42).to_number(), 42.0);
}

#[test]
fn test_to_number_float() {
    assert_eq!(JSValue::float(3.14).to_number(), 3.14);
}

#[test]
fn test_to_number_string_numeric() {
    assert_eq!(JSValue::string("123").to_number(), 123.0);
}

#[test]
fn test_to_number_string_empty() {
    assert_eq!(JSValue::string("").to_number(), 0.0);
}

#[test]
fn test_to_number_string_non_numeric() {
    assert!(JSValue::string("abc").to_number().is_nan());
}

#[test]
fn test_to_number_object() {
    // Object -> NaN (no toPrimitive in unit test)
    assert!(JSValue::object("X").to_number().is_nan());
}

// ============================================================================
// to_int32 / to_uint32
// ============================================================================

#[test]
fn test_to_int32_normal() {
    assert_eq!(JSValue::int(100).to_int32(), 100);
}

#[test]
fn test_to_int32_negative() {
    assert_eq!(JSValue::int(-50).to_int32(), -50);
}

#[test]
fn test_to_int32_from_float() {
    assert_eq!(JSValue::float(3.7).to_int32(), 3);
}

#[test]
fn test_to_int32_nan() {
    assert_eq!(JSValue::float(f64::NAN).to_int32(), 0);
}

#[test]
fn test_to_int32_infinity() {
    assert_eq!(JSValue::float(f64::INFINITY).to_int32(), 0);
}

#[test]
fn test_to_uint32_normal() {
    assert_eq!(JSValue::int(42).to_uint32(), 42);
}

#[test]
fn test_to_uint32_negative() {
    // Negative values wrap
    let v = JSValue::int(-1);
    assert_eq!(v.to_uint32(), 0xFFFFFFFF);
}

#[test]
fn test_to_uint32_nan() {
    assert_eq!(JSValue::float(f64::NAN).to_uint32(), 0);
}

#[test]
fn test_to_uint32_from_string() {
    assert_eq!(JSValue::string("10").to_uint32(), 10);
}

// ============================================================================
// to_string for all types
// ============================================================================

#[test]
fn test_to_string_undefined() {
    assert_eq!(JSValue::undefined().to_string(), "undefined");
}

#[test]
fn test_to_string_null() {
    assert_eq!(JSValue::null().to_string(), "null");
}

#[test]
fn test_to_string_bool_true() {
    assert_eq!(JSValue::bool(true).to_string(), "true");
}

#[test]
fn test_to_string_bool_false() {
    assert_eq!(JSValue::bool(false).to_string(), "false");
}

#[test]
fn test_to_string_int() {
    assert_eq!(JSValue::int(0).to_string(), "0");
    assert_eq!(JSValue::int(-42).to_string(), "-42");
}

#[test]
fn test_to_string_float_integer() {
    // Floats that are integers should not have a decimal point
    assert_eq!(JSValue::float(5.0).to_string(), "5");
}

#[test]
fn test_to_string_float_fraction() {
    assert_eq!(JSValue::float(3.14).to_string(), "3.14");
}

#[test]
fn test_to_string_float_nan() {
    assert_eq!(JSValue::float(f64::NAN).to_string(), "NaN");
}

#[test]
fn test_to_string_float_infinity() {
    assert_eq!(JSValue::float(f64::INFINITY).to_string(), "Infinity");
}

#[test]
fn test_to_string_float_neg_infinity() {
    assert_eq!(JSValue::float(f64::NEG_INFINITY).to_string(), "-Infinity");
}

#[test]
fn test_to_string_string() {
    assert_eq!(JSValue::string("hello").to_string(), "hello");
}

#[test]
fn test_to_string_empty_array() {
    let arr = JSValue::object("Array");
    arr.set_property("length", JSValue::int(0));
    assert_eq!(arr.to_string(), "");
}

#[test]
fn test_to_string_array_with_elements() {
    let arr = JSValue::object("Array");
    arr.set_property("0", JSValue::int(1));
    arr.set_property("1", JSValue::int(2));
    arr.set_property("2", JSValue::int(3));
    arr.set_property("length", JSValue::int(3));
    assert_eq!(arr.to_string(), "1,2,3");
}

#[test]
fn test_to_string_object_default() {
    let obj = JSValue::object("MyClass");
    assert_eq!(obj.to_string(), "[object Object]");
}

#[test]
fn test_to_string_function() {
    let f = JSValue::function(Some("myFunc"), vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
    assert_eq!(f.to_string(), "function myFunc() { [native code] }");
}

#[test]
fn test_to_string_function_unnamed() {
    let f = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
    assert_eq!(f.to_string(), "function() { [native code] }");
}

// ============================================================================
// strict_eq and abstract_eq
// ============================================================================

#[test]
fn test_strict_eq_same_type() {
    assert!(JSValue::int(5).strict_eq(&JSValue::int(5)));
    assert!(!JSValue::int(5).strict_eq(&JSValue::int(6)));
}

#[test]
fn test_strict_eq_different_type() {
    assert!(!JSValue::int(1).strict_eq(&JSValue::string("1")));
}

#[test]
fn test_strict_eq_undefined_null() {
    // In JS, undefined === null is false
    assert!(!JSValue::undefined().strict_eq(&JSValue::null()));
}

#[test]
fn test_strict_eq_same_string() {
    assert!(JSValue::string("abc").strict_eq(&JSValue::string("abc")));
}

#[test]
fn test_strict_eq_different_string() {
    assert!(!JSValue::string("abc").strict_eq(&JSValue::string("def")));
}

#[test]
fn test_strict_eq_float_nan() {
    // NaN !== NaN
    assert!(!JSValue::float(f64::NAN).strict_eq(&JSValue::float(f64::NAN)));
}

#[test]
fn test_strict_eq_object_identity() {
    let obj = JSValue::object("X");
    assert!(obj.strict_eq(&obj));
}

#[test]
fn test_strict_eq_different_objects() {
    let a = JSValue::object("X");
    let b = JSValue::object("X");
    assert!(!a.strict_eq(&b));
}

#[test]
fn test_abstract_eq_same_type() {
    assert!(JSValue::int(5).abstract_eq(&JSValue::int(5)));
}

#[test]
fn test_abstract_eq_undefined_null() {
    // undefined == null is true in JS
    assert!(JSValue::undefined().abstract_eq(&JSValue::null()));
    assert!(JSValue::null().abstract_eq(&JSValue::undefined()));
}

#[test]
fn test_abstract_eq_number_string() {
    assert!(JSValue::int(1).abstract_eq(&JSValue::string("1")));
    assert!(JSValue::string("1").abstract_eq(&JSValue::int(1)));
}

#[test]
fn test_abstract_eq_bool_number() {
    assert!(JSValue::bool(true).abstract_eq(&JSValue::int(1)));
    assert!(JSValue::bool(false).abstract_eq(&JSValue::int(0)));
}

#[test]
fn test_abstract_eq_string_bool() {
    assert!(JSValue::string("1").abstract_eq(&JSValue::bool(true)));
}

#[test]
fn test_abstract_eq_nan() {
    // NaN != NaN even with abstract equality
    assert!(!JSValue::float(f64::NAN).abstract_eq(&JSValue::float(f64::NAN)));
}

// ============================================================================
// is_truthy / is_falsy
// ============================================================================

#[test]
fn test_is_truthy() {
    assert!(JSValue::bool(true).is_truthy());
    assert!(JSValue::int(1).is_truthy());
    assert!(JSValue::string("x").is_truthy());
    assert!(JSValue::object("X").is_truthy());
}

#[test]
fn test_is_falsy() {
    assert!(JSValue::bool(false).is_falsy());
    assert!(JSValue::int(0).is_falsy());
    assert!(JSValue::float(f64::NAN).is_falsy());
    assert!(JSValue::string("").is_falsy());
    assert!(JSValue::undefined().is_falsy());
    assert!(JSValue::null().is_falsy());
}

// ============================================================================
// Object property operations
// ============================================================================

#[test]
fn test_set_get_property() {
    let obj = JSValue::object("X");
    obj.set_property("foo", JSValue::int(42));
    let val = obj.get_property("foo").unwrap();
    assert_eq!(val.to_int32(), 42);
}

#[test]
fn test_has_property() {
    let obj = JSValue::object("X");
    assert!(!obj.has_property("foo"));
    obj.set_property("foo", JSValue::int(1));
    assert!(obj.has_property("foo"));
}

#[test]
fn test_get_property_nonexistent() {
    let obj = JSValue::object("X");
    assert!(obj.get_property("nonexistent").is_none());
}

#[test]
fn test_property_on_function() {
    let f = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
    f.set_property("myProp", JSValue::int(99));
    assert_eq!(f.get_property("myProp").unwrap().to_int32(), 99);
}

#[test]
fn test_property_on_string() {
    let s = JSValue::string("hello");
    // String objects expose length
    let len = s.get_property("length");
    assert!(len.is_some());
    assert_eq!(len.unwrap().to_int32(), 5);
}

#[test]
fn test_get_property_on_non_object() {
    let v = JSValue::int(42);
    assert!(v.get_property("foo").is_none());
}

#[test]
fn test_has_property_on_non_object() {
    assert!(!JSValue::int(42).has_property("foo"));
    assert!(!JSValue::undefined().has_property("foo"));
}

// ============================================================================
// Prototype chain
// ============================================================================

#[test]
fn test_prototype_chain_lookup() {
    let proto = JSValue::object("Proto");
    proto.set_property("inherited", JSValue::int(100));

    let child = JSValue::object("Child");
    if let JSValue::Object(ref o) = child {
        if let JSValue::Object(ref p) = proto {
            o.borrow_mut().prototype = Some(p.clone());
        }
    }

    assert_eq!(child.get_property("inherited").unwrap().to_int32(), 100);
}

#[test]
fn test_prototype_chain_shadow() {
    let proto = JSValue::object("Proto");
    proto.set_property("x", JSValue::int(1));

    let child = JSValue::object("Child");
    if let JSValue::Object(ref o) = child {
        if let JSValue::Object(ref p) = proto {
            o.borrow_mut().prototype = Some(p.clone());
        }
    }
    child.set_property("x", JSValue::int(2));

    assert_eq!(child.get_property("x").unwrap().to_int32(), 2);
}

#[test]
fn test_prototype_chain_deep() {
    let grandparent = JSValue::object("GrandParent");
    grandparent.set_property("deep", JSValue::string("found"));

    let parent = JSValue::object("Parent");
    if let JSValue::Object(ref o) = parent {
        if let JSValue::Object(ref gp) = grandparent {
            o.borrow_mut().prototype = Some(gp.clone());
        }
    }

    let child = JSValue::object("Child");
    if let JSValue::Object(ref o) = child {
        if let JSValue::Object(ref p) = parent {
            o.borrow_mut().prototype = Some(p.clone());
        }
    }

    assert_eq!(child.get_property("deep").unwrap().to_string(), "found");
}

// ============================================================================
// Getter / Setter
// ============================================================================

#[test]
fn test_define_getter() {
    let obj = JSValue::object("X");
    let getter = JSValue::function(
        None,
        vec![],
        FunctionBody::Native(|_, _| JSValue::int(42)),
    );
    obj.define_getter("myProp", getter);
    // define_getter stores in descriptors, not properties
    if let JSValue::Object(ref o) = obj {
        assert!(o.borrow().descriptors.contains_key("myProp"));
    }
}

#[test]
fn test_define_setter() {
    let obj = JSValue::object("X");
    let setter = JSValue::function(
        None,
        vec!["val".to_string()],
        FunctionBody::Native(|_, _| JSValue::undefined()),
    );
    obj.define_setter("myProp", setter);
}

// ============================================================================
// hash_string
// ============================================================================

#[test]
fn test_hash_string_deterministic() {
    let h1 = JSValue::hash_string("test");
    let h2 = JSValue::hash_string("test");
    assert_eq!(h1, h2);
}

#[test]
fn test_hash_string_different() {
    let h1 = JSValue::hash_string("abc");
    let h2 = JSValue::hash_string("def");
    assert_ne!(h1, h2);
}

#[test]
fn test_hash_string_empty() {
    let h = JSValue::hash_string("");
    assert_eq!(h, 0); // empty string hashes to 0
}

// ============================================================================
// PartialEq
// ============================================================================

#[test]
fn test_partial_eq_primitives() {
    assert_eq!(JSValue::int(5), JSValue::int(5));
    assert_ne!(JSValue::int(5), JSValue::int(6));
    assert_eq!(JSValue::bool(true), JSValue::bool(true));
    assert_eq!(JSValue::null(), JSValue::null());
    assert_eq!(JSValue::undefined(), JSValue::undefined());
}

#[test]
fn test_partial_eq_string_identity() {
    let s1 = JSValue::string("hello");
    let s2 = s1.clone();
    assert_eq!(s1, s2); // same Rc
}

#[test]
fn test_partial_eq_different_strings() {
    // Different JSString Rc's with same content are NOT equal (pointer comparison)
    let s1 = JSValue::string("hello");
    let s2 = JSValue::string("hello");
    // These are different Rc allocations, so they should NOT be equal
    assert_ne!(s1, s2);
}

#[test]
fn test_partial_eq_object_identity() {
    let obj = JSValue::object("X");
    let obj2 = obj.clone();
    assert_eq!(obj, obj2);
}

#[test]
fn test_partial_eq_different_objects() {
    let a = JSValue::object("X");
    let b = JSValue::object("X");
    assert_ne!(a, b);
}

#[test]
fn test_partial_eq_cross_type() {
    assert_ne!(JSValue::int(1), JSValue::string("1"));
    assert_ne!(JSValue::null(), JSValue::undefined());
}

// ============================================================================
// Display trait
// ============================================================================

#[test]
fn test_display_delegates_to_to_string() {
    let v = JSValue::int(42);
    assert_eq!(format!("{}", v), "42");
}

#[test]
fn test_display_undefined() {
    assert_eq!(format!("{}", JSValue::undefined()), "undefined");
}

#[test]
fn test_display_null() {
    assert_eq!(format!("{}", JSValue::null()), "null");
}

// ============================================================================
// BytecodeFunction
// ============================================================================

#[test]
fn test_bytecode_function_new() {
    let bc = BytecodeFunction::new();
    assert!(bc.name.is_none());
    assert!(bc.params.is_empty());
    assert!(bc.bytecode.is_empty());
    assert!(bc.constants.is_empty());
    assert!(bc.variables.is_empty());
    assert!(bc.functions.is_empty());
    assert!(bc.line_numbers.is_empty());
    assert!(bc.filename.is_none());
    assert!(!bc.is_generator);
    assert!(!bc.is_async);
    assert!(!bc.is_arrow);
    assert!(!bc.is_module);
    assert!(!bc.strict_mode);
    assert!(bc.rest_param_index.is_none());
    assert!(bc.closure_vars.is_empty());
}

#[test]
fn test_bytecode_function_line_number_empty() {
    let bc = BytecodeFunction::new();
    assert!(bc.line_number(0).is_none());
}

#[test]
fn test_bytecode_function_line_number_with_data() {
    let mut bc = BytecodeFunction::new();
    bc.line_numbers.push((0, 10)); // pc 0 -> line 10
    bc.line_numbers.push((5, 11)); // pc 5 -> line 11
    assert_eq!(bc.line_number(0), Some(10));
    assert_eq!(bc.line_number(3), Some(10));
    assert_eq!(bc.line_number(5), Some(11));
    assert_eq!(bc.line_number(10), Some(11));
}

// ============================================================================
// Debug formatting
// ============================================================================

#[test]
fn test_debug_jsvalue() {
    let v = JSValue::int(42);
    let s = format!("{:?}", v);
    assert!(s.contains("42"));
}

#[test]
fn test_debug_function_body() {
    let fb = FunctionBody::Native(|_, _| JSValue::undefined());
    let s = format!("{:?}", fb);
    assert!(s.contains("Native"));
}

#[test]
fn test_debug_source_body() {
    let fb = FunctionBody::Source("return 1".to_string());
    let s = format!("{:?}", fb);
    assert!(s.contains("Source"));
}

// ============================================================================
// JSObject
// ============================================================================

#[test]
fn test_jsobject_default() {
    let obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Test".to_string(),
    };
    assert_eq!(obj.class_name, "Test");
    assert!(obj.properties.is_empty());
    assert!(obj.prototype.is_none());
}

#[test]
fn test_jsobject_with_properties() {
    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Test".to_string(),
    };
    obj.properties.insert("key".to_string(), JSValue::int(42));
    assert_eq!(obj.properties.get("key").unwrap().to_int32(), 42);
}

// ============================================================================
// JSSymbol
// ============================================================================

#[test]
fn test_jssymbol_with_description() {
    let sym = JSSymbol {
        description: Some("test".to_string()),
        id: 1,
    };
    assert_eq!(sym.description, Some("test".to_string()));
    assert_eq!(sym.id, 1);
}

#[test]
fn test_jssymbol_without_description() {
    let sym = JSSymbol {
        description: None,
        id: 2,
    };
    assert!(sym.description.is_none());
}

#[test]
fn test_jssymbol_partial_eq() {
    let a = JSSymbol { description: None, id: 5 };
    let b = JSSymbol { description: None, id: 5 };
    let c = JSSymbol { description: None, id: 6 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ============================================================================
// JSBigInt
// ============================================================================

#[test]
fn test_jsbigint_creation() {
    let bi = JSBigInt {
        value: "12345678901234567890".to_string(),
    };
    assert_eq!(bi.value, "12345678901234567890");
}

#[test]
fn test_jsbigint_partial_eq() {
    let a = JSBigInt { value: "100".to_string() };
    let b = JSBigInt { value: "100".to_string() };
    let c = JSBigInt { value: "200".to_string() };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ============================================================================
// PropertyDescriptor
// ============================================================================

#[test]
fn test_data_descriptor() {
    let desc = PropertyDescriptor::Data {
        value: JSValue::int(42),
        writable: true,
        enumerable: true,
        configurable: true,
    };
    match desc {
        PropertyDescriptor::Data { value, writable, enumerable, configurable } => {
            assert_eq!(value.to_int32(), 42);
            assert!(writable);
            assert!(enumerable);
            assert!(configurable);
        }
        _ => panic!("Expected Data descriptor"),
    }
}

#[test]
fn test_accessor_descriptor() {
    let getter = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::int(1)));
    let desc = PropertyDescriptor::Accessor {
        get: Some(getter),
        set: None,
        enumerable: false,
        configurable: true,
    };
    match desc {
        PropertyDescriptor::Accessor { get, set, enumerable, configurable } => {
            assert!(get.is_some());
            assert!(set.is_none());
            assert!(!enumerable);
            assert!(configurable);
        }
        _ => panic!("Expected Accessor descriptor"),
    }
}

// ============================================================================
// Variable
// ============================================================================

#[test]
fn test_variable_creation() {
    let v = Variable {
        name: "x".to_string(),
        kind: VariableKind::Let,
        scope_level: 1,
        is_captured: false,
        is_parameter: false,
    };
    assert_eq!(v.name, "x");
    assert_eq!(v.scope_level, 1);
}

#[test]
fn test_variable_kinds() {
    let v_var = Variable { name: "a".to_string(), kind: VariableKind::Var, scope_level: 0, is_captured: false, is_parameter: false };
    let v_let = Variable { name: "b".to_string(), kind: VariableKind::Let, scope_level: 0, is_captured: false, is_parameter: false };
    let v_const = Variable { name: "c".to_string(), kind: VariableKind::Const, scope_level: 0, is_captured: false, is_parameter: false };
    assert_eq!(v_var.kind, VariableKind::Var);
    assert_eq!(v_let.kind, VariableKind::Let);
    assert_eq!(v_const.kind, VariableKind::Const);
}

// ============================================================================
// Opcode (basic enum test)
// ============================================================================

#[test]
fn test_opcode_variants() {
    let op = Opcode::Add;
    assert_eq!(op, Opcode::Add);
    assert_ne!(op, Opcode::Sub);
}

#[test]
fn test_opcode_push_int() {
    let op = Opcode::PushInt(42);
    match op {
        Opcode::PushInt(n) => assert_eq!(n, 42),
        _ => panic!("Expected PushInt"),
    }
}

#[test]
fn test_opcode_jump() {
    let op = Opcode::Jump(10);
    match op {
        Opcode::Jump(addr) => assert_eq!(addr, 10),
        _ => panic!("Expected Jump"),
    }
}

// ============================================================================
// Constant
// ============================================================================

#[test]
fn test_constant_float() {
    let c = Constant::Float(3.14);
    match c {
        Constant::Float(f) => assert!((f - 3.14).abs() < f64::EPSILON),
        _ => panic!("Expected Float"),
    }
}

#[test]
fn test_constant_string() {
    let c = Constant::String("hello".to_string());
    match c {
        Constant::String(s) => assert_eq!(s, "hello"),
        _ => panic!("Expected String"),
    }
}

#[test]
fn test_constant_bigint() {
    let c = Constant::BigInt("123456789".to_string());
    match c {
        Constant::BigInt(s) => assert_eq!(s, "123456789"),
        _ => panic!("Expected BigInt"),
    }
}

#[test]
fn test_constant_regexp() {
    let c = Constant::RegExp {
        pattern: "\\d+".to_string(),
        flags: "gi".to_string(),
    };
    match c {
        Constant::RegExp { pattern, flags } => {
            assert_eq!(pattern, "\\d+");
            assert_eq!(flags, "gi");
        }
        _ => panic!("Expected RegExp"),
    }
}
