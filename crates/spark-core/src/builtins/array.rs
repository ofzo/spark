#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Array built-in.
//!
//! Implements the JavaScript Array constructor and its methods.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Array helpers
// ============================================================================

/// Create a new array object with elements.
pub fn create_array(elements: Vec<JSValue>) -> JSValue {
    let arr = JSValue::object("Array");
    let len = elements.len();
    for (i, elem) in elements.into_iter().enumerate() {
        arr.set_property(&i.to_string(), elem);
    }
    arr.set_property("length", JSValue::int(len as i32));
    arr
}

/// Get the length of an array-like object.
pub fn array_length(arr: &JSValue) -> usize {
    match arr {
        JSValue::Object(obj) => obj.borrow()
            .properties
            .get("length")
            .map(|v| v.to_number() as usize)
            .unwrap_or(0),
        _ => 0,
    }
}

/// Get an element at a numeric index.
pub fn array_get(arr: &JSValue, index: usize) -> JSValue {
    arr.get_property(&index.to_string()).unwrap_or(JSValue::undefined())
}

/// Set an element at a numeric index.
pub fn array_set(arr: &JSValue, index: usize, value: JSValue) {
    arr.set_property(&index.to_string(), value);
}

/// Delete an element at a numeric index (set to undefined).
pub fn array_delete(arr: &JSValue, index: usize) {
    arr.set_property(&index.to_string(), JSValue::undefined());
}

/// Set the length property.
fn set_length(arr: &JSValue, len: usize) {
    arr.set_property("length", JSValue::int(len as i32));
}

/// Check if a value is an array.
fn is_array(val: &JSValue) -> bool {
    match val {
        JSValue::Object(obj) => obj.borrow().class_name == "Array",
        _ => false,
    }
}

// ============================================================================
// Array constructor
// ============================================================================

/// Array constructor - `new Array(length)` or `new Array(...items)`
pub fn array_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    if args.len() == 1 {
        match &args[0] {
            JSValue::Int(n) if *n >= 0 => {
                let arr = JSValue::object("Array");
                set_length(&arr, *n as usize);
                return arr;
            }
            JSValue::Float(f) if *f >= 0.0 && f.fract() == 0.0 => {
                let arr = JSValue::object("Array");
                set_length(&arr, *f as usize);
                return arr;
            }
            _ => {}
        }
    }

    create_array(args.to_vec())
}

// ============================================================================
// Array static methods
// ============================================================================

/// Array.isArray(value)
pub fn array_is_array(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let default = JSValue::undefined();
    let val = args.first().unwrap_or(&default);
    JSValue::bool(is_array(val))
}

/// Array.from(arrayLike, mapFn)
pub fn array_from(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let source = match args.first() {
        Some(v) => v,
        None => return create_array(vec![]),
    };

    let length = source.get_property("length")
        .map(|v| v.to_number() as usize)
        .unwrap_or(0);

    let mut elements = Vec::with_capacity(length);
    for i in 0..length {
        let val = source.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
        elements.push(val);
    }

    create_array(elements)
}

/// Array.of(...items)
pub fn array_of(_this: &JSValue, args: &[JSValue]) -> JSValue {
    create_array(args.to_vec())
}

// ============================================================================
// Array.prototype methods
// ============================================================================

/// Array.prototype.push(...items) -> new length
pub fn array_push(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    for (i, arg) in args.iter().enumerate() {
        array_set(this, len + i, arg.clone());
    }
    let new_len = len + args.len();
    set_length(this, new_len);
    JSValue::int(new_len as i32)
}

/// Array.prototype.pop() -> removed element
pub fn array_pop(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    if len == 0 {
        set_length(this, 0);
        return JSValue::undefined();
    }
    let last_idx = len - 1;
    let val = array_get(this, last_idx);
    array_delete(this, last_idx);
    set_length(this, last_idx);
    val
}

/// Array.prototype.shift() -> removed element
pub fn array_shift(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    if len == 0 {
        return JSValue::undefined();
    }
    let val = array_get(this, 0);
    // Shift all elements down
    for i in 1..len {
        let next = array_get(this, i);
        array_set(this, i - 1, next);
    }
    array_delete(this, len - 1);
    set_length(this, len - 1);
    val
}

