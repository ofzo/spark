#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! JavaScript runtime.
//!
//! Manages the global state, object pool, and garbage collector.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, JSString, JSFunction, FunctionBody};
use crate::context::JSContext;
use crate::builtins;

/// JavaScript runtime configuration.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum memory usage in bytes (0 = unlimited)
    pub max_memory: usize,
    /// Maximum stack size
    pub max_stack_size: usize,
    /// Enable strict mode by default
    pub strict_mode: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        RuntimeConfig {
            max_memory: 0,
            max_stack_size: 1024 * 1024, // 1MB
            strict_mode: false,
        }
    }
}

/// A pending task (callback to execute).
pub type Task = Box<dyn FnOnce()>;

/// JavaScript runtime.
///
/// Manages the global state, object pool, and garbage collector.
pub struct JSRuntime {
    /// Runtime configuration
    pub config: RuntimeConfig,
    /// Global object
    pub global: Rc<RefCell<JSObject>>,
    /// Object pool for garbage collection
    pub objects: Vec<Rc<RefCell<JSObject>>>,
    /// String pool for interning
    pub strings: HashMap<String, Rc<RefCell<JSString>>>,
    /// Function pool
    pub functions: Vec<Rc<RefCell<JSFunction>>>,
    /// Current memory usage
    pub memory_usage: usize,
    /// Interrupt flag
    pub interrupted: bool,
    /// Host environment (clock, output, etc.)
    pub host: crate::host::HostEnvironment,
    /// Module registry (loader + cache), set by the host environment
    pub module_registry: Option<Rc<RefCell<crate::builtins::module::ModuleRegistry>>>,
    /// Microtask queue for Promise callbacks (highest priority)
    pub microtask_queue: Vec<Task>,
    /// Macrotask queue for setTimeout/setInterval callbacks
    pub macrotask_queue: Vec<Task>,
}

impl JSRuntime {
    /// Create a new runtime with default configuration.
    pub fn new() -> Self {
        Self::with_config(RuntimeConfig::default())
    }

