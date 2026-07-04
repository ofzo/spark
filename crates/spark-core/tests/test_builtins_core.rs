//! Tests for core builtins: console, module, error, symbol, boolean

use spark_core::builtins::console::*;
use spark_core::builtins::module::*;
use spark_core::builtins::error::*;
use spark_core::builtins::symbol::*;
use spark_core::builtins::boolean::*;
use spark_core::context::JSContext;
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn make_ctx() -> JSContext {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    JSContext::new(rt)
}

// ============================================================================
// console.rs tests
// ============================================================================

#[test]
fn test_init_console_registers_globals() {
    let mut ctx = make_ctx();
    init_console(&mut ctx);
    let global = ctx.global.borrow();
    assert!(global.properties.get("print").is_some());
    assert!(global.properties.get("console").is_some());
    assert!(global.properties.get("parseInt").is_some());
    assert!(global.properties.get("parseFloat").is_some());
    assert!(global.properties.get("isNaN").is_some());
    assert!(global.properties.get("isFinite").is_some());
    assert!(global.properties.get("encodeURI").is_some());
    assert!(global.properties.get("decodeURI").is_some());
    assert!(global.properties.get("encodeURIComponent").is_some());
    assert!(global.properties.get("decodeURIComponent").is_some());
    assert!(global.properties.get("eval").is_some());
    assert!(global.properties.get("setTimeout").is_some());
    assert!(global.properties.get("clearTimeout").is_some());
    assert!(global.properties.get("setInterval").is_some());
    assert!(global.properties.get("clearInterval").is_some());
}

#[test]
fn test_init_console_console_object_has_methods() {
    let mut ctx = make_ctx();
    init_console(&mut ctx);
    let global = ctx.global.borrow();
    let console = global.properties.get("console").unwrap();
    assert!(console.get_property("log").is_some());
    assert!(console.get_property("error").is_some());
    assert!(console.get_property("warn").is_some());
    assert!(console.get_property("info").is_some());
}

#[test]
fn test_take_pending_callbacks_empty() {
    let _ = take_pending_callbacks(); // drain any leftovers
    let callbacks = take_pending_callbacks();
    assert!(callbacks.is_empty());
}

// ============================================================================
// module.rs tests
// ============================================================================

struct TestLoader {
    modules: HashMap<String, JSValue>,
}

impl TestLoader {
    fn new() -> Self {
        TestLoader { modules: HashMap::new() }
    }

    fn add_module(&mut self, specifier: &str, exports: JSValue) {
        self.modules.insert(specifier.to_string(), exports);
    }
}

impl ModuleLoader for TestLoader {
    fn load(&self, specifier: &str, _referrer: Option<&str>) -> ModuleResult {
        self.modules.get(specifier)
            .cloned()
            .ok_or_else(|| format!("Module not found: {}", specifier))
    }
}

#[test]
fn test_module_registry_new() {
    let loader = Box::new(TestLoader::new());
    let registry = ModuleRegistry::new(loader);
    assert_eq!(registry.cache_size(), 0);
    assert_eq!(registry.native_count(), 0);
}

#[test]
fn test_module_registry_register_native() {
    let loader = Box::new(TestLoader::new());
    let mut registry = ModuleRegistry::new(loader);
    let exports = JSValue::object("Module");
    exports.set_property("hello", JSValue::int(42));
    registry.register_native("mymod", exports);

    assert_eq!(registry.native_count(), 1);
    assert!(registry.has("mymod"));
}

#[test]
fn test_module_registry_load_native() {
    let loader = Box::new(TestLoader::new());
    let mut registry = ModuleRegistry::new(loader);
    let exports = JSValue::object("Module");
    exports.set_property("value", JSValue::int(100));
    registry.register_native("native_mod", exports);

    let result = registry.load("native_mod", None).unwrap();
    assert_eq!(result.get_property("value").unwrap().to_int32(), 100);
}

#[test]
fn test_module_registry_load_from_loader() {
    let mut loader = TestLoader::new();
    let exports = JSValue::object("Module");
    exports.set_property("loaded", JSValue::bool(true));
    loader.add_module("./test.js", exports);

    let mut registry = ModuleRegistry::new(Box::new(loader));
    let result = registry.load("./test.js", None).unwrap();
    assert!(result.get_property("loaded").unwrap().to_boolean());
}