/// Array.prototype.unshift(...items) -> new length
pub fn array_unshift(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let new_len = len + args.len();

    // Shift elements to make room (from end to avoid overwriting)
    for i in (0..len).rev() {
        let val = array_get(this, i);
        array_set(this, i + args.len(), val);
    }

    // Insert new items at the beginning
    for (i, arg) in args.iter().enumerate() {
        array_set(this, i, arg.clone());
    }

    set_length(this, new_len);
    JSValue::int(new_len as i32)
}

/// Array.prototype.includes(searchElement, fromIndex)
pub fn array_includes(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let search = args.first().cloned().unwrap_or(JSValue::undefined());
    let from_index = args.get(1)
        .map(|v| {
            let n = v.to_int32();
            if n < 0 {
                (len as i32 + n).max(0) as usize
            } else {
                n as usize
            }
        })
        .unwrap_or(0);

    for i in from_index..len {
        let val = array_get(this, i);
        if val.strict_eq(&search) {
            return JSValue::bool(true);
        }
        // Also check NaN === NaN (special case: NaN is never === NaN, but includes uses SameValueZero)
        if let (JSValue::Float(a), JSValue::Float(b)) = (&val, &search) {
            if a.is_nan() && b.is_nan() {
                return JSValue::bool(true);
            }
        }
        if let (JSValue::Float(a), JSValue::Int(b)) = (&val, &search) {
            if a.is_nan() && (*b as f64).is_nan() {
                return JSValue::bool(true);
            }
        }
        if let (JSValue::Int(a), JSValue::Float(b)) = (&val, &search) {
            if (*a as f64).is_nan() && b.is_nan() {
                return JSValue::bool(true);
            }
        }
    }

    JSValue::bool(false)
}

/// Array.prototype.indexOf(searchElement, fromIndex)
pub fn array_index_of(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let search = args.first().cloned().unwrap_or(JSValue::undefined());
    let from_index = args.get(1)
        .map(|v| {
            let n = v.to_int32();
            if n < 0 {
                (len as i32 + n).max(0) as usize
            } else {
                n as usize
            }
        })
        .unwrap_or(0);

    for i in from_index..len {
        let val = array_get(this, i);
        if val.strict_eq(&search) {
            return JSValue::int(i as i32);
        }
    }

    JSValue::int(-1)
}

/// Array.prototype.lastIndexOf(searchElement, fromIndex)
pub fn array_last_index_of(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let search = args.first().cloned().unwrap_or(JSValue::undefined());
    let from_index = args.get(1)
        .map(|v| {
            let n = v.to_int32();
            if n < 0 {
                (len as i32 + n).max(0) as usize
            } else {
                n.min(len as i32 - 1).max(0) as usize
            }
        })
        .unwrap_or(len.saturating_sub(1));

    for i in (0..=from_index).rev() {
        let val = array_get(this, i);
        if val.strict_eq(&search) {
            return JSValue::int(i as i32);
        }
    }

    JSValue::int(-1)
}

/// Array.prototype.reverse()
pub fn array_reverse(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let mid = len / 2;
    for i in 0..mid {
        let j = len - 1 - i;
        let a = array_get(this, i);
        let b = array_get(this, j);
        array_set(this, i, b);
        array_set(this, j, a);
    }
    this.clone()
}

/// Array.prototype.slice(start, end)
pub fn array_slice(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let start = normalize_slice_index(
        args.get(0).map(|v| v.to_int32()).unwrap_or(0),
        len as isize,
    );
    let end = normalize_slice_index(
        args.get(1).map(|v| v.to_int32()).unwrap_or(len as i32),
        len as isize,
    );

    let count = end.saturating_sub(start);
    let mut elements = Vec::with_capacity(count);
    for i in start..start + count {
        elements.push(array_get(this, i));
    }
    create_array(elements)
}

