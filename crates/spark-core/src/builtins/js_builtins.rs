//! JavaScript-implemented built-in functions.
//!
//! These functions are compiled to bytecode at initialization time,
//! eliminating the need for unsafe native-to-bytecode callback bridges.

use crate::compiler::compile_source;
use crate::value::{JSValue, FunctionBody, BytecodeFunction};

/// Compile a JavaScript function declaration into a JSValue::Function.
/// The source should be a function declaration like:
///   "function name(a, b) { return a + b; }"
pub fn compile_js_function(source: &str) -> Result<JSValue, String> {
    let bc = compile_source(source, None).map_err(|e| e.to_string())?;
    // The compiled source is a script; the function declaration is in bc.functions
    let inner = bc.functions.into_iter().next()
        .ok_or_else(|| "No function found in source".to_string())?;
    let name = inner.name.clone();
    let params = inner.params.clone();
    Ok(JSValue::function(
        name.as_deref(),
        params,
        FunctionBody::Bytecode(inner),
    ))
}

/// JavaScript implementations of Array.prototype methods that use callbacks.
/// These are compiled to bytecode so callbacks (arrow functions) work natively.
pub const ARRAY_METHODS_JS: &str = r#"
function __array_forEach(fn) {
    let len = this.length;
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            fn(this[i], i, this);
        }
    }
}

function __array_map(fn) {
    let len = this.length;
    let result = new Array(len);
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            result[i] = fn(this[i], i, this);
        }
    }
    return result;
}

function __array_filter(fn) {
    let len = this.length;
    let result = [];
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                result.push(this[i]);
            }
        }
    }
    return result;
}

function __array_reduce(fn, initial) {
    let len = this.length;
    let i = 0;
    let acc;
    if (initial !== undefined) {
        acc = initial;
    } else {
        if (len === 0) {
            throw new TypeError('Reduce of empty array with no initial value');
        }
        acc = this[0];
        i = 1;
    }
    for (; i < len; i = i + 1) {
        if (i in this) {
            acc = fn(acc, this[i], i, this);
        }
    }
    return acc;
}

function __array_reduceRight(fn, initial) {
    let len = this.length;
    let i = len - 1;
    let acc;
    if (initial !== undefined) {
        acc = initial;
    } else {
        if (len === 0) {
            throw new TypeError('Reduce of empty array with no initial value');
        }
        i = len - 1;
        acc = this[i];
        i = i - 1;
    }
    for (; i >= 0; i = i - 1) {
        if (i in this) {
            acc = fn(acc, this[i], i, this);
        }
    }
    return acc;
}

function __array_find(fn) {
    let len = this.length;
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                return this[i];
            }
        }
    }
}

function __array_findIndex(fn) {
    let len = this.length;
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                return i;
            }
        }
    }
    return -1;
}

function __array_findLast(fn) {
    let len = this.length;
    for (let i = len - 1; i >= 0; i = i - 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                return this[i];
            }
        }
    }
}

function __array_findLastIndex(fn) {
    let len = this.length;
    for (let i = len - 1; i >= 0; i = i - 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                return i;
            }
        }
    }
    return -1;
}

function __array_some(fn) {
    let len = this.length;
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            if (fn(this[i], i, this)) {
                return true;
            }
        }
    }
    return false;
}

function __array_every(fn) {
    let len = this.length;
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            if (!fn(this[i], i, this)) {
                return false;
            }
        }
    }
    return true;
}

function __array_flatMap(fn) {
    let len = this.length;
    let result = [];
    for (let i = 0; i < len; i = i + 1) {
        if (i in this) {
            let mapped = fn(this[i], i, this);
            if (Array.isArray(mapped)) {
                let mlen = mapped.length;
                for (let j = 0; j < mlen; j = j + 1) {
                    result.push(mapped[j]);
                }
            } else {
                result.push(mapped);
            }
        }
    }
    return result;
}

function __array_sort(cmp) {
    let len = this.length;
    let arr = [];
    for (let i = 0; i < len; i = i + 1) {
        arr.push(this[i]);
    }
    if (typeof cmp === 'function') {
        arr.sort(cmp);
    } else {
        arr.sort();
    }
    for (let i = 0; i < len; i = i + 1) {
        this[i] = arr[i];
    }
    return this;
}
"#;

