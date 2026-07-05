//! Lightweight test262 runner for Spark JavaScript engine.
//!
//! Walks test262 test files, parses YAML frontmatter,
//! executes tests in sloppy/strict mode, reports results.

#![allow(unused)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::rc::Rc;
use std::time::Instant;

use spark_core::context::JSContext;
use spark_core::runtime::JSRuntime;
use spark_core::value::JSValue;

// ============================================================================
// Test262 harness JS — injected before each test file.
// ============================================================================

const HARNESS_PRELUDE: &str = r#"
var Test262Error = function(msg) {
    this.name = 'Test262Error';
    this.message = msg || '';
};
Test262Error.prototype = Object.create(Error.prototype);

function $DONOTEVALUATE() {
    throw new Test262Error('This code should not be evaluated');
}

var assert = (function() {
    function _isSameValue(a, b) {
        if (a === b) {
            return a !== 0 || 1 / a === 1 / b;
        }
        return a !== a && b !== b;
    }

    function sameValue(actual, expected, msg) {
        if (!_isSameValue(actual, expected)) {
            throw new Test262Error(
                (msg ? msg + ': ' : '') +
                'Expected SameValue(' + JSON.stringify(expected) + ', ' + JSON.stringify(actual) + ') to be true'
            );
        }
    }

    function notSameValue(actual, unexpected, msg) {
        if (_isSameValue(actual, unexpected)) {
            throw new Test262Error(
                (msg ? msg + ': ' : '') +
                'Expected SameValue(' + JSON.stringify(unexpected) + ', ' + JSON.stringify(actual) + ') to be false'
            );
        }
    }

    function _throws(expectedError, fn, msg) {
        var threw = false;
        var exc;
        try {
            fn();
        } catch (e) {
            threw = true;
            exc = e;
        }
        if (!threw) {
            throw new Test262Error(msg || 'Expected function to throw, but it did not');
        }
        if (expectedError !== undefined && expectedError !== null) {
            if (typeof expectedError === 'function') {
                if (!(exc instanceof expectedError)) {
                    throw new Test262Error(
                        (msg ? msg + ': ' : '') +
                        'Expected ' + (expectedError.name || 'error') +
                        ' but got ' + (exc && exc.name ? exc.name : typeof exc)
                    );
                }
            } else if (exc !== expectedError && (exc && exc.constructor !== expectedError)) {
                throw new Test262Error(
                    (msg ? msg + ': ' : '') +
                    'Expected ' + String(expectedError) + ' but got ' + String(exc)
                );
            }
        }
        return exc;
    }

    function _assert(v, msg) {
        if (!v) {
            throw new Test262Error(msg || 'assertion failed');
        }
    }

    var api = function(value, message) { _assert(value, message); };
    api.sameValue = sameValue;
    api.notSameValue = notSameValue;
    api.throws = _throws;
    return api;
})();
"#;

// ============================================================================
// Metadata types
// ============================================================================

struct TestMetadata {
    description: String,
    flags: Vec<String>,
    negative: Option<NegativeInfo>,
    includes: Vec<String>,
    features: Vec<String>,
}

struct NegativeInfo {
    phase: String,
    error_type: Option<String>,
}

// ============================================================================
// Metadata parser — minimal YAML subset, no dependencies.
// ============================================================================

fn parse_metadata(source: &str) -> Option<(TestMetadata, usize)> {
    let start = source.find("/*---")?;
    let end = source[start..].find("---*/")?;
    let yaml_block = &source[start + 5..start + end];

    let mut meta = TestMetadata {
        description: String::new(),
        flags: Vec::new(),
        negative: None,
        includes: Vec::new(),
        features: Vec::new(),
    };

    let mut in_negative = false;
    let mut neg_phase = String::new();
    let mut neg_type: Option<String> = None;

    for raw in yaml_block.lines() {
        let line = raw.trim();
        let indented = raw.starts_with("  ");

        // Skip empty lines, metadata headers, and continuation lines
        if line.is_empty() || line.starts_with("info:") || line.starts_with("esid:")
            || line.starts_with("es5id:") || line.starts_with("es6id:") || line.starts_with("description:")
        {
            in_negative = false;
            continue;
        }

        // Indented lines inside a negative: block
        if indented && in_negative {
            if line.starts_with("phase:") {
                neg_phase = line.strip_prefix("phase:").unwrap().trim().to_string();
            } else if line.starts_with("type:") {
                neg_type = Some(line.strip_prefix("type:").unwrap().trim().to_string());
            }
            continue;
        }

        // Skip other indented lines (multi-line string continuations)
        if indented {
            continue;
        }

        in_negative = false;

        if line == "flags:" || line.starts_with("flags: [") {
            meta.flags = parse_yaml_list(line);
        } else if line == "includes:" || line.starts_with("includes: [") {
            meta.includes = parse_yaml_list(line);
        } else if line == "features:" || line.starts_with("features: [") {
            meta.features = parse_yaml_list(line);
        } else if line == "negative:" {
            in_negative = true;
            neg_phase = String::new();
            neg_type = None;
        }
    }

    if !neg_phase.is_empty() {
        meta.negative = Some(NegativeInfo {
            phase: neg_phase,
            error_type: neg_type,
        });
    }

    let content_start = start + end + 5; // after ---*/
    Some((meta, content_start))
}