/// Array.prototype.splice(start, deleteCount, ...items)
pub fn array_splice(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let start = normalize_slice_index(
        args.first().map(|v| v.to_int32()).unwrap_or(0),
        len as isize,
    );

    let delete_count = if args.len() > 1 {
        let dc = args[1].to_int32().max(0) as usize;
        dc.min(len.saturating_sub(start))
    } else {
        // If deleteCount not provided, delete from start to end
        len.saturating_sub(start)
    };

    let new_items: Vec<JSValue> = args.get(2..).unwrap_or(&[]).to_vec();

    // Collect deleted elements
    let mut deleted = Vec::with_capacity(delete_count);
    for i in start..start + delete_count {
        deleted.push(array_get(this, i));
    }

    // Shift elements to make room or close gap
    let diff = new_items.len() as isize - delete_count as isize;

    if diff > 0 {
        // Expanding: shift right from end
        for i in (start + delete_count..len).rev() {
            let val = array_get(this, i);
            array_set(this, (i as isize + diff) as usize, val);
        }
    } else if diff < 0 {
        // Contracting: shift left
        for i in start + delete_count..len {
            let val = array_get(this, i);
            array_set(this, (i as isize + diff) as usize, val);
        }
        // Clear old trailing elements
        for i in (len as isize + diff) as usize..len {
            array_delete(this, i);
        }
    }

    // Insert new items
    for (i, item) in new_items.into_iter().enumerate() {
        array_set(this, start + i, item);
    }

    // Update length
    set_length(this, (len as isize + diff) as usize);

    create_array(deleted)
}

/// Array.prototype.concat(...values)
pub fn array_concat(this: &JSValue, args: &[JSValue]) -> JSValue {
    let mut elements = Vec::new();

    // Add elements from this
    let len = array_length(this);
    for i in 0..len {
        elements.push(array_get(this, i));
    }

    // Add elements from each argument
    for arg in args {
        if is_array(arg) {
            let arg_len = array_length(arg);
            for i in 0..arg_len {
                elements.push(array_get(arg, i));
            }
        } else {
            elements.push(arg.clone());
        }
    }

    create_array(elements)
}

/// Array.prototype.join(separator)
pub fn array_join(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let sep = match args.first() {
        Some(JSValue::Undefined) | None => ",".to_string(),
        Some(v) => v.to_string(),
    };

    if len == 0 {
        return JSValue::string("");
    }

    let mut result = String::new();
    for i in 0..len {
        if i > 0 {
            result.push_str(&sep);
        }
        let val = array_get(this, i);
        match &val {
            JSValue::Undefined | JSValue::Null => {}
            _ => result.push_str(&val.to_string()),
        }
    }

    JSValue::string(&result)
}

/// Array.prototype.flat(depth)
pub fn array_flat(this: &JSValue, args: &[JSValue]) -> JSValue {
    let depth = args.get(0).map(|v| v.to_int32()).unwrap_or(1).max(0) as usize;
    let mut result = Vec::new();

    fn flat_into(arr: &JSValue, depth: usize, result: &mut Vec<JSValue>) {
        let len = array_length(arr);
        for i in 0..len {
            let val = array_get(arr, i);
            if is_array(&val) && depth > 0 {
                flat_into(&val, depth - 1, result);
            } else {
                result.push(val);
            }
        }
    }

    flat_into(this, depth, &mut result);
    create_array(result)
}

/// Array.prototype.fill(value, start, end)
///
/// Fills elements from `start` (inclusive) to `end` (exclusive) with `value`.
/// `start` defaults to 0; `end` defaults to `this.length`.
pub fn array_fill(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let value = args.first().cloned().unwrap_or(JSValue::undefined());
    let start = normalize_slice_index(
        args.get(1).map(|v| v.to_int32()).unwrap_or(0),
        len as isize,
    );
    // JS spec: end defaults to this.length (exclusive bound)
    let end = normalize_slice_index(
        args.get(2).map(|v| v.to_int32()).unwrap_or(len as i32),
        len as isize,
    );

    for i in start..end {
        array_set(this, i, value.clone());
    }

    this.clone()
}

/// Array.prototype.copyWithin(target, start, end)
pub fn array_copy_within(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let target = normalize_slice_index(
        args.first().map(|v| v.to_int32()).unwrap_or(0),
        len as isize,
    );
    let start = normalize_slice_index(
        args.get(1).map(|v| v.to_int32()).unwrap_or(0),
        len as isize,
    );
    let end = normalize_slice_index(
        args.get(2).map(|v| v.to_int32()).unwrap_or(len as i32),
        len as isize,
    );

    let count = if end > start { (end - start).min(len - target) } else { 0 };

    // Collect source elements first (to handle overlapping ranges)
    let source: Vec<JSValue> = (0..count).map(|i| array_get(this, start + i)).collect();
    for (i, val) in source.into_iter().enumerate() {
        array_set(this, target + i, val);
    }

    this.clone()
}

