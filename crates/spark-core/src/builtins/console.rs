//! Console built-in.
//!
//! Implements console.log, console.error, print, and related functions.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Global functions
// ============================================================================

/// parseInt(string, radix)
fn global_parse_int(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let radix = args.get(1).map(|v| v.to_int32()).unwrap_or(0);
    let s = s.trim();
    if s.is_empty() {
        return JSValue::float(f64::NAN);
    }
    let (sign, s) = if s.starts_with('-') {
        (-1.0, &s[1..])
    } else if s.starts_with('+') {
        (1.0, &s[1..])
    } else {
        (1.0, s)
    };
    let radix = if radix == 0 {
        if s.starts_with("0x") || s.starts_with("0X") { 16 } else { 10 }
    } else {
        radix as u32
    };
    let s = if radix == 16 && (s.starts_with("0x") || s.starts_with("0X")) {
        &s[2..]
    } else {
        s
    };
    let mut result: f64 = 0.0;
    let mut found = false;
    for c in s.chars() {
        let digit = match c {
            '0'..='9' => (c as u8 - b'0') as u32,
            'a'..='z' => (c as u8 - b'a' + 10) as u32,
            'A'..='Z' => (c as u8 - b'A' + 10) as u32,
            _ => break,
        };
        if digit >= radix {
            break;
        }
        result = result * radix as f64 + digit as f64;
        found = true;
    }
    if !found {
        return JSValue::float(f64::NAN);
    }
    JSValue::float(sign * result)
}

/// parseFloat(string)
fn global_parse_float(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let s = s.trim();
    if s.is_empty() {
        return JSValue::float(f64::NAN);
    }
    // Try to parse as much as possible
    let mut end = 0;
    let chars: Vec<char> = s.chars().collect();
    if chars[0] == '-' || chars[0] == '+' {
        end = 1;
    }
    let mut has_dot = false;
    let mut has_exp = false;
    while end < chars.len() {
        match chars[end] {
            '0'..='9' => end += 1,
            '.' if !has_dot => { has_dot = true; end += 1; }
            'e' | 'E' if !has_exp => {
                has_exp = true;
                end += 1;
                if end < chars.len() && (chars[end] == '+' || chars[end] == '-') {
                    end += 1;
                }
            }
            _ => break,
        }
    }
    if end == 0 || (end == 1 && (chars[0] == '-' || chars[0] == '+')) {
        return JSValue::float(f64::NAN);
    }
    let s: String = chars[..end].iter().collect();
    match s.parse::<f64>() {
        Ok(v) => JSValue::float(v),
        Err(_) => JSValue::float(f64::NAN),
    }
}

/// isNaN(value)
fn global_is_nan(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).cloned().unwrap_or(JSValue::undefined());
    JSValue::bool(val.to_number().is_nan())
}

/// isFinite(value)
fn global_is_finite(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let n = val.to_number();
    JSValue::bool(!n.is_nan() && !n.is_infinite())
}

/// encodeURI(uri)
fn global_encode_uri(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let uri = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let mut result = String::new();
    for byte in uri.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' |
            b';' | b',' | b'/' | b'?' | b':' | b'@' | b'&' | b'=' | b'+' |
            b'$' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' | b'#' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    JSValue::string(&result)
}

/// decodeURI(uri)
fn global_decode_uri(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let uri = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let mut result = Vec::new();
    let bytes = uri.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1]);
            let lo = hex_digit(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    match String::from_utf8(result) {
        Ok(s) => JSValue::string(&s),
        Err(_) => JSValue::string(&uri),
    }
}

/// encodeURIComponent(uriComponent)
fn global_encode_uri_component(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let uri = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let mut result = String::new();
    for byte in uri.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' |
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    JSValue::string(&result)
}

/// eval(code) - compiles and executes JavaScript code
fn global_eval(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let code = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    if code.is_empty() {
        return JSValue::undefined();
    }

    // Compile the code
    let bc = match crate::compiler::compile_source(&code, Some("<eval>")) {
        Ok(bc) => bc,
        Err(_) => {
            return JSValue::undefined();
        }
    };

    // Try to use the current interpreter's context (for scope-aware eval)
    if let Some(ctx) = crate::interpreter::get_eval_context() {
        let mut interp = crate::interpreter::Interpreter::new(ctx);
        match interp.execute(&bc) {
            Ok(result) => result,
            Err(_) => JSValue::undefined(),
        }
    } else {
        // Fallback: create a new runtime
        let rt = std::rc::Rc::new(std::cell::RefCell::new(crate::runtime::JSRuntime::new()));
        rt.borrow_mut().init_builtins();
        let ctx = crate::context::JSContext::new(rt);
        let mut interp = crate::interpreter::Interpreter::new(ctx);
        match interp.execute(&bc) {
            Ok(result) => result,
            Err(_) => JSValue::undefined(),
        }
    }
}

