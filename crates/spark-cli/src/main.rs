#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Spark CLI - Pure Rust JavaScript Engine
//!
//! Command-line interface for the Spark JavaScript engine.

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::rc::Rc;

use spark_core::builtins::module::ModuleLoader;
use spark_core::context::JSContext;
use spark_core::host::{AsyncExecutor, Clock, HostEnvironment, Output, StdioOutput, SystemClock};
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;

// ============================================================================
// CLI async executor
// ============================================================================

/// CLI executor: processes microtasks immediately, queues macrotasks.
///
/// This is a basic event loop suitable for CLI usage:
/// - Microtasks (Promise callbacks) run immediately
/// - Macrotasks (setTimeout) are queued and drained after script execution
/// - `await` on pending Promises resolves synchronously (no real async I/O)
struct CliExecutor {
    /// Queued macrotasks (setTimeout callbacks)
    macrotasks: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl CliExecutor {
    fn new() -> Self {
        CliExecutor {
            macrotasks: RefCell::new(Vec::new()),
        }
    }

    /// Drain all pending macrotasks. Returns number executed.
    fn drain(&self) -> usize {
        let mut count = 0;
        loop {
            let task = self.macrotasks.borrow_mut().pop();
            match task {
                Some(t) => { t(); count += 1; }
                None => break,
            }
        }
        count
    }
}

impl AsyncExecutor for CliExecutor {
    fn enqueue_microtask(&self, task: Box<dyn FnOnce()>) {
        // Microtasks run immediately in CLI mode
        task();
    }

    fn enqueue_macrotask(&self, task: Box<dyn FnOnce()>, _delay_ms: u64) {
        // Queue for later draining (CLI doesn't have a real event loop)
        self.macrotasks.borrow_mut().push(task);
    }

    fn on_await(&self, promise: &JSValue, continuation: Box<dyn FnOnce(JSValue)>) {
        // Synchronously extract the Promise result
        let result = if let Some(state) = promise.get_property("__state") {
            match state {
                JSValue::Int(1) => promise.get_property("__result").unwrap_or(JSValue::undefined()),
                JSValue::Int(2) => promise.get_property("__result").unwrap_or(JSValue::undefined()),
                _ => JSValue::undefined(),
            }
        } else {
            JSValue::undefined()
        };
        continuation(result);
    }
}

// ============================================================================
// File-based module loader
// ============================================================================

/// File system module loader.
///
/// Resolves module specifiers to files on disk and compiles/executes them.
/// Supports:
/// - Relative paths: `./utils.js`, `../lib/helper.js`
/// - Absolute paths: `/usr/lib/module.js`
/// - Extension resolution: `./foo` → `./foo.js` → `./foo.mjs` → `./foo/index.js`
/// - Nested imports: imported modules can import other modules
struct FileModuleLoader {
    /// Base directory for resolving bare specifiers
    base_dir: PathBuf,
    /// Cache of already-loaded modules (resolved path → exports)
    cache: RefCell<HashMap<PathBuf, JSValue>>,
    /// Set of modules currently being loaded (for circular dependency detection)
    loading: RefCell<Vec<PathBuf>>,
}

impl FileModuleLoader {
    fn new() -> Self {
        FileModuleLoader {
            base_dir: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            cache: RefCell::new(HashMap::new()),
            loading: RefCell::new(Vec::new()),
        }
    }

    fn new_with_base(base_dir: PathBuf) -> Self {
        FileModuleLoader {
            base_dir,
            cache: RefCell::new(HashMap::new()),
            loading: RefCell::new(Vec::new()),
        }
    }

    /// Resolve a module specifier to an absolute file path.
    fn resolve(&self, specifier: &str, referrer: Option<&str>) -> Result<PathBuf, String> {
        let path = Path::new(specifier);

        // 1. Relative path: resolve against referrer's directory
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let base = match referrer {
                Some(r) => Path::new(r)
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
                None => self.base_dir.clone(),
            };
            return self.find_file(base.join(specifier));
        }

        // 2. Absolute path: use as-is
        if path.is_absolute() {
            return self.find_file(path.to_path_buf());
        }

        // 3. Bare specifier: resolve against base_dir
        self.find_file(self.base_dir.join(specifier))
    }

    /// Try to find the file with various extensions and index files.
    fn find_file(&self, path: PathBuf) -> Result<PathBuf, String> {
        if path.is_file() {
            return Ok(path);
        }
        let with_js = path.with_extension("js");
        if with_js.is_file() {
            return Ok(with_js);
        }
        let with_mjs = path.with_extension("mjs");
        if with_mjs.is_file() {
            return Ok(with_mjs);
        }
        if path.is_dir() {
            let index = path.join("index.js");
            if index.is_file() {
                return Ok(index);
            }
            let index_mjs = path.join("index.mjs");
            if index_mjs.is_file() {
                return Ok(index_mjs);
            }
        }
        Err(format!("Module not found: {}", path.display()))
    }