// ============================================================================
// Array.prototype.sort (native implementation)
// ============================================================================

/// Array.prototype.sort(compareFn)
pub fn array_sort(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let cmp_fn = args.get(0).filter(|v| v.is_callable());

    // Collect elements
    let mut elements: Vec<JSValue> = (0..len).map(|i| array_get(this, i)).collect();

    // Sort using the comparison function or default string comparison
    if let Some(cmp) = cmp_fn {
        elements.sort_by(|a, b| {
            // Call the comparison function
            use crate::interpreter::Interpreter;
            use crate::context::JSContext;
            use crate::runtime::JSRuntime;
            // For native sort with a callback, we need to call the JS function
            // Since we can't easily call JS from here, use a simple approach:
            // Convert to strings and compare lexicographically as fallback
            let a_str = a.to_string();
            let b_str = b.to_string();
            a_str.cmp(&b_str)
        });
    } else {
        // Default: convert to strings and sort lexicographically
        elements.sort_by(|a, b| {
            let a_str = a.to_string();
            let b_str = b.to_string();
            a_str.cmp(&b_str)
        });
    }

    // Write sorted elements back
    for (i, elem) in elements.into_iter().enumerate() {
        array_set(this, i, elem);
    }

    this.clone()
}

// ============================================================================
// Additional Array.prototype methods
// ============================================================================

/// Array.prototype.at(index) - Get element at index (supports negative)
pub fn array_at(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let idx = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
    let actual_idx = if idx < 0 {
        (len as i32 + idx).max(0) as usize
    } else {
        idx as usize
    };
    if actual_idx < len {
        array_get(this, actual_idx)
    } else {
        JSValue::undefined()
    }
}

/// Array.prototype.findLast(predicate) - Find last element matching predicate
pub fn array_find_last(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let predicate = match args.first() {
        Some(f) if f.is_callable() => f,
        _ => return JSValue::undefined(),
    };
    for i in (0..len).rev() {
        let val = array_get(this, i);
        let result = call_callback(predicate, this, &[val.clone(), JSValue::int(i as i32), this.clone()]);
        if result.to_boolean() {
            return val;
        }
    }
    JSValue::undefined()
}

/// Array.prototype.findLastIndex(predicate) - Find last index matching predicate
pub fn array_find_last_index(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let predicate = match args.first() {
        Some(f) if f.is_callable() => f,
        _ => return JSValue::Int(-1),
    };
    for i in (0..len).rev() {
        let val = array_get(this, i);
        let result = call_callback(predicate, this, &[val, JSValue::int(i as i32), this.clone()]);
        if result.to_boolean() {
            return JSValue::Int(i as i32);
        }
    }
    JSValue::Int(-1)
}

/// Array.prototype.flatMap(callback) - Map and flatten one level
pub fn array_flat_map(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let callback = match args.first() {
        Some(f) if f.is_callable() => f,
        _ => return this.clone(),
    };
    let mut result = Vec::new();
    for i in 0..len {
        let val = array_get(this, i);
        let mapped = call_callback(callback, this, &[val, JSValue::int(i as i32), this.clone()]);
        if let Some(length_val) = mapped.get_property("length") {
            let mapped_len = length_val.to_uint32();
            for j in 0..mapped_len {
                if let Some(elem) = mapped.get_property(&j.to_string()) {
                    result.push(elem);
                }
            }
        } else {
            result.push(mapped);
        }
    }
    create_array(result)
}

/// Array.prototype.toReversed() - Return reversed copy
pub fn array_to_reversed(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let mut result = Vec::with_capacity(len);
    for i in (0..len).rev() {
        result.push(array_get(this, i));
    }
    create_array(result)
}