fn parse_yaml_list(first_line: &str) -> Vec<String> {
    let mut items = Vec::new();

    // Handle inline list: flags: [a, b]
    if let Some(bracket) = first_line.find('[') {
        let inner = &first_line[bracket + 1..];
        let inner = inner.trim_end_matches(']');
        for item in inner.split(',') {
            let item = item.trim().trim_matches('"').trim_matches('\'');
            if !item.is_empty() {
                items.push(item.to_string());
            }
        }
    }
    // Multi-line list not fully handled here (would need lookahead),
    // but test262 rarely uses multi-line flags/includes/features.

    items
}

// ============================================================================
// Test execution
// ============================================================================

enum TestResult {
    Pass,
    Fail(String),
    Skip(String),
}

struct TestStats {
    pass: usize,
    fail: usize,
    skip: usize,
    failures: Vec<(PathBuf, String)>, // (path, error message)
}

fn create_runtime() -> (Rc<RefCell<JSRuntime>>, JSContext) {
    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    rt.borrow_mut().init_builtins();
    let ctx = JSContext::new(rt.clone());
    (rt, ctx)
}

fn compile_only(code: &str, strict: bool) -> Result<(), String> {
    use spark_core::compiler::Compiler;
    use spark_core::parser::Parser;

    let mut parser = Parser::new(code);
    parser.set_strict_mode(strict);
    let ast = parser.parse().map_err(|e| format!("Parse error: {}", e))?;
    let mut compiler = Compiler::new();
    compiler.compile(&ast).map_err(|e| format!("Compile error: {}", e))?;
    Ok(())
}

fn eval_test(rt: &Rc<RefCell<JSRuntime>>, code: &str, strict: bool) -> Result<String, String> {
    use spark_core::compiler::Compiler;
    use spark_core::interpreter::Interpreter;
    use spark_core::parser::Parser;

    let mut parser = Parser::new(code);
    parser.set_strict_mode(strict);
    let ast = parser.parse().map_err(|e| format!("Parse error: {}", e))?;
    let mut compiler = Compiler::new();
    compiler.compile(&ast).map_err(|e| format!("Compile error: {}", e))?;
    let bc = compiler.into_function();

    let ctx = JSContext::new(rt.clone());
    let mut interp = Interpreter::new(ctx);
    interp.execute(&bc).map(|v| v.to_string()).map_err(|e| format!("Runtime error: {}", e))
}

fn run_test_file(path: &Path) -> TestResult {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return TestResult::Skip(format!("Cannot read: {}", e)),
    };

    let (meta, content_start) = match parse_metadata(&source) {
        Some(m) => m,
        None => return TestResult::Skip("No metadata found".into()),
    };

    let test_code = &source[content_start..];

    // Skip unsupported test types
    if meta.flags.contains(&"module".to_string()) {
        return TestResult::Skip("module test".into());
    }
    if meta.flags.contains(&"async".to_string()) {
        return TestResult::Skip("async test".into());
    }
    if meta.flags.contains(&"raw".to_string()) {
        return TestResult::Skip("raw test (needs custom wrapper)".into());
    }
    if !meta.includes.is_empty() {
        return TestResult::Skip(format!("includes: {:?} (helpers not supported)", meta.includes));
    }

    // Determine modes for runtime tests
    let only_strict = meta.flags.contains(&"onlyStrict".to_string());
    let no_strict = meta.flags.contains(&"noStrict".to_string());

    // Handle parse-phase negative tests: compile test code only (no harness),
    // expect a parse error matching the expected type.
    if let Some(ref neg) = meta.negative {
        if neg.phase == "parse" {
            let strict = only_strict;
            let result = compile_only(test_code, strict);
            match result {
                Ok(()) => {
                    return TestResult::Fail(
                        "Expected parse error, but test parsed successfully".into()
                    );
                }
                Err(e) => {
                    let expected = neg.error_type.as_deref().unwrap_or("Error");
                    if e.to_lowercase().contains(&expected.to_lowercase()) {
                        return TestResult::Pass;
                    }
                    return TestResult::Fail(format!(
                        "Expected {} but got: {}", expected, e
                    ));
                }
            }
        }
    }

    let modes: Vec<(&str, bool)> = if only_strict {
        vec![("strict", true)]
    } else if no_strict {
        vec![("sloppy", false)]
    } else {
        vec![("sloppy", false), ("strict", true)]
    };

    for (mode_name, use_strict) in &modes {
        let mut code = String::new();

        if *use_strict {
            code.push_str("\"use strict\";\n");
        }

        code.push_str(HARNESS_PRELUDE);
        code.push_str("\n// === TEST262 TEST ===\n");
        code.push_str(test_code);

        if let Some(ref neg) = meta.negative {
            // Runtime/resolution phase negative test
            let (rt, _) = create_runtime();
            let result = eval_test(&rt, &code, *use_strict);
            match result {
                Ok(_) => {
                    return TestResult::Fail(format!(
                        "[{}] Expected runtime error ({}), but test passed",
                        mode_name,
                        neg.error_type.as_deref().unwrap_or("any")
                    ));
                }
                Err(e) => {
                    if let Some(ref expected_type) = neg.error_type {
                        let el = e.to_lowercase();
                        if el.contains(&expected_type.to_lowercase()) {
                            // pass — expected error thrown
                        } else {
                            return TestResult::Fail(format!(
                                "[{}] Wrong error: expected {}, got: {}",
                                mode_name, expected_type, e
                            ));
                        }
                    }
                }
            }
            if mode_name == &modes.last().unwrap().0 {
                return TestResult::Pass;
            }
        } else {
            // Normal test
            let (rt, _) = create_runtime();
            let result = eval_test(&rt, &code, *use_strict);
            match result {
                Ok(_) => {}
                Err(e) => {
                    let msg = e.lines().last().unwrap_or(&e).to_string();
                    return TestResult::Fail(format!("[{}] {}", mode_name, msg));
                }
            }
        }
    }

    TestResult::Pass
}