/// Dynamic import: import(specifier) - returns a Promise that resolves with the module exports
fn global_dynamic_import(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let specifier = args.get(0).map(|v| v.to_string()).unwrap_or_default();

    // Try to use the module registry from the current runtime
    let result = if let Some(ctx) = crate::interpreter::get_eval_context() {
        let rt = ctx.runtime.clone();
        let rt_borrow = rt.borrow();
        if let Some(ref registry) = rt_borrow.module_registry {
            let mut registry = registry.borrow_mut();
            registry.load(&specifier, None)
        } else {
            Err("No module loader configured".to_string())
        }
    } else {
        Err("No runtime context available".to_string())
    };

    // Create a resolved Promise with the result
    match result {
        Ok(exports) => {
            let promise = JSValue::object("Promise");
            promise.set_property("__state", JSValue::Int(1)); // FULFILLED
            promise.set_property("__result", exports);
            promise.set_property("__reactions", crate::builtins::array::create_array(vec![]));
            promise
        }
        Err(_) => {
            let promise = JSValue::object("Promise");
            promise.set_property("__state", JSValue::Int(2)); // REJECTED
            promise.set_property("__result", JSValue::string("Module not found"));
            promise.set_property("__reactions", crate::builtins::array::create_array(vec![]));
            promise
        }
    }
}

/// decodeURIComponent(uriComponent)
fn global_decode_uri_component(_this: &JSValue, args: &[JSValue]) -> JSValue {
    // Same as decodeURI but decodes all percent-encoded characters
    global_decode_uri(_this, args)
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Initialize console built-in functions.
pub fn init_console(ctx: &mut JSContext) {
    let global = ctx.runtime.borrow().global.clone();

    // Register `print` as a global function
    let print_func = JSValue::function(Some("print"), vec!["...args".to_string()], FunctionBody::Native(print_impl));
    global.borrow_mut().properties.insert("print".to_string(), print_func);

    // Create console object
    let console = JSValue::object("Console");

    let log_func = JSValue::function(Some("log"), vec!["...args".to_string()], FunctionBody::Native(console_log_impl));
    console.set_property("log", log_func);

    let error_func = JSValue::function(Some("error"), vec!["...args".to_string()], FunctionBody::Native(console_error_impl));
    console.set_property("error", error_func);

    let warn_func = JSValue::function(Some("warn"), vec!["...args".to_string()], FunctionBody::Native(console_warn_impl));
    console.set_property("warn", warn_func);

    let info_func = JSValue::function(Some("info"), vec!["...args".to_string()], FunctionBody::Native(console_info_impl));
    console.set_property("info", info_func);

    global.borrow_mut().properties.insert("console".to_string(), console);

    // Register timer functions
    global.borrow_mut().properties.insert("setTimeout".to_string(),
        JSValue::function(Some("setTimeout"), vec!["callback".to_string(), "delay".to_string()], FunctionBody::Native(set_timeout_impl)));
    global.borrow_mut().properties.insert("clearTimeout".to_string(),
        JSValue::function(Some("clearTimeout"), vec!["id".to_string()], FunctionBody::Native(clear_timeout_impl)));
    global.borrow_mut().properties.insert("setInterval".to_string(),
        JSValue::function(Some("setInterval"), vec!["callback".to_string(), "delay".to_string()], FunctionBody::Native(set_interval_impl)));
    global.borrow_mut().properties.insert("clearInterval".to_string(),
        JSValue::function(Some("clearInterval"), vec!["id".to_string()], FunctionBody::Native(clear_interval_impl)));

    // Register global functions
    global.borrow_mut().properties.insert("parseInt".to_string(),
        JSValue::function(Some("parseInt"), vec!["string".to_string(), "radix".to_string()], FunctionBody::Native(global_parse_int)));
    global.borrow_mut().properties.insert("parseFloat".to_string(),
        JSValue::function(Some("parseFloat"), vec!["string".to_string()], FunctionBody::Native(global_parse_float)));
    global.borrow_mut().properties.insert("isNaN".to_string(),
        JSValue::function(Some("isNaN"), vec!["value".to_string()], FunctionBody::Native(global_is_nan)));
    global.borrow_mut().properties.insert("isFinite".to_string(),
        JSValue::function(Some("isFinite"), vec!["value".to_string()], FunctionBody::Native(global_is_finite)));
    global.borrow_mut().properties.insert("encodeURI".to_string(),
        JSValue::function(Some("encodeURI"), vec!["uri".to_string()], FunctionBody::Native(global_encode_uri)));
    global.borrow_mut().properties.insert("decodeURI".to_string(),
        JSValue::function(Some("decodeURI"), vec!["uri".to_string()], FunctionBody::Native(global_decode_uri)));
    global.borrow_mut().properties.insert("encodeURIComponent".to_string(),
        JSValue::function(Some("encodeURIComponent"), vec!["uriComponent".to_string()], FunctionBody::Native(global_encode_uri_component)));
    global.borrow_mut().properties.insert("decodeURIComponent".to_string(),
        JSValue::function(Some("decodeURIComponent"), vec!["uriComponent".to_string()], FunctionBody::Native(global_decode_uri_component)));

    // Register eval
    global.borrow_mut().properties.insert("eval".to_string(),
        JSValue::function(Some("eval"), vec!["code".to_string()], FunctionBody::Native(global_eval)));

    // Register dynamic import
    global.borrow_mut().properties.insert("__dynamic_import__".to_string(),
        JSValue::function(Some("import"), vec!["specifier".to_string()], FunctionBody::Native(global_dynamic_import)));
}

/// Format arguments to a string, space-separated.
fn format_args(args: &[JSValue]) -> String {
    args.iter()
        .map(|a| a.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `print(...args)` - prints to stdout with newline.
fn print_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = format_args(args);
    if let Some(output) = crate::interpreter::get_output() {
        output.write_stdout(&text);
    } else {
        println!("{}", text);
    }
    JSValue::undefined()
}

/// `console.log(...args)` - prints to stdout with newline.
fn console_log_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = format_args(args);
    if let Some(output) = crate::interpreter::get_output() {
        output.write_stdout(&text);
    } else {
        println!("{}", text);
    }
    JSValue::undefined()
}

/// `console.error(...args)` - prints to stderr with newline.
fn console_error_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = format_args(args);
    if let Some(output) = crate::interpreter::get_output() {
        output.write_stderr(&text);
    } else {
        eprintln!("{}", text);
    }
    JSValue::undefined()
}