/// Array.prototype.toSorted(compareFn) - Return sorted copy
pub fn array_to_sorted(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let mut elements: Vec<JSValue> = (0..len).map(|i| array_get(this, i)).collect();
    if let Some(cmp) = args.first().filter(|v| v.is_callable()) {
        elements.sort_by(|a, b| {
            let result = call_callback(cmp, this, &[a.clone(), b.clone()]);
            let val = result.to_number() as i64;
            if val < 0 { std::cmp::Ordering::Less }
            else if val > 0 { std::cmp::Ordering::Greater }
            else { std::cmp::Ordering::Equal }
        });
    } else {
        elements.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    }
    create_array(elements)
}

/// Array.prototype.toSpliced(start, deleteCount, ...items) - Return spliced copy
pub fn array_to_spliced(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let start = normalize_slice_index(args.get(0).map(|v| v.to_int32()).unwrap_or(0), len as isize);
    let delete_count = args.get(1).map(|v| v.to_int32().max(0) as usize).unwrap_or(len - start);
    let insert_items: Vec<JSValue> = args.iter().skip(2).cloned().collect();

    let mut result = Vec::new();
    // Copy before start
    for i in 0..start {
        result.push(array_get(this, i));
    }
    // Insert new items
    result.extend(insert_items);
    // Copy after deleted region
    let end = (start + delete_count).min(len);
    for i in end..len {
        result.push(array_get(this, i));
    }
    create_array(result)
}

/// Array.prototype.with(index, value) - Return copy with element replaced
pub fn array_with(this: &JSValue, args: &[JSValue]) -> JSValue {
    let len = array_length(this);
    let idx = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
    let val = args.get(1).cloned().unwrap_or(JSValue::undefined());
    let actual_idx = if idx < 0 { (len as i32 + idx).max(0) as usize } else { idx as usize };

    let mut result = Vec::with_capacity(len);
    for i in 0..len {
        if i == actual_idx {
            result.push(val.clone());
        } else {
            result.push(array_get(this, i));
        }
    }
    create_array(result)
}