// ============================================================================
// Directory walker
// ============================================================================

fn collect_test_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }

    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        if let Ok(entries) = fs::read_dir(&d) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip harness directory
                    if path.file_name().map_or(false, |n| n == "harness") {
                        continue;
                    }
                    dirs.push(path);
                } else if path.extension().map_or(false, |e| e == "js") {
                    files.push(path);
                }
            }
        }
    }
    files
}

// ============================================================================
// Main
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
    let mut verbose = false;
    let mut path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--verbose" | "-v" => verbose = true,
            "--path" | "-p" => {
                i += 1;
                if i < args.len() {
                    path = Some(PathBuf::from(&args[i]));
                }
            }
            _ => {
                if path.is_none() {
                    path = Some(PathBuf::from(&args[i]));
                }
            }
        }
        i += 1;
    }

    let test_dir = path.unwrap_or_else(|| {
        // Default: look for test262 relative to workspace root
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("../../tests/test262/test");
        p
    });

    if !test_dir.is_dir() {
        eprintln!(
            "Error: test262 directory not found at '{}'. \n\
             Run: git submodule update --init --depth 1",
            test_dir.display()
        );
        process::exit(1);
    }

    eprintln!("Collecting test files from {} ...", test_dir.display());
    let files = collect_test_files(&test_dir);
    eprintln!("Found {} test files", files.len());

    let mut stats = TestStats {
        pass: 0,
        fail: 0,
        skip: 0,
        failures: Vec::new(),
    };

    let start = Instant::now();
    let total = files.len();

    for (idx, file) in files.iter().enumerate() {
        let rel = file.strip_prefix(&test_dir).unwrap_or(file);

        if verbose || (idx + 1) % 100 == 0 {
            eprint!(
                "\r[{}/{}] pass={} fail={} skip={} ...",
                idx + 1,
                total,
                stats.pass,
                stats.fail,
                stats.skip
            );
        }

        match run_test_file(file) {
            TestResult::Pass => stats.pass += 1,
            TestResult::Fail(msg) => {
                stats.fail += 1;
                stats.failures.push((file.clone(), msg));
                if verbose {
                    eprintln!("\nFAIL: {} — {}", rel.display(), stats.failures.last().unwrap().1);
                }
            }
            TestResult::Skip(_) => stats.skip += 1,
        }
    }

    let elapsed = start.elapsed();

    // Final report
    println!();
    println!("========================================");
    println!("  test262 Runner Results");
    println!("========================================");
    println!("  Total:  {}", total);
    println!("  Pass:   {} ({:.1}%)", stats.pass,
        if total > 0 { stats.pass as f64 / total as f64 * 100.0 } else { 0.0 });
    println!("  Fail:   {} ({:.1}%)", stats.fail,
        if total > 0 { stats.fail as f64 / total as f64 * 100.0 } else { 0.0 });
    println!("  Skip:   {} ({:.1}%)", stats.skip,
        if total > 0 { stats.skip as f64 / total as f64 * 100.0 } else { 0.0 });
    println!("  Time:   {:.1}s", elapsed.as_secs_f64());
    println!("----------------------------------------");

    // Print failures at the end
    if !stats.failures.is_empty() {
        let show = stats.failures.len().min(50);
        println!();
        println!("First {} failures:", show);
        for (i, (path, msg)) in stats.failures.iter().take(50).enumerate() {
            let rel = path.strip_prefix(&test_dir).unwrap_or(path);
            println!("  {}. {}", i + 1, rel.display());
            for line in msg.lines() {
                println!("     {}", line);
            }
        }
        if stats.failures.len() > 50 {
            println!("  ... and {} more failures", stats.failures.len() - 50);
        }
    }

    if stats.fail > 0 {
        process::exit(1);
    }
}