    /// Load a module file, compile, execute, and return its exports.
    fn load_file(&self, resolved: &Path) -> Result<JSValue, String> {
        let path_str = resolved.to_string_lossy().to_string();

        let source = fs::read_to_string(resolved)
            .map_err(|e| format!("Cannot read module '{}': {}", path_str, e))?;

        let bc = spark_core::compiler::compile_source(&source, Some(&path_str))
            .map_err(|e| format!("Syntax error in '{}': {}", path_str, e))?;

        // Create a fresh runtime for this module
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        rt.borrow_mut().init_builtins();

        // Set up nested module loading with shared cache
        let nested_loader = FileModuleLoader::new_with_base(self.base_dir.clone());
        *nested_loader.cache.borrow_mut() = self.cache.borrow().clone();
        rt.borrow_mut().set_module_loader(Box::new(nested_loader));

        let ctx = JSContext::new(rt);
        let mut interp = spark_core::interpreter::Interpreter::new(ctx);

        interp
            .execute(&bc)
            .map_err(|e| format!("Runtime error in '{}': {}", path_str, e))?;

        // Build exports from top-level variables
        let exports = JSValue::object("Module");
        for (i, local) in interp.saved_locals.iter().enumerate() {
            if i < interp.saved_var_names.len() {
                let name = &interp.saved_var_names[i];
                if !name.is_empty() && !name.starts_with("__") {
                    exports.set_property(name, local.borrow().clone());
                }
            }
        }

        Ok(exports)
    }
}

impl ModuleLoader for FileModuleLoader {
    fn load(&self, specifier: &str, referrer: Option<&str>) -> Result<JSValue, String> {
        let resolved = self.resolve(specifier, referrer)?;

        if let Some(cached) = self.cache.borrow().get(&resolved) {
            return Ok(cached.clone());
        }

        if self.loading.borrow().contains(&resolved) {
            return Ok(JSValue::object("Module"));
        }

        self.loading.borrow_mut().push(resolved.clone());
        let exports = self.load_file(&resolved)?;
        self.loading.borrow_mut().retain(|p| p != &resolved);
        self.cache.borrow_mut().insert(resolved, exports.clone());

        Ok(exports)
    }
}

// ============================================================================
// CLI entry point
// ============================================================================

fn main() {
    let stack_size = 64 * 1024 * 1024; // 64MB
    std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(|| real_main())
        .unwrap()
        .join()
        .unwrap();
}

fn real_main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            print_usage();
        }
        "--version" | "-v" => {
            println!("Spark v{}", env!("CARGO_PKG_VERSION"));
        }
        "--eval" | "-e" => {
            if args.len() < 3 {
                eprintln!("Error: --eval requires an argument");
                process::exit(1);
            }
            let code = &args[2];
            if let Err(e) = eval_string(code, None) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        "--interactive" | "-i" => {
            run_repl();
        }
        filename => {
            if let Err(e) = eval_file(filename) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

fn print_usage() {
    println!("Spark - Pure Rust JavaScript Engine");
    println!();
    println!("Usage:");
    println!("  spark [options] [filename]");
    println!();
    println!("Options:");
    println!("  --help, -h          Show this help");
    println!("  --version, -v       Show version");
    println!("  --eval, -e CODE     Evaluate code");
    println!("  --interactive, -i   Interactive mode");
}

/// Evaluate JavaScript code.
fn eval_string(code: &str, base_dir: Option<PathBuf>) -> Result<(), String> {
    use spark_core::compiler::Compiler;
    use spark_core::interpreter::Interpreter;
    use spark_core::parser::Parser;

    // Parse
    let mut parser = Parser::new(code);
    let ast = parser
        .parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    // Compile
    let mut compiler = Compiler::new();
    compiler
        .compile(&ast)
        .map_err(|e| format!("Compile error: {}", e))?;
    let bytecode = compiler.into_function();

    // Create runtime with CLI host environment
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    rt.borrow_mut().init_builtins();

    // Set up host environment
    let executor = Rc::new(CliExecutor::new());
    {
        let mut rt_mut = rt.borrow_mut();
        rt_mut.host = HostEnvironment {
            clock: Rc::new(SystemClock),
            output: Rc::new(StdioOutput),
            executor: Rc::new(spark_core::host::SyncExecutor),
        };
        // Set up module loader
        let loader = match base_dir {
            Some(dir) => FileModuleLoader::new_with_base(dir),
            None => FileModuleLoader::new(),
        };
        rt_mut.set_module_loader(Box::new(loader));
    }

    let rt_clone = rt.clone();
    let ctx = JSContext::new(rt);
    let mut interpreter = Interpreter::new(ctx);

    match interpreter.execute(&bytecode) {
        Ok(result) => {
            // Drain macrotasks (setTimeout callbacks)
            loop {
                let tasks = executor.drain();
                if tasks == 0 {
                    break;
                }
            }

            if !result.is_undefined() {
                println!("{}", result.to_string());
            }
        }
        Err(e) => {
            eprintln!("Runtime error: {}", e);
            return Err(e.to_string());
        }
    }
    Ok(())
}

fn eval_file(filename: &str) -> Result<(), String> {
    let code = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    let base_dir = Path::new(filename)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    eval_string(&code, Some(base_dir))
}

fn run_repl() {
    println!("Spark REPL");
    println!("Type .exit to quit");
    println!();

    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    rt.borrow_mut().init_builtins();
    rt.borrow_mut().set_module_loader(Box::new(FileModuleLoader::new()));
    let ctx = JSContext::new(rt);
    let mut interpreter = spark_core::interpreter::Interpreter::new(ctx);

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {
                let input = input.trim();
                if input == ".exit" {
                    break;
                }
                if input.is_empty() {
                    continue;
                }

                match spark_core::parser::Parser::new(input).parse() {
                    Ok(ast) => {
                        let mut compiler = spark_core::compiler::Compiler::new();
                        match compiler.compile(&ast) {
                            Ok(()) => {
                                let bytecode = compiler.into_function();
                                match interpreter.execute(&bytecode) {
                                    Ok(result) => {
                                        if !result.is_undefined() {
                                            println!("{}", result.to_string());
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Compile error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Parse error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    println!("Goodbye!");
}