/// Helper to call a callback function
fn call_callback(callback: &JSValue, this_arg: &JSValue, args: &[JSValue]) -> JSValue {
    if let JSValue::Function(f) = callback {
        let func = f.borrow();
        match &func.body {
            FunctionBody::Native(native_fn) => native_fn(this_arg, args),
            FunctionBody::Closure(closure_fn) => closure_fn(this_arg, args),
            _ => JSValue::undefined(),
        }
    } else {
        JSValue::undefined()
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Normalize a slice index (handle negative values)
fn normalize_slice_index(idx: i32, len: isize) -> usize {
    if idx < 0 {
        (len + idx as isize).max(0) as usize
    } else {
        (idx as usize).min(len as usize)
    }
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Array constructor and prototype.
pub fn init_array(ctx: &mut JSContext) {
    // Create the Array constructor function
    let constructor = JSValue::function(
        Some("Array"),
        vec!["...args".to_string()],
        FunctionBody::Native(array_constructor),
    );

    // Create Array.prototype
    let prototype = JSValue::object("Array");

    // Non-callback methods: implemented as native Rust functions
    let native_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("push", array_push),
        ("pop", array_pop),
        ("shift", array_shift),
        ("unshift", array_unshift),
        ("includes", array_includes),
        ("indexOf", array_index_of),
        ("lastIndexOf", array_last_index_of),
        ("reverse", array_reverse),
        ("slice", array_slice),
        ("splice", array_splice),
        ("concat", array_concat),
        ("join", array_join),
        ("flat", array_flat),
        ("fill", array_fill),
        ("copyWithin", array_copy_within),
        ("toString", array_to_string),
        ("toLocaleString", array_to_string),
        ("sort", array_sort),
        ("at", array_at),
        ("findLast", array_find_last),
        ("findLastIndex", array_find_last_index),
        ("flatMap", array_flat_map),
        ("toReversed", array_to_reversed),
        ("toSorted", array_to_sorted),
        ("toSpliced", array_to_spliced),
        ("with", array_with),
    ];

    for &(name, func) in native_methods {
        prototype.set_property(name, JSValue::function(
            Some(name),
            vec![],
            FunctionBody::Native(func),
        ));
    }

    // Callback-based methods: implemented in JavaScript bytecode
    // This eliminates the need for unsafe native-to-bytecode callback bridges
    let js_methods = super::js_builtins::compile_array_js_methods();
    for (name, func) in js_methods {
        prototype.set_property(&name, func);
    }

    // Add Symbol.iterator method - creates an iterator for the array
    let iterator_func = JSValue::function(
        Some("[Symbol.iterator]"),
        vec![],
        FunctionBody::Native(array_iterator),
    );
    // Store as a regular property named "Symbol.iterator" for now
    // The for-of loop will look for this
    prototype.set_property("@@iterator", iterator_func);

    // Set Array.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Add static methods
    constructor.set_property("isArray", JSValue::function(
        Some("isArray"),
        vec!["value".to_string()],
        FunctionBody::Native(array_is_array),
    ));
    constructor.set_property("from", JSValue::function(
        Some("from"),
        vec!["arrayLike".to_string(), "mapFn".to_string()],
        FunctionBody::Native(array_from),
    ));
    constructor.set_property("of", JSValue::function(
        Some("of"),
        vec![],
        FunctionBody::Native(array_of),
    ));

    // Set Array on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("Array".to_string(), constructor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_array_push_pop() {
        let arr = create_array(vec![]);
        let result = array_push(&arr, &[JSValue::int(1), JSValue::int(2)]);
        assert_eq!(result.to_int32(), 2);
        assert_eq!(array_length(&arr), 2);
        assert_eq!(array_get(&arr, 0).to_int32(), 1);

        let popped = array_pop(&arr, &[]);
        assert_eq!(popped.to_int32(), 2);
        assert_eq!(array_length(&arr), 1);
    }

    #[test]
    fn test_array_shift_unshift() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);

        let shifted = array_shift(&arr, &[]);
        assert_eq!(shifted.to_int32(), 1);
        assert_eq!(array_length(&arr), 2);
        assert_eq!(array_get(&arr, 0).to_int32(), 2);

        let result = array_unshift(&arr, &[JSValue::int(0)]);
        assert_eq!(result.to_int32(), 3);
        assert_eq!(array_get(&arr, 0).to_int32(), 0);
    }

    #[test]
    fn test_array_includes() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
        assert!(array_includes(&arr, &[JSValue::int(2)]).to_boolean());
        assert!(!array_includes(&arr, &[JSValue::int(4)]).to_boolean());
    }

    #[test]
    fn test_array_reverse() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
        array_reverse(&arr, &[]);
        assert_eq!(array_get(&arr, 0).to_int32(), 3);
        assert_eq!(array_get(&arr, 2).to_int32(), 1);
    }

    #[test]
    fn test_array_slice() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3), JSValue::int(4)]);
        let sliced = array_slice(&arr, &[JSValue::int(1), JSValue::int(3)]);
        assert_eq!(array_length(&sliced), 2);
        assert_eq!(array_get(&sliced, 0).to_int32(), 2);
        assert_eq!(array_get(&sliced, 1).to_int32(), 3);
    }

    #[test]
    fn test_array_splice() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3), JSValue::int(4)]);
        let removed = array_splice(&arr, &[JSValue::int(1), JSValue::int(2)]);
        assert_eq!(array_length(&removed), 2);
        assert_eq!(array_get(&removed, 0).to_int32(), 2);
        assert_eq!(array_length(&arr), 2);
        assert_eq!(array_get(&arr, 0).to_int32(), 1);
        assert_eq!(array_get(&arr, 1).to_int32(), 4);
    }

    #[test]
    fn test_array_concat() {
        let arr1 = create_array(vec![JSValue::int(1), JSValue::int(2)]);
        let arr2 = create_array(vec![JSValue::int(3), JSValue::int(4)]);
        let result = array_concat(&arr1, &[arr2]);
        assert_eq!(array_length(&result), 4);
        assert_eq!(array_get(&result, 2).to_int32(), 3);
    }

    #[test]
    fn test_array_join() {
        let arr = create_array(vec![JSValue::string("a"), JSValue::string("b"), JSValue::string("c")]);
        let result = array_join(&arr, &[JSValue::string(", ")]);
        assert_eq!(result.to_string(), "a, b, c");
    }

    #[test]
    fn test_array_is_array() {
        let this = JSValue::undefined();
        let arr = create_array(vec![]);
        assert!(array_is_array(&this, &[arr]).to_boolean());
        assert!(!array_is_array(&this, &[JSValue::int(1)]).to_boolean());
    }

    #[test]
    fn test_array_flat() {
        let inner = create_array(vec![JSValue::int(3), JSValue::int(4)]);
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), inner]);
        let result = array_flat(&arr, &[JSValue::int(1)]);
        assert_eq!(array_length(&result), 4);
        assert_eq!(array_get(&result, 2).to_int32(), 3);
    }

    #[test]
    fn test_array_fill() {
        let arr = create_array(vec![JSValue::int(1), JSValue::int(2), JSValue::int(3)]);
        array_fill(&arr, &[JSValue::int(0), JSValue::int(1), JSValue::int(2)]);
        assert_eq!(array_get(&arr, 0).to_int32(), 1);
        assert_eq!(array_get(&arr, 1).to_int32(), 0);
        assert_eq!(array_get(&arr, 2).to_int32(), 3);
    }

    #[test]
    fn test_init_array() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        init_array(&mut ctx);
        let array = ctx.global.borrow().properties.get("Array").cloned();
        assert!(array.is_some());
    }

    #[test]
    fn test_array_copy_within() {
        let arr = create_array(vec![
            JSValue::int(1), JSValue::int(2), JSValue::int(3), JSValue::int(4), JSValue::int(5),
        ]);
        array_copy_within(&arr, &[JSValue::int(0), JSValue::int(3)]);
        assert_eq!(array_get(&arr, 0).to_int32(), 4);
        assert_eq!(array_get(&arr, 1).to_int32(), 5);
    }

    #[test]
    fn test_array_constructor_length() {
        let this = JSValue::undefined();
        let arr = array_constructor(&this, &[JSValue::int(5)]);
        assert_eq!(array_length(&arr), 5);
        assert!(array_get(&arr, 0).is_undefined());
    }

    #[test]
    fn test_array_of() {
        let result = array_of(&JSValue::undefined(), &[
            JSValue::int(1), JSValue::string("two"), JSValue::int(3),
        ]);
        assert_eq!(array_length(&result), 3);
        assert_eq!(array_get(&result, 0).to_int32(), 1);
        assert_eq!(array_get(&result, 1).to_string(), "two");
    }

    #[test]
    fn test_array_includes_nan() {
        let arr = create_array(vec![
            JSValue::int(1),
            JSValue::Float(f64::NAN),
            JSValue::int(3),
        ]);
        assert!(array_includes(&arr, &[JSValue::Float(f64::NAN)]).to_boolean());
    }
}