/// Map from internal function names to JavaScript method names.
const ARRAY_METHOD_MAP: &[(&str, &str)] = &[
    ("__array_forEach", "forEach"),
    ("__array_map", "map"),
    ("__array_filter", "filter"),
    ("__array_reduce", "reduce"),
    ("__array_reduceRight", "reduceRight"),
    ("__array_find", "find"),
    ("__array_findIndex", "findIndex"),
    ("__array_findLast", "findLast"),
    ("__array_findLastIndex", "findLastIndex"),
    ("__array_some", "some"),
    ("__array_every", "every"),
    ("__array_flatMap", "flatMap"),
];

/// Compile all JS-implemented array methods and return them as a Vec of (name, JSValue).
pub fn compile_array_js_methods() -> Vec<(String, JSValue)> {
    let mut methods = Vec::new();

    let js_source = format!("{}\nfunction __stub() {{}}", ARRAY_METHODS_JS);
    let bc = match compile_source(&js_source, None) {
        Ok(bc) => bc,
        Err(e) => {
            if let Some(output) = crate::interpreter::get_output() {
                output.write_stderr(&format!("Failed to compile array JS methods: {}", e));
            }
            return methods;
        },
    };

    for inner in bc.functions {
        let internal_name = match inner.name.clone() {
            Some(n) => n,
            None => continue,
        };
        // Map internal name to public method name
        let public_name = ARRAY_METHOD_MAP.iter()
            .find(|(internal, _)| *internal == internal_name)
            .map(|(_, public)| *public)
            .unwrap_or(&internal_name);
        let params = inner.params.clone();
        let func = JSValue::function(
            Some(public_name),
            params,
            FunctionBody::Bytecode(inner),
        );
        methods.push((public_name.to_string(), func));
    }

    methods
}

/// JavaScript implementations of Promise.prototype methods.
pub const PROMISE_METHODS_JS: &str = r#"
function __promise_createResolved(value) {
    let p = new Promise(function() {});
    p.__state = 1;
    p.__result = value;
    return p;
}

function __promise_createRejected(reason) {
    let p = new Promise(function() {});
    p.__state = 2;
    p.__result = reason;
    return p;
}

function __promise_then(onFulfilled, onRejected) {
    let state = this.__state;
    let result = this.__result;
    let newPromise = new Promise(function() {});

    if (state === 1) {
        if (typeof onFulfilled === 'function') {
            let value = onFulfilled(result);
            newPromise.__state = 1;
            newPromise.__result = value;
        } else {
            newPromise.__state = 1;
            newPromise.__result = result;
        }
    } else if (state === 2) {
        if (typeof onRejected === 'function') {
            let value = onRejected(result);
            newPromise.__state = 1;
            newPromise.__result = value;
        } else {
            newPromise.__state = 2;
            newPromise.__result = result;
        }
    } else {
        this.__reactions.push({ onFulfilled: onFulfilled, onRejected: onRejected, promise: newPromise });
    }

    return newPromise;
}

function __promise_catch(onRejected) {
    return this.then(undefined, onRejected);
}

function __promise_finally(onFinally) {
    return this.then(
        function(value) {
            if (typeof onFinally === 'function') { onFinally(); }
            return value;
        },
        function(reason) {
            if (typeof onFinally === 'function') { onFinally(); }
            throw reason;
        }
    );
}
"#;

const PROMISE_METHOD_MAP: &[(&str, &str)] = &[
    ("__promise_then", "then"),
    ("__promise_catch", "catch"),
    ("__promise_finally", "finally"),
];

/// Compile all JS-implemented Promise methods and return them as a Vec of (name, JSValue).
pub fn compile_promise_js_methods() -> Vec<(String, JSValue)> {
    let mut methods = Vec::new();

    let js_source = format!("{}\nfunction __stub() {{}}", PROMISE_METHODS_JS);
    let bc = match compile_source(&js_source, None) {
        Ok(bc) => bc,
        Err(e) => {
            if let Some(output) = crate::interpreter::get_output() {
                output.write_stderr(&format!("Failed to compile Promise JS methods: {}", e));
            }
            return methods;
        },
    };

    for inner in bc.functions {
        let internal_name = match inner.name.clone() {
            Some(n) => n,
            None => continue,
        };
        let public_name = PROMISE_METHOD_MAP.iter()
            .find(|(internal, _)| *internal == internal_name)
            .map(|(_, public)| *public)
            .unwrap_or(&internal_name);
        let params = inner.params.clone();
        let func = JSValue::function(
            Some(public_name),
            params,
            FunctionBody::Bytecode(inner),
        );
        methods.push((public_name.to_string(), func));
    }

    methods
}