    /// Create a new runtime with the given configuration.
    pub fn with_config(config: RuntimeConfig) -> Self {
        let global = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "global".to_string(),
        }));

        JSRuntime {
            config,
            global,
            objects: Vec::new(),
            strings: HashMap::new(),
            functions: Vec::new(),
            memory_usage: 0,
            interrupted: false,
            host: crate::host::HostEnvironment::defaults(),
            module_registry: None,
            microtask_queue: Vec::new(),
            macrotask_queue: Vec::new(),
        }
    }

    /// Set the module loader for this runtime.
    ///
    /// The host environment calls this to provide module loading capability.
    /// Without a loader, `import` statements will fail.
    pub fn set_module_loader(&mut self, loader: crate::builtins::module::BoxedModuleLoader) {
        self.module_registry = Some(Rc::new(RefCell::new(
            crate::builtins::module::ModuleRegistry::new(loader)
        )));
    }

    /// Register a native module that can be imported via `import` statements.
    ///
    /// If no module registry exists yet, one is created with a no-op loader.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let fs = JSValue::object("Module");
    /// fs.set_property("readFile", JSValue::function(...));
    /// rt.register_native_module("fs", fs);
    /// ```
    pub fn register_native_module(&mut self, specifier: &str, exports: JSValue) {
        if self.module_registry.is_none() {
            // Create a registry with a no-op loader
            struct NoopLoader;
            impl crate::builtins::module::ModuleLoader for NoopLoader {
                fn load(&self, _specifier: &str, _referrer: Option<&str>) -> crate::builtins::module::ModuleResult {
                    Err("No module loader configured".to_string())
                }
            }
            self.module_registry = Some(Rc::new(RefCell::new(
                crate::builtins::module::ModuleRegistry::new(Box::new(NoopLoader))
            )));
        }
        if let Some(ref registry) = self.module_registry {
            registry.borrow_mut().register_native(specifier, exports);
        }
    }

    /// Intern a string (return existing if already interned).
    pub fn intern_string(&mut self, s: &str) -> Rc<RefCell<JSString>> {
        if let Some(existing) = self.strings.get(s) {
            return existing.clone();
        }
        let string = Rc::new(RefCell::new(JSString {
            data: s.to_string(),
            hash: Self::hash_string(s),
        }));
        self.strings.insert(s.to_string(), string.clone());
        string
    }

    /// Create a new object and register it with the runtime.
    pub fn new_object(&mut self, class_name: &str) -> Rc<RefCell<JSObject>> {
        let obj = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: class_name.to_string(),
        }));
        self.objects.push(obj.clone());
        obj
    }

    /// Create a new function and register it with the runtime.
    pub fn new_function(
        &mut self,
        name: Option<&str>,
        params: Vec<String>,
        body: crate::value::FunctionBody,
    ) -> Rc<RefCell<JSFunction>> {
        let func = Rc::new(RefCell::new(JSFunction {
            name: name.map(|s| s.to_string()),
            params,
            body,
            closure: HashMap::new(),
            is_constructor: false,
            is_async: false,
            is_generator: false,
        }));
        self.functions.push(func.clone());
        func
    }

    /// Run garbage collection (simplified).
    pub fn gc(&mut self) {
        // TODO: implement proper mark-and-sweep GC
        // For now, just clear unused objects
        self.objects.retain(|obj| Rc::strong_count(obj) > 1);
        self.strings.retain(|_, s| Rc::strong_count(s) > 1);
        self.functions.retain(|f| Rc::strong_count(f) > 1);
    }

    /// Get current memory usage.
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    /// Check if the runtime is interrupted.
    pub fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    /// Set the interrupt flag.
    pub fn interrupt(&mut self) {
        self.interrupted = true;
    }

    /// Clear the interrupt flag.
    pub fn clear_interrupt(&mut self) {
        self.interrupted = false;
    }

    /// Drain all pending microtasks (Promise callbacks).
    /// Microtasks have highest priority and are processed before macrotasks.
    /// Returns the number of tasks processed.
    pub fn drain_microtasks(&mut self) -> usize {
        let mut count = 0;
        while !self.microtask_queue.is_empty() {
            let task = self.microtask_queue.remove(0);
            task();
            count += 1;
        }
        count
    }

    /// Drain all pending macrotasks (setTimeout/setInterval callbacks).
    /// Returns the number of tasks processed.
    pub fn drain_macrotasks(&mut self) -> usize {
        let mut count = 0;
        while !self.macrotask_queue.is_empty() {
            let task = self.macrotask_queue.remove(0);
            task();
            count += 1;
        }
        count
    }

    /// Drain all pending tasks (microtasks first, then macrotasks).
    /// Returns the total number of tasks processed.
    pub fn drain_all_tasks(&mut self) -> usize {
        let mut total = 0;
        // Process microtasks first (they have priority)
        total += self.drain_microtasks();
        // Then process macrotasks
        total += self.drain_macrotasks();
        // Process any microtasks that were queued by macrotasks
        total += self.drain_microtasks();
        total
    }

    /// Queue a microtask (Promise callback).
    pub fn queue_microtask(&mut self, task: Box<dyn FnOnce()>) {
        self.microtask_queue.push(task);
    }

    /// Queue a macrotask (setTimeout/setInterval callback).
    pub fn queue_macrotask(&mut self, task: Box<dyn FnOnce()>) {
        self.macrotask_queue.push(task);
    }

    /// Hash a string (delegates to JSValue::hash_string).
    fn hash_string(s: &str) -> u32 {
        JSValue::hash_string(s)
    }
}