/// Array.prototype[@@iterator]() - Returns an iterator for the array.
fn array_iterator(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let arr = this.clone();
    let len = array_length(&arr);

    // Create iterator object with next() method
    let iter = JSValue::object("ArrayIterator");
    iter.set_property("__array", arr);
    iter.set_property("__index", JSValue::Int(0));
    iter.set_property("__length", JSValue::Int(len as i32));

    let iter_clone = iter.clone();
    let next_fn = JSValue::function(
        Some("next"),
        vec![],
        FunctionBody::Closure(Rc::new(move |_this, _args| {
            let arr = iter_clone.get_property("__array").unwrap_or(JSValue::undefined());
            let index = iter_clone.get_property("__index")
                .map(|v| v.to_number() as usize)
                .unwrap_or(0);
            let length = iter_clone.get_property("__length")
                .map(|v| v.to_number() as usize)
                .unwrap_or(0);

            let mut result = JSValue::object("Object");
            if index < length {
                let value = array_get(&arr, index);
                result.set_property("value", value);
                result.set_property("done", JSValue::Bool(false));
                iter_clone.set_property("__index", JSValue::Int((index + 1) as i32));
            } else {
                result.set_property("value", JSValue::undefined());
                result.set_property("done", JSValue::Bool(true));
            }
            result
        })),
    );
    iter.set_property("next", next_fn);
    iter
}

/// Array.prototype.toString()
pub fn array_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    // Equivalent to join(",")
    let len = array_length(this);
    let mut parts = Vec::new();
    for i in 0..len {
        let elem = array_get(this, i);
        parts.push(elem.to_string());
    }
    JSValue::string(&parts.join(","))
}

