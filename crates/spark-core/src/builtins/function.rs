#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Function built-in.
//!
//! Implements the JavaScript Function constructor and its methods.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, JSFunction, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Function constructor
// ============================================================================

/// Function constructor - `new Function(params, body)` or `Function(params, body)`
pub fn function_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    // Parse parameters and body from arguments
    let mut params = Vec::new();
    let mut body = String::from("{}");

    let args_len = args.len();
    if args_len == 0 {
        // No params, no body
    } else if args_len == 1 {
        body = args[0].to_string();
    } else {
        // All args except last are parameter names
        for i in 0..args_len - 1 {
            let param_str = args[i].to_string();
            for param in param_str.split(',') {
                let param = param.trim().to_string();
                if !param.is_empty() {
                    params.push(param);
                }
            }
        }
        body = args[args_len - 1].to_string();
    }

    JSValue::function(None, params, FunctionBody::Source(body))
}

// ============================================================================
// Function.prototype methods
// ============================================================================

/// Function.prototype.call(thisArg, ...args)
///
/// Calls the function with a given `this` value and arguments provided
/// individually. The first argument is the `this` binding; subsequent
/// arguments are passed to the function.
pub fn function_call(this: &JSValue, args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Function(f) => {
            let this_arg = args.first().cloned().unwrap_or(JSValue::undefined());
            let call_args: Vec<JSValue> = args.get(1..).unwrap_or(&[]).to_vec();

            let func = f.borrow();
            match &func.body {
                FunctionBody::Native(native_fn) => {
                    native_fn(&this_arg, &call_args)
                }
                FunctionBody::Closure(closure_fn) => {
                    closure_fn(&this_arg, &call_args)
                }
                _ => JSValue::undefined(),
            }
        }
        _ => JSValue::undefined(),
    }
}

/// Function.prototype.apply(thisArg, argsArray)
///
/// Calls the function with a given `this` value and arguments provided
/// as an array (or array-like object).
pub fn function_apply(this: &JSValue, args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Function(f) => {
            let this_arg = args.get(0).cloned().unwrap_or(JSValue::undefined());
            let call_args = match args.get(1) {
                Some(JSValue::Object(arr)) => {
                    // Extract arguments from the array-like object
                    let length = arr.borrow().properties.get("length")
                        .map(|v| v.to_number() as usize)
                        .unwrap_or(0);
                    let mut call_args = Vec::with_capacity(length);
                    for i in 0..length {
                        let val = arr.borrow().properties.get(&i.to_string())
                            .cloned()
                            .unwrap_or(JSValue::undefined());
                        call_args.push(val);
                    }
                    call_args
                }
                Some(JSValue::Undefined) | None => Vec::new(),
                _ => Vec::new(),
            };

            let func = f.borrow();
            match &func.body {
                FunctionBody::Native(native_fn) => {
                    native_fn(&this_arg, &call_args)
                }
                FunctionBody::Closure(closure_fn) => {
                    closure_fn(&this_arg, &call_args)
                }
                _ => {
                    // For bytecode functions, we can't easily call from here
                    // without an interpreter reference. Return undefined.
                    JSValue::undefined()
                }
            }
        }
        _ => JSValue::undefined(),
    }
}

/// Function.prototype.bind(thisArg, ...args)
///
/// Creates a new function that, when called, has its `this` set to the
/// provided value, with a given sequence of arguments preceding any
/// provided when the new function is called.
///
/// The returned bound function has:
/// - `length`: max(0, original.length - boundArgs.length)
/// - `name`: "bound " + original.name
pub fn function_bind(this: &JSValue, args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Function(f) => {
            let this_arg = args.first().cloned().unwrap_or(JSValue::undefined());
            let bound_args: Vec<JSValue> = args.get(1..).unwrap_or(&[]).to_vec();

            // Clone references we need to capture in the closure
            let original_fn = f.clone();
            let original_name = f.borrow().name.clone();
            let original_params = f.borrow().params.clone();
            let original_param_count = original_params.len() as i32;
            let bound_count = bound_args.len() as i32;

            // The bound function name is "bound <originalName>"
            let bound_name = match original_name.as_deref() {
                Some(n) if !n.is_empty() => format!("bound {}", n),
                _ => "bound".to_string(),
            };

            // Create a proper closure that captures the bound state
            let bound_fn = JSValue::function(
                Some(&bound_name),
                original_params,
                FunctionBody::Closure(Rc::new(move |this, call_args| {
                    // Merge bound args with call-time args
                    let mut all_args = bound_args.clone();
                    all_args.extend_from_slice(call_args);

                    // Use the bound this, or fall back to the call-time this
                    let call_this = if this_arg.is_undefined() && this.is_undefined() {
                        JSValue::undefined()
                    } else if this_arg.is_undefined() {
                        this.clone()
                    } else {
                        this_arg.clone()
                    };

                    // Delegate to the original function
                    let func = original_fn.borrow();
                    match &func.body {
                        FunctionBody::Native(native_fn) => native_fn(&call_this, &all_args),
                        FunctionBody::Closure(closure_fn) => closure_fn(&call_this, &all_args),
                        _ => JSValue::undefined(),
                    }
                })),
            );

            // length = max(0, originalParamCount - boundArgsCount)
            let length = (original_param_count - bound_count).max(0);
            bound_fn.set_property("length", JSValue::int(length));

            bound_fn
        }
        _ => JSValue::undefined(),
    }
}