/// `console.warn(...args)` - prints to stderr with newline.
fn console_warn_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = format_args(args);
    if let Some(output) = crate::interpreter::get_output() {
        output.write_stderr(&text);
    } else {
        eprintln!("{}", text);
    }
    JSValue::undefined()
}

/// `console.info(...args)` - prints to stdout with newline.
fn console_info_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let text = format_args(args);
    if let Some(output) = crate::interpreter::get_output() {
        output.write_stdout(&text);
    } else {
        println!("{}", text);
    }
    JSValue::undefined()
}

/// Pending callbacks queue for setTimeout/setInterval.
/// The host environment drains this after script execution.
thread_local! {
    static PENDING_CALLBACKS: RefCell<Vec<JSValue>> = RefCell::new(Vec::new());
}

/// Take pending callbacks for execution by the host.
pub fn take_pending_callbacks() -> Vec<JSValue> {
    PENDING_CALLBACKS.with(|callbacks| {
        std::mem::take(&mut *callbacks.borrow_mut())
    })
}

/// `setTimeout(callback, delay)` - schedules callback for later execution.
fn set_timeout_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let callback = args.first().cloned().unwrap_or(JSValue::undefined());
    PENDING_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().push(callback);
    });
    JSValue::Int(1) // timer ID
}

/// `clearTimeout(id)` - clears a timeout (no-op in synchronous mode).
fn clear_timeout_impl(_this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::undefined()
}

/// `setInterval(callback, delay)` - repeatedly executes callback.
/// In synchronous mode, executes once immediately.
/// `setInterval(callback, delay)` - queues callback for repeated execution.
fn set_interval_impl(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let callback = args.first().cloned().unwrap_or(JSValue::undefined());
    PENDING_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().push(callback);
    });
    JSValue::Int(1) // interval ID
}

/// `clearInterval(id)` - clears an interval (no-op in synchronous mode).
fn clear_interval_impl(_this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::undefined()
}