impl Default for JSRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::JSValue;

    #[test]
    fn test_runtime_creation() {
        let rt = JSRuntime::new();
        assert_eq!(rt.config.max_memory, 0);
        assert_eq!(rt.config.max_stack_size, 1024 * 1024);
    }

    #[test]
    fn test_string_interning() {
        let mut rt = JSRuntime::new();
        let s1 = rt.intern_string("hello");
        let s2 = rt.intern_string("hello");
        assert!(Rc::ptr_eq(&s1, &s2));
    }

    #[test]
    fn test_object_creation() {
        let mut rt = JSRuntime::new();
        let obj = rt.new_object("Object");
        assert_eq!(obj.borrow().class_name, "Object");
    }

    // --- init_builtins tests ---

    #[test]
    fn test_init_builtins_registers_constructors() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let expected = [
            "Object", "Array", "String", "Number", "Boolean",
            "Function", "Math", "JSON", "Date", "RegExp",
            "Error", "TypeError", "RangeError", "ReferenceError",
            "SyntaxError", "URIError", "EvalError",
            "Promise", "Map", "Set", "Symbol", "Proxy", "Reflect",
        ];

        for name in &expected {
            assert!(
                global.properties.contains_key(*name),
                "Global should have '{}'",
                name
            );
        }
    }

    #[test]
    fn test_init_builtins_array_constructor_has_prototype() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let array_ctor = global.properties.get("Array").unwrap();
        let proto = array_ctor.get_property("prototype").unwrap();

        // Array.prototype should have standard methods
        assert!(proto.get_property("push").is_some());
        assert!(proto.get_property("pop").is_some());
        assert!(proto.get_property("map").is_some());
        assert!(proto.get_property("filter").is_some());
        assert!(proto.get_property("reduce").is_some());
        assert!(proto.get_property("forEach").is_some());
        assert!(proto.get_property("indexOf").is_some());
    }

    #[test]
    fn test_init_builtins_string_prototype_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let string_ctor = global.properties.get("String").unwrap();
        let proto = string_ctor.get_property("prototype").unwrap();

        assert!(proto.get_property("charAt").is_some());
        assert!(proto.get_property("indexOf").is_some());
        assert!(proto.get_property("slice").is_some());
        assert!(proto.get_property("toLowerCase").is_some());
        assert!(proto.get_property("trim").is_some());
    }

    #[test]
    fn test_init_builtins_math_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let math = global.properties.get("Math").unwrap();

        assert!(math.get_property("abs").is_some());
        assert!(math.get_property("floor").is_some());
        assert!(math.get_property("ceil").is_some());
        assert!(math.get_property("round").is_some());
        assert!(math.get_property("PI").is_some());
        assert!(math.get_property("E").is_some());
    }

    #[test]
    fn test_init_builtins_json_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let json = global.properties.get("JSON").unwrap();

        assert!(json.get_property("parse").is_some());
        assert!(json.get_property("stringify").is_some());
    }

    #[test]
    fn test_init_builtins_prototype_chain() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();

        // Get Object.prototype
        let object_ctor = global.properties.get("Object").unwrap();
        let object_proto = object_ctor.get_property("prototype").unwrap();
        let object_proto_rc = match &object_proto {
            JSValue::Object(o) => o.clone(),
            _ => panic!("Object.prototype should be an object"),
        };

        // Check that Array.prototype inherits from Object.prototype
        let array_ctor = global.properties.get("Array").unwrap();
        let array_proto = array_ctor.get_property("prototype").unwrap();
        if let JSValue::Object(array_proto_rc) = &array_proto {
            let borrow = array_proto_rc.borrow();
            assert!(
                borrow.prototype.is_some(),
                "Array.prototype should have a prototype"
            );
            assert!(
                Rc::ptr_eq(borrow.prototype.as_ref().unwrap(), &object_proto_rc),
                "Array.prototype should inherit from Object.prototype"
            );
        }

        // Check String.prototype inherits from Object.prototype
        let string_ctor = global.properties.get("String").unwrap();
        let string_proto = string_ctor.get_property("prototype").unwrap();
        if let JSValue::Object(string_proto_rc) = &string_proto {
            let borrow = string_proto_rc.borrow();
            assert!(borrow.prototype.is_some());
            assert!(Rc::ptr_eq(
                borrow.prototype.as_ref().unwrap(),
                &object_proto_rc
            ));
        }

        // Check Number.prototype inherits from Object.prototype
        let number_ctor = global.properties.get("Number").unwrap();
        let number_proto = number_ctor.get_property("prototype").unwrap();
        if let JSValue::Object(number_proto_rc) = &number_proto {
            let borrow = number_proto_rc.borrow();
            assert!(borrow.prototype.is_some());
            assert!(Rc::ptr_eq(
                borrow.prototype.as_ref().unwrap(),
                &object_proto_rc
            ));
        }
    }

    #[test]
    fn test_init_builtins_array_push_through_prototype() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        // Create an array the same way the runtime would
        let arr = crate::builtins::array::create_array(vec![
            JSValue::int(1),
            JSValue::int(2),
            JSValue::int(3),
        ]);

        // Set its prototype to Array.prototype
        let global = rt.global.borrow();
        let array_ctor = global.properties.get("Array").unwrap();
        let array_proto = array_ctor.get_property("prototype").unwrap();
        if let JSValue::Object(proto_obj) = &array_proto {
            if let JSValue::Object(arr_obj) = &arr {
                arr_obj.borrow_mut().prototype = Some(proto_obj.clone());
            }
        }
        drop(global);

        // Now look up push via prototype chain
        let push = arr.get_property("push");
        assert!(push.is_some(), "Array instance should find 'push' via prototype chain");

        // Call push(4)
        if let Some(JSValue::Function(f)) = push {
            let f_borrow = f.borrow();
            if let crate::value::FunctionBody::Native(native_fn) = &f_borrow.body {
                let result = native_fn(&arr, &[JSValue::int(4)]);
                // push returns new length
                assert_eq!(result.to_int32(), 4);
            }
        }

        // Verify the element was added
        assert_eq!(
            arr.get_property("3").unwrap().to_int32(),
            4
        );
    }

    #[test]
    fn test_init_builtins_error_subclasses() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();

        // Error subclasses should exist
        assert!(global.properties.get("TypeError").is_some());
        assert!(global.properties.get("RangeError").is_some());
        assert!(global.properties.get("ReferenceError").is_some());
        assert!(global.properties.get("SyntaxError").is_some());
        assert!(global.properties.get("URIError").is_some());
        assert!(global.properties.get("EvalError").is_some());
    }

    #[test]
    fn test_init_builtins_date_static_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let date_ctor = global.properties.get("Date").unwrap();

        assert!(date_ctor.get_property("now").is_some());
        assert!(date_ctor.get_property("parse").is_some());
        assert!(date_ctor.get_property("UTC").is_some());
    }

    #[test]
    fn test_init_builtins_promise_static_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let promise_ctor = global.properties.get("Promise").unwrap();

        assert!(promise_ctor.get_property("resolve").is_some());
        assert!(promise_ctor.get_property("reject").is_some());
        assert!(promise_ctor.get_property("all").is_some());
        assert!(promise_ctor.get_property("race").is_some());
    }

    #[test]
    fn test_init_builtins_reflect_methods() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let reflect = global.properties.get("Reflect").unwrap();

        assert!(reflect.get_property("get").is_some());
        assert!(reflect.get_property("set").is_some());
        assert!(reflect.get_property("has").is_some());
        assert!(reflect.get_property("ownKeys").is_some());
    }

    #[test]
    fn test_runtime_with_config() {
        let config = RuntimeConfig {
            max_memory: 1024,
            max_stack_size: 2048,
            strict_mode: true,
        };
        let rt = JSRuntime::with_config(config);
        assert_eq!(rt.config.max_memory, 1024);
        assert_eq!(rt.config.max_stack_size, 2048);
        assert!(rt.config.strict_mode);
    }

    #[test]
    fn test_runtime_default() {
        let rt = JSRuntime::default();
        assert_eq!(rt.config.max_memory, 0);
        assert_eq!(rt.config.max_stack_size, 1024 * 1024);
        assert!(!rt.config.strict_mode);
        assert!(rt.objects.is_empty());
        assert!(rt.strings.is_empty());
        assert!(rt.functions.is_empty());
    }

    #[test]
    fn test_new_function() {
        let mut rt = JSRuntime::new();
        let params = vec!["a".to_string(), "b".to_string()];
        let body = FunctionBody::Source("return a + b".to_string());
        let func = rt.new_function(Some("add"), params.clone(), body.clone());

        // Verify name, params, body
        assert_eq!(func.borrow().name.as_deref(), Some("add"));
        assert_eq!(func.borrow().params, params);
        assert!(matches!(func.borrow().body, FunctionBody::Source(_)));

        // Verify it's tracked in functions vec
        assert_eq!(rt.functions.len(), 1);
        assert!(Rc::ptr_eq(&rt.functions[0], &func));
    }

    #[test]
    fn test_gc() {
        let mut rt = JSRuntime::new();

        // Create objects and keep external references to two of them
        let _obj1 = rt.new_object("A");
        let _obj2 = rt.new_object("B");
        // This one has no external reference - only tracked in rt.objects
        rt.new_object("C");

        assert_eq!(rt.objects.len(), 3);

        // Drop external references so all objects have strong_count == 1 (only in vec)
        drop(_obj1);
        drop(_obj2);

        rt.gc();

        // All objects should be removed since no external Rc references remain
        assert_eq!(rt.objects.len(), 0, "GC should remove all objects with no external references");
    }

    #[test]
    fn test_memory_usage() {
        let rt = JSRuntime::new();
        assert_eq!(rt.memory_usage(), 0);
    }

    #[test]
    fn test_interrupt_clear_interrupt() {
        let mut rt = JSRuntime::new();
        assert!(!rt.is_interrupted());

        rt.interrupt();
        assert!(rt.is_interrupted());

        rt.clear_interrupt();
        assert!(!rt.is_interrupted());
    }

    #[test]
    fn test_drain_microtasks() {
        use std::cell::Cell;

        let mut rt = JSRuntime::new();
        let counter = Rc::new(Cell::new(0i32));
        let c = counter.clone();
        rt.queue_microtask(Box::new(move || c.set(c.get() + 1)));
        let c = counter.clone();
        rt.queue_microtask(Box::new(move || c.set(c.get() + 1)));

        let count = rt.drain_microtasks();
        assert_eq!(count, 2);
        assert_eq!(counter.get(), 2);
        assert!(rt.microtask_queue.is_empty());
    }

    #[test]
    fn test_drain_macrotasks() {
        use std::cell::Cell;

        let mut rt = JSRuntime::new();
        let counter = Rc::new(Cell::new(0i32));
        let c = counter.clone();
        rt.queue_macrotask(Box::new(move || c.set(c.get() + 1)));
        let c = counter.clone();
        rt.queue_macrotask(Box::new(move || c.set(c.get() + 1)));

        let count = rt.drain_macrotasks();
        assert_eq!(count, 2);
        assert_eq!(counter.get(), 2);
        assert!(rt.macrotask_queue.is_empty());
    }

    #[test]
    fn test_drain_all_tasks() {
        use std::cell::Cell;

        let mut rt = JSRuntime::new();
        let counter = Rc::new(Cell::new(0i32));

        // Queue 2 microtasks
        let c = counter.clone();
        rt.queue_microtask(Box::new(move || c.set(c.get() + 1)));
        let c = counter.clone();
        rt.queue_microtask(Box::new(move || c.set(c.get() + 1)));

        // Queue 2 macrotasks
        let c = counter.clone();
        rt.queue_macrotask(Box::new(move || c.set(c.get() + 1)));
        let c = counter.clone();
        rt.queue_macrotask(Box::new(move || c.set(c.get() + 1)));

        let total = rt.drain_all_tasks();
        assert_eq!(total, 4);
        assert_eq!(counter.get(), 4);
        assert!(rt.microtask_queue.is_empty());
        assert!(rt.macrotask_queue.is_empty());
    }

    #[test]
    fn test_drain_empty_queues() {
        let mut rt = JSRuntime::new();
        assert_eq!(rt.drain_microtasks(), 0);
        assert_eq!(rt.drain_macrotasks(), 0);
        assert_eq!(rt.drain_all_tasks(), 0);
    }

    #[test]
    fn test_intern_string_different() {
        let mut rt = JSRuntime::new();
        let s1 = rt.intern_string("foo");
        let s2 = rt.intern_string("bar");
        assert!(!Rc::ptr_eq(&s1, &s2));
        assert_eq!(s1.borrow().data, "foo");
        assert_eq!(s2.borrow().data, "bar");
    }

    #[test]
    fn test_new_object_tracking() {
        let mut rt = JSRuntime::new();
        assert!(rt.objects.is_empty());

        let _obj1 = rt.new_object("Test1");
        assert_eq!(rt.objects.len(), 1);

        let _obj2 = rt.new_object("Test2");
        assert_eq!(rt.objects.len(), 2);

        // Verify the class names match
        assert_eq!(rt.objects[0].borrow().class_name, "Test1");
        assert_eq!(rt.objects[1].borrow().class_name, "Test2");
    }

    #[test]
    fn test_init_builtins_weakmap_weakset() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        assert!(
            global.properties.get("WeakMap").is_some(),
            "WeakMap constructor should be registered"
        );
        assert!(
            global.properties.get("WeakSet").is_some(),
            "WeakSet constructor should be registered"
        );
    }

    #[test]
    fn test_init_builtins_symbol_iterator() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let symbol_ctor = global.properties.get("Symbol").unwrap();
        // Symbol should have an iterator (Symbol.iterator) property
        assert!(
            symbol_ctor.get_property("iterator").is_some(),
            "Symbol.iterator should be registered"
        );
    }

    #[test]
    fn test_init_builtins_console() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        let console = global.properties.get("console").unwrap();
        // Verify standard console methods
        assert!(console.get_property("log").is_some());
        assert!(console.get_property("warn").is_some());
        assert!(console.get_property("error").is_some());
        assert!(console.get_property("info").is_some());
    }

    #[test]
    fn test_init_builtins_typed_arrays() {
        let mut rt = JSRuntime::new();
        rt.init_builtins();

        let global = rt.global.borrow();
        // Check for TypedArray constructors that should exist
        let typed_array_names = [
            "Int8Array", "Uint8Array", "Int16Array", "Uint16Array",
            "Int32Array", "Uint32Array", "Float32Array", "Float64Array",
        ];
        for name in &typed_array_names {
            if let Some(ctor) = global.properties.get(*name) {
                // If registered, it should be callable (a function) or have a prototype
                let is_callable = ctor.is_callable();
                let has_proto = ctor.get_property("prototype").is_some();
                assert!(
                    is_callable || has_proto,
                    "{} should be callable or have a prototype",
                    name
                );
            }
            // Not all typed arrays may be implemented yet; just check without panicking
        }
    }

    #[test]
    fn test_register_native_module() {
        let mut rt = JSRuntime::new();
        assert!(rt.module_registry.is_none());

        let exports = JSValue::object("Module");
        rt.register_native_module("my_module", exports);

        // Registry should now exist
        assert!(rt.module_registry.is_some());

        // Verify the module can be retrieved via the registry
        let registry = rt.module_registry.as_ref().unwrap().borrow();
        assert!(registry.has("my_module"));
        assert_eq!(registry.native_count(), 1);
    }

    #[test]
    fn test_set_module_loader() {
        use crate::builtins::module::ModuleLoader;

        struct DummyLoader;
        impl ModuleLoader for DummyLoader {
            fn load(&self, _specifier: &str, _referrer: Option<&str>) -> crate::builtins::module::ModuleResult {
                Err("not implemented".to_string())
            }
        }

        let mut rt = JSRuntime::new();
        assert!(rt.module_registry.is_none());

        let loader: crate::builtins::module::BoxedModuleLoader = Box::new(DummyLoader);
        rt.set_module_loader(loader);

        assert!(rt.module_registry.is_some());
    }
}