/// Function.prototype.toString()
///
/// Returns a string representing the source code of the function.
pub fn function_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Function(f) => {
            let func = f.borrow();
            match &func.body {
                FunctionBody::Native(_) => {
                    let name = func.name.clone().unwrap_or_default();
                    if name.is_empty() {
                        JSValue::string("function() { [native code] }")
                    } else {
                        JSValue::string(&format!("function {}() {{ [native code] }}", name))
                    }
                }
                FunctionBody::Source(source) => {
                    let name = func.name.clone().unwrap_or_default();
                    if name.is_empty() {
                        JSValue::string(&format!("function({}) {{ {} }}",
                            func.params.join(", "), source))
                    } else {
                        JSValue::string(&format!("function {}({}) {{ {} }}",
                            name, func.params.join(", "), source))
                    }
                }
                FunctionBody::Bytecode(_) => {
                    let name = func.name.clone().unwrap_or_default();
                    if name.is_empty() {
                        JSValue::string("function() { [bytecode] }")
                    } else {
                        JSValue::string(&format!("function {}() {{ [bytecode] }}", name))
                    }
                }
                FunctionBody::Closure(_) => {
                    let name = func.name.clone().unwrap_or_default();
                    if name.is_empty() {
                        JSValue::string("function() { [native code] }")
                    } else {
                        JSValue::string(&format!("function {}() {{ [native code] }}", name))
                    }
                }
                FunctionBody::Generator { .. } => {
                    let name = func.name.clone().unwrap_or_default();
                    if name.is_empty() {
                        JSValue::string("function*() { [generator] }")
                    } else {
                        JSValue::string(&format!("function* {}() {{ [generator] }}", name))
                    }
                }
                FunctionBody::GeneratorNext { .. } => {
                    JSValue::string("function next() { [native code] }")
                }
            }
        }
        _ => JSValue::undefined(),
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Function constructor and prototype.
pub fn init_function(ctx: &mut JSContext) {
    // Create the Function constructor function
    let constructor = JSValue::function(
        Some("Function"),
        vec!["...args".to_string()],
        FunctionBody::Native(function_constructor),
    );

    // Create Function.prototype
    let prototype = JSValue::object("Function");

    // Add prototype methods
    prototype.set_property("call", JSValue::function(
        Some("call"),
        vec!["thisArg".to_string()],
        FunctionBody::Native(function_call),
    ));
    prototype.set_property("apply", JSValue::function(
        Some("apply"),
        vec!["thisArg".to_string(), "argsArray".to_string()],
        FunctionBody::Native(function_apply),
    ));
    prototype.set_property("bind", JSValue::function(
        Some("bind"),
        vec!["thisArg".to_string()],
        FunctionBody::Native(function_bind),
    ));
    prototype.set_property("toString", JSValue::function(
        Some("toString"),
        vec![],
        FunctionBody::Native(function_to_string),
    ));

    // Add name and length as explicit prototype properties
    // (they are also handled by JSValue::get_property for Function, but
    //  having them on the prototype ensures they appear in for..in and
    //  Object.keys(Function.prototype))
    prototype.set_property("name", JSValue::string(""));
    prototype.set_property("length", JSValue::int(0));

    // Set Function.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Set Function on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("Function".to_string(), constructor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_function_to_string() {
        let func = JSValue::function(
            Some("test"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, _args| JSValue::undefined()),
        );
        let result = function_to_string(&func, &[]);
        assert_eq!(result.to_string(), "function test() { [native code] }");
    }

    #[test]
    fn test_function_call() {
        let func = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::float(a + b)
            }),
        );

        let result = function_call(&func, &[JSValue::undefined(), JSValue::int(3), JSValue::int(4)]);
        assert_eq!(result.to_number(), 7.0);
    }

    #[test]
    fn test_function_call_with_this() {
        let func = JSValue::function(
            Some("getVal"),
            vec![],
            FunctionBody::Native(|this, _args| {
                this.get_property("x").unwrap_or(JSValue::undefined())
            }),
        );

        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(42));

        let result = function_call(&func, &[obj]);
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_function_apply() {
        let func = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::float(a + b)
            }),
        );

        let args_arr = JSValue::object("Array");
        args_arr.set_property("0", JSValue::int(5));
        args_arr.set_property("1", JSValue::int(6));
        args_arr.set_property("length", JSValue::int(2));

        let result = function_apply(&func, &[JSValue::undefined(), args_arr]);
        assert_eq!(result.to_number(), 11.0);
    }

    #[test]
    fn test_function_apply_with_this() {
        let func = JSValue::function(
            Some("getVal"),
            vec![],
            FunctionBody::Native(|this, _args| {
                this.get_property("val").unwrap_or(JSValue::undefined())
            }),
        );

        let obj = JSValue::object("Object");
        obj.set_property("val", JSValue::int(99));

        let result = function_apply(&func, &[obj, JSValue::object("Array")]);
        assert_eq!(result.to_int32(), 99);
    }

    #[test]
    fn test_function_bind_basic() {
        let func = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::float(a + b)
            }),
        );

        // Bind with this = undefined and first arg = 10
        let bound = function_bind(&func, &[JSValue::undefined(), JSValue::int(10)]);

        // Call the bound function with second arg
        let result = function_call(&bound, &[JSValue::undefined(), JSValue::int(20)]);
        assert_eq!(result.to_number(), 30.0);

        // Check name
        let name = bound.get_property("name").unwrap();
        assert_eq!(name.to_string(), "bound add");

        // Check length: original 2 params - 1 bound arg = 1
        let length = bound.get_property("length").unwrap();
        assert_eq!(length.to_int32(), 1);
    }

    #[test]
    fn test_function_bind_this() {
        let func = JSValue::function(
            Some("getX"),
            vec![],
            FunctionBody::Native(|this, _args| {
                this.get_property("x").unwrap_or(JSValue::undefined())
            }),
        );

        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(77));

        // Bind this to obj
        let bound = function_bind(&func, &[obj]);
        let result = function_call(&bound, &[]);
        assert_eq!(result.to_int32(), 77);
    }

    #[test]
    fn test_function_bind_override_this() {
        let func = JSValue::function(
            Some("getX"),
            vec![],
            FunctionBody::Native(|this, _args| {
                this.get_property("x").unwrap_or(JSValue::undefined())
            }),
        );

        let bound_this = JSValue::object("Object");
        bound_this.set_property("x", JSValue::int(1));

        let override_this = JSValue::object("Object");
        override_this.set_property("x", JSValue::int(2));

        // Bind this to bound_this
        let bound = function_bind(&func, &[bound_this]);
        // When calling with a different this, the bound this should win
        let result = function_call(&bound, &[override_this]);
        assert_eq!(result.to_int32(), 1);
    }

    #[test]
    fn test_function_bind_no_args() {
        let func = JSValue::function(
            Some("noop"),
            vec![],
            FunctionBody::Native(|_this, _args| JSValue::int(42)),
        );

        let bound = function_bind(&func, &[]);
        let result = function_call(&bound, &[]);
        assert_eq!(result.to_int32(), 42);

        let length = bound.get_property("length").unwrap();
        assert_eq!(length.to_int32(), 0);
    }

    #[test]
    fn test_function_bind_all_params_bound() {
        let func = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::float(a + b)
            }),
        );

        let bound = function_bind(&func, &[
            JSValue::undefined(),
            JSValue::int(3),
            JSValue::int(4),
        ]);

        // length should be 0 (2 params - 2 bound args)
        let length = bound.get_property("length").unwrap();
        assert_eq!(length.to_int32(), 0);

        // Calling with extra args still works
        let result = function_call(&bound, &[JSValue::undefined()]);
        assert_eq!(result.to_number(), 7.0);
    }

    #[test]
    fn test_function_bind_non_function() {
        let result = function_bind(&JSValue::int(42), &[JSValue::undefined()]);
        assert!(result.is_undefined());
    }

    #[test]
    fn test_init_function() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        init_function(&mut ctx);
        let func = ctx.global.borrow().properties.get("Function").cloned();
        assert!(func.is_some());

        // Check Function.prototype has the expected methods
        let func_val = func.unwrap();
        let proto = func_val.get_property("prototype").unwrap();
        assert!(proto.get_property("call").is_some());
        assert!(proto.get_property("apply").is_some());
        assert!(proto.get_property("bind").is_some());
        assert!(proto.get_property("toString").is_some());
        assert!(proto.get_property("name").is_some());
        assert!(proto.get_property("length").is_some());
    }
}
