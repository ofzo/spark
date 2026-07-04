//! Module loading interface.
//!
//! The engine defines the interface for module loading.
//! The execution environment (CLI, browser, embedded) provides the implementation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::JSValue;

/// Result of loading a module.
pub type ModuleResult = Result<JSValue, String>;

/// Trait for module loaders.
///
/// The execution environment implements this trait to provide module loading
/// capabilities. The engine calls `load()` when it encounters an `import` statement.
///
/// # Example
///
/// ```ignore
/// struct FileModuleLoader { base_dir: PathBuf }
///
/// impl ModuleLoader for FileModuleLoader {
///     fn load(&self, specifier: &str, referrer: Option<&str>) -> ModuleResult {
///         let path = resolve_path(specifier, referrer, &self.base_dir);
///         let source = fs::read_to_string(&path)?;
///         let bc = compile_source(&source)?;
///         // execute and return exports...
///     }
/// }
/// ```
pub trait ModuleLoader {
    /// Load a module and return its exports object.
    ///
    /// - `specifier`: The module specifier (e.g. "./math.js", "lodash")
    /// - `referrer`: The path of the importing module (for relative resolution)
    ///
    /// Returns a JSValue::Object containing the module's exports.
    fn load(&self, specifier: &str, referrer: Option<&str>) -> ModuleResult;
}

/// Type-erased module loader stored in the runtime.
pub type BoxedModuleLoader = Box<dyn ModuleLoader>;

/// Module loader registry with caching and native module support.
///
/// Wraps a `ModuleLoader` implementation and adds:
/// - Caching of loaded modules
/// - Circular dependency detection
/// - Native module registration (Rust modules importable via `import`)
pub struct ModuleRegistry {
    /// The actual loader implementation provided by the host environment
    loader: BoxedModuleLoader,
    /// Pre-registered native modules (specifier -> exports)
    native_modules: HashMap<String, JSValue>,
    /// Cache of loaded modules (specifier -> exports)
    cache: HashMap<String, JSValue>,
    /// Modules currently being loaded (for circular dependency detection)
    loading: Vec<String>,
}

impl ModuleRegistry {
    /// Create a new module registry with the given loader.
    pub fn new(loader: BoxedModuleLoader) -> Self {
        ModuleRegistry {
            loader,
            native_modules: HashMap::new(),
            cache: HashMap::new(),
            loading: Vec::new(),
        }
    }

    /// Register a native module.
    ///
    /// Native modules are Rust-implemented modules that can be imported
    /// via `import` statements just like JavaScript modules.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let fs_module = JSValue::object("Module");
    /// fs_module.set_property("readFile", JSValue::function(...));
    /// fs_module.set_property("writeFile", JSValue::function(...));
    /// registry.register_native("fs", fs_module);
    ///
    /// // Now in JavaScript:
    /// // import { readFile } from "fs";
    /// ```
    pub fn register_native(&mut self, specifier: &str, exports: JSValue) {
        self.native_modules.insert(specifier.to_string(), exports);
    }

    /// Load a module with caching, native module lookup, and circular dependency detection.
    pub fn load(&mut self, specifier: &str, referrer: Option<&str>) -> ModuleResult {
        // 1. Check native modules first
        if let Some(native) = self.native_modules.get(specifier) {
            return Ok(native.clone());
        }

        // 2. Check cache
        if let Some(cached) = self.cache.get(specifier) {
            return Ok(cached.clone());
        }

        // 3. Check for circular dependency
        if self.loading.contains(&specifier.to_string()) {
            return Ok(JSValue::object("Module"));
        }

        // 4. Mark as loading
        self.loading.push(specifier.to_string());

        // 5. Delegate to the host-provided loader
        let result = self.loader.load(specifier, referrer);

        // 6. Remove from loading set
        self.loading.pop();

        // 7. Cache successful result
        if let Ok(ref exports) = result {
            self.cache.insert(specifier.to_string(), exports.clone());
        }

        result
    }

    /// Check if a module is available (native or cached).
    pub fn has(&self, specifier: &str) -> bool {
        self.native_modules.contains_key(specifier) || self.cache.contains_key(specifier)
    }

    /// Get the number of cached modules (excluding native).
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Get the number of registered native modules.
    pub fn native_count(&self) -> usize {
        self.native_modules.len()
    }
}