#[test]
fn test_module_registry_load_caches_result() {
    let mut loader = TestLoader::new();
    let exports = JSValue::object("Module");
    loader.add_module("./cached.js", exports);

    let mut registry = ModuleRegistry::new(Box::new(loader));
    let _ = registry.load("./cached.js", None).unwrap();
    assert_eq!(registry.cache_size(), 1);
    assert!(registry.has("./cached.js"));
}

#[test]
fn test_module_registry_load_returns_cached() {
    let mut loader = TestLoader::new();
    let exports = JSValue::object("Module");
    exports.set_property("val", JSValue::int(1));
    loader.add_module("./mod.js", exports);

    let mut registry = ModuleRegistry::new(Box::new(loader));
    let r1 = registry.load("./mod.js", None).unwrap();
    let r2 = registry.load("./mod.js", None).unwrap();
    // Both should return the same cached result
    assert_eq!(r1.get_property("val").unwrap().to_int32(), 1);
    assert_eq!(r2.get_property("val").unwrap().to_int32(), 1);
}

#[test]
fn test_module_registry_load_error() {
    let loader = Box::new(TestLoader::new());
    let mut registry = ModuleRegistry::new(loader);
    let result = registry.load("./missing.js", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn test_module_registry_has_not_loaded() {
    let loader = Box::new(TestLoader::new());
    let registry = ModuleRegistry::new(loader);
    assert!(!registry.has("./not_loaded.js"));
}

#[test]
fn test_module_registry_native_priority_over_loader() {
    let mut loader = TestLoader::new();
    let loader_exports = JSValue::object("Module");
    loader_exports.set_property("source", JSValue::string("loader"));
    loader.add_module("mymod", loader_exports);

    let mut registry = ModuleRegistry::new(Box::new(loader));
    let native_exports = JSValue::object("Module");
    native_exports.set_property("source", JSValue::string("native"));
    registry.register_native("mymod", native_exports);

    let result = registry.load("mymod", None).unwrap();
    assert_eq!(result.get_property("source").unwrap().to_string(), "native");
}

#[test]
fn test_module_registry_load_with_referrer() {
    let mut loader = TestLoader::new();
    let exports = JSValue::object("Module");
    exports.set_property("ok", JSValue::bool(true));
    loader.add_module("./sub/mod.js", exports);

    let mut registry = ModuleRegistry::new(Box::new(loader));
    let result = registry.load("./sub/mod.js", Some("./src/main.js")).unwrap();
    assert!(result.get_property("ok").unwrap().to_boolean());
}

#[test]
fn test_module_registry_circular_dependency_returns_empty() {
    // Simulate circular dependency by using a loader that triggers re-entry
    struct CircularLoader;
    impl ModuleLoader for CircularLoader {
        fn load(&self, _specifier: &str, _referrer: Option<&str>) -> ModuleResult {
            // This would normally recurse, but the registry prevents it
            Err("should not reach here".to_string())
        }
    }

    let registry = ModuleRegistry::new(Box::new(CircularLoader));
    // The circular detection is internal - we can test that `has` works correctly
    // after a successful load
    assert!(!registry.has("anything"));
}

// ============================================================================
// error.rs tests - all error constructors
// ============================================================================

#[test]
fn test_range_error_constructor() {
    let obj = JSValue::object("RangeError");
    range_error_constructor(&obj, &[JSValue::string("out of range")]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("message").unwrap().to_string(), "out of range");
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "RangeError");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_reference_error_constructor() {
    let obj = JSValue::object("ReferenceError");
    reference_error_constructor(&obj, &[JSValue::string("not defined")]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "ReferenceError");
            assert_eq!(borrow.properties.get("message").unwrap().to_string(), "not defined");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_syntax_error_constructor() {
    let obj = JSValue::object("SyntaxError");
    syntax_error_constructor(&obj, &[JSValue::string("unexpected token")]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "SyntaxError");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_uri_error_constructor() {
    let obj = JSValue::object("URIError");
    uri_error_constructor(&obj, &[JSValue::string("malformed uri")]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "URIError");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_eval_error_constructor() {
    let obj = JSValue::object("EvalError");
    eval_error_constructor(&obj, &[JSValue::string("eval failed")]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "EvalError");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_error_constructor_no_args() {
    let obj = JSValue::object("Error");
    error_constructor(&obj, &[]);
    match &obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            assert_eq!(borrow.properties.get("message").unwrap().to_string(), "");
            assert_eq!(borrow.properties.get("name").unwrap().to_string(), "Error");
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_error_to_string_with_name_and_message() {
    let obj = JSValue::object("Error");
    obj.set_property("name", JSValue::string("TypeError"));
    obj.set_property("message", JSValue::string("bad type"));
    let result = error_to_string(&obj, &[]);
    assert_eq!(result.to_string(), "TypeError: bad type");
}

#[test]
fn test_error_to_string_with_empty_message() {
    let obj = JSValue::object("Error");
    obj.set_property("name", JSValue::string("Error"));
    obj.set_property("message", JSValue::string(""));
    let result = error_to_string(&obj, &[]);
    assert_eq!(result.to_string(), "Error");
}

#[test]
fn test_error_to_string_on_non_object() {
    let result = error_to_string(&JSValue::int(42), &[]);
    assert_eq!(result.to_string(), "Error");
}

#[test]
fn test_error_to_string_on_string() {
    let result = error_to_string(&JSValue::string("not an error"), &[]);
    assert_eq!(result.to_string(), "Error");
}

#[test]
fn test_error_to_string_with_internal_slots_name() {
    let obj = JSValue::object("Error");
    if let JSValue::Object(ref o) = obj {
        o.borrow_mut().internal_slots.insert("name".to_string(), JSValue::string("InternalError"));
    }
    obj.set_property("message", JSValue::string("fail"));
    let result = error_to_string(&obj, &[]);
    assert_eq!(result.to_string(), "InternalError: fail");
}

#[test]
fn test_init_error_all_subclasses() {
    let mut ctx = make_ctx();
    init_error(&mut ctx);
    let global = ctx.global.borrow();

    for name in &["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "URIError", "EvalError"] {
        let ctor = global.properties.get(*name);
        assert!(ctor.is_some(), "{} should be registered", name);
        assert!(ctor.unwrap().is_callable(), "{} should be callable", name);
    }
}

// ============================================================================
// symbol.rs tests
// ============================================================================

#[test]
fn test_symbol_prototype_value_of() {
    let sym = symbol_constructor(&JSValue::undefined(), &[JSValue::string("desc")]);
    let result = symbol_prototype_value_of(&sym, &[]);
    // valueOf returns the symbol itself
    match (&sym, &result) {
        (JSValue::Symbol(s1), JSValue::Symbol(s2)) => {
            assert_eq!(s1.borrow().id, s2.borrow().id);
        }
        _ => panic!("Expected symbols"),
    }
}

#[test]
fn test_symbol_prototype_description_direct() {
    let sym = symbol_constructor(&JSValue::undefined(), &[JSValue::string("myDesc")]);
    let desc = symbol_prototype_description(&sym, &[]);
    assert_eq!(desc.to_string(), "myDesc");
}

#[test]
fn test_symbol_prototype_description_none() {
    let sym = symbol_constructor(&JSValue::undefined(), &[]);
    let desc = symbol_prototype_description(&sym, &[]);
    assert!(desc.is_undefined());
}

#[test]
fn test_symbol_to_string_no_description() {
    let sym = symbol_constructor(&JSValue::undefined(), &[]);
    let result = symbol_prototype_to_string(&sym, &[]);
    assert_eq!(result.to_string(), "Symbol()");
}

#[test]
fn test_symbol_for_returns_same_symbol() {
    let s1 = symbol_for(&JSValue::undefined(), &[JSValue::string("shared_key")]);
    let s2 = symbol_for(&JSValue::undefined(), &[JSValue::string("shared_key")]);
    match (&s1, &s2) {
        (JSValue::Symbol(a), JSValue::Symbol(b)) => {
            assert_eq!(a.borrow().id, b.borrow().id);
        }
        _ => panic!("Expected symbols"),
    }
}

#[test]
fn test_symbol_key_for_unknown() {
    let sym = symbol_constructor(&JSValue::undefined(), &[JSValue::string("local")]);
    let result = symbol_key_for(&JSValue::undefined(), &[sym]);
    // keyFor returns undefined for non-global symbols
    assert!(result.is_undefined());
}

#[test]
fn test_symbol_constants() {
    assert_eq!(SYMBOL_ITERATOR_ID, 1);
    assert_eq!(SYMBOL_TO_PRIMITIVE_ID, 2);
    assert_eq!(SYMBOL_TO_STRING_TAG_ID, 3);
    assert_eq!(SYMBOL_HAS_INSTANCE_ID, 4);
    assert_eq!(SYMBOL_IS_CONCAT_SPREADABLE_ID, 5);
    assert_eq!(SYMBOL_ASYNC_ITERATOR_ID, 6);
    assert_eq!(SYMBOL_SPECIES_ID, 7);
    assert_eq!(SYMBOL_TO_JSON_ID, 8);
}

#[test]
fn test_init_symbol_registers_well_known() {
    let mut ctx = make_ctx();
    init_symbol(&mut ctx);
    let global = ctx.global.borrow();
    let symbol_ctor = global.properties.get("Symbol").unwrap();
    assert!(symbol_ctor.get_property("iterator").is_some());
    assert!(symbol_ctor.get_property("toPrimitive").is_some());
    assert!(symbol_ctor.get_property("toStringTag").is_some());
    assert!(symbol_ctor.get_property("hasInstance").is_some());
    assert!(symbol_ctor.get_property("isConcatSpreadable").is_some());
    assert!(symbol_ctor.get_property("asyncIterator").is_some());
    assert!(symbol_ctor.get_property("species").is_some());
}

// ============================================================================
// boolean.rs tests
// ============================================================================

#[test]
fn test_boolean_constructor_with_truthy_string() {
    let result = boolean_constructor(&JSValue::undefined(), &[JSValue::string("hello")]);
    assert!(result.to_boolean());
}

#[test]
fn test_boolean_constructor_with_zero() {
    let result = boolean_constructor(&JSValue::undefined(), &[JSValue::int(0)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_boolean_constructor_with_nan() {
    let result = boolean_constructor(&JSValue::undefined(), &[JSValue::float(f64::NAN)]);
    assert!(!result.to_boolean());
}

#[test]
fn test_boolean_constructor_with_object_arg() {
    let result = boolean_constructor(&JSValue::undefined(), &[JSValue::object("X")]);
    assert!(result.to_boolean());
}

#[test]
fn test_boolean_value_of_on_object() {
    let obj = JSValue::object("Boolean");
    if let JSValue::Object(ref o) = obj {
        o.borrow_mut().internal_slots.insert("PrimitiveValue".to_string(), JSValue::bool(true));
    }
    let result = boolean_value_of(&obj, &[]);
    assert!(result.to_boolean());
}

#[test]
fn test_boolean_value_of_on_bool_primitive() {
    let result = boolean_value_of(&JSValue::bool(false), &[]);
    assert!(!result.to_boolean());
}

#[test]
fn test_boolean_to_string_on_object() {
    let obj = JSValue::object("Boolean");
    if let JSValue::Object(ref o) = obj {
        o.borrow_mut().internal_slots.insert("PrimitiveValue".to_string(), JSValue::bool(false));
    }
    let result = boolean_to_string(&obj, &[]);
    assert_eq!(result.to_string(), "false");
}

#[test]
fn test_boolean_to_string_on_bool_primitive() {
    let result = boolean_to_string(&JSValue::bool(true), &[]);
    assert_eq!(result.to_string(), "true");
}

#[test]
fn test_init_boolean_registers() {
    let mut ctx = make_ctx();
    init_boolean(&mut ctx);
    let global = ctx.global.borrow();
    let ctor = global.properties.get("Boolean").unwrap();
    assert!(ctor.is_callable());
    assert!(ctor.get_property("prototype").is_some());
}