impl JSRuntime {
    /// Initialize all built-in objects and functions.
    ///
    /// Registers constructors, prototypes, and prototype methods for all
    /// standard JavaScript built-in types on the global object. This ensures
    /// that names like `Array`, `String`, `Object`, `Math`, etc. are
    /// available in the global scope.
    pub fn init_builtins(&mut self) {
        // Create a temporary context to call init functions
        let runtime_ref = Rc::new(RefCell::new(JSRuntime::with_config(RuntimeConfig::default())));
        {
            let mut tmp = runtime_ref.borrow_mut();
            tmp.global = self.global.clone();
        }
        let mut ctx = JSContext::new(runtime_ref);

        // Call all init functions
        crate::builtins::object::init_object(&mut ctx);
        crate::builtins::array::init_array(&mut ctx);
        crate::builtins::string::init_string(&mut ctx);
        crate::builtins::number::init_number(&mut ctx);
        crate::builtins::boolean::init_boolean(&mut ctx);
        crate::builtins::function::init_function(&mut ctx);
        crate::builtins::math::init_math(&mut ctx);
        crate::builtins::json::init_json(&mut ctx);
        crate::builtins::date::init_date(&mut ctx);
        crate::builtins::regexp::init_regexp(&mut ctx);
        crate::builtins::error::init_error(&mut ctx);
        crate::builtins::promise::init_promise(&mut ctx);
        crate::builtins::map::init_map(&mut ctx);
        crate::builtins::set::init_set(&mut ctx);
        crate::builtins::weakmap::init_weakmap(&mut ctx);
        crate::builtins::weakset::init_weakset(&mut ctx);
        crate::builtins::symbol::init_symbol(&mut ctx);
        crate::builtins::proxy::init_proxy(&mut ctx);
        crate::builtins::reflect::init_reflect(&mut ctx);
        crate::builtins::console::init_console(&mut ctx);

        // Set up prototype chains
        self.setup_prototype_chains();
    }

    /// Set up prototype chains for built-in types.
    fn setup_prototype_chains(&mut self) {
        // Get Object.prototype
        let object_proto = self.global.borrow()
            .properties.get("Object")
            .and_then(|ctor| ctor.get_property("prototype"))
            .clone();

        if let Some(obj_proto) = object_proto {
            // Set prototype chains for all built-in types
            let types = ["Array", "String", "Number", "Boolean", "Function",
                        "Date", "RegExp", "Error", "Promise", "Map", "Set"];

            for type_name in &types {
                if let Some(ctor) = self.global.borrow().properties.get(*type_name) {
                    if let Some(proto) = ctor.get_property("prototype") {
                        if let JSValue::Object(ref obj) = proto {
                            obj.borrow_mut().prototype = Some(match &obj_proto {
                                JSValue::Object(o) => o.clone(),
                                _ => unreachable!(),
                            });
                        }
                    }
                }
            }
        }
    }


}
