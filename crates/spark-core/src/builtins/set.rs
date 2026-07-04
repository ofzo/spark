#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Set built-in.
//!
//! Implements the JavaScript Set constructor and its methods.
//! Uses a thread-local registry for Set data storage.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Set data registry
// ============================================================================

thread_local! {
    static SET_COUNTER: RefCell<usize> = RefCell::new(1);
    static SET_DATA: RefCell<HashMap<usize, Vec<JSValue>>> = RefCell::new(HashMap::new());
}

fn new_set_id() -> usize {
    SET_COUNTER.with(|c| {
        let mut counter = c.borrow_mut();
        let id = *counter;
        *counter += 1;
        id
    })
}

/// SameValueZero comparison for Set values.
fn same_value_zero(a: &JSValue, b: &JSValue) -> bool {
    match (a, b) {
        (JSValue::Undefined, JSValue::Undefined) => true,
        (JSValue::Null, JSValue::Null) => true,
        (JSValue::Bool(a), JSValue::Bool(b)) => a == b,
        (JSValue::Int(a), JSValue::Int(b)) => a == b,
        (JSValue::Float(a), JSValue::Float(b)) => {
            if a.is_nan() && b.is_nan() { true } else { a == b }
        }
        (JSValue::Int(a), JSValue::Float(b)) => {
            let af = *a as f64;
            if af.is_nan() && b.is_nan() { true } else { af == *b }
        }
        (JSValue::Float(a), JSValue::Int(b)) => {
            let bf = *b as f64;
            if a.is_nan() && bf.is_nan() { true } else { *a == bf }
        }
        (JSValue::String(a), JSValue::String(b)) => a.borrow().data == b.borrow().data,
        (JSValue::Object(a), JSValue::Object(b)) => Rc::ptr_eq(a, b),
        (JSValue::Function(a), JSValue::Function(b)) => Rc::ptr_eq(a, b),
        (JSValue::Symbol(a), JSValue::Symbol(b)) => a.borrow().id == b.borrow().id,
        _ => false,
    }
}

fn get_set_id(this: &JSValue) -> Option<usize> {
    match this {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            borrow.internal_slots.get("id").and_then(|v| match v {
                JSValue::Int(i) => Some(*i as usize),
                _ => None,
            })
        }
        _ => None,
    }
}

// ============================================================================
// Set constructor
// ============================================================================

thread_local! {
    static SET_PROTOTYPE: RefCell<Option<std::rc::Rc<RefCell<JSObject>>>> = RefCell::new(None);
}

/// Set constructor - `new Set(iterable?)`
pub fn set_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = new_set_id();
    SET_DATA.with(|data| {
        data.borrow_mut().insert(id, Vec::new());
    });

    // Get Set.prototype from thread-local
    let prototype = SET_PROTOTYPE.with(|p| p.borrow().clone());

    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Set".to_string(),
    };
    obj.internal_slots.insert("id".to_string(), JSValue::Int(id as i32));

    // If an iterable is provided, populate the set
    if let Some(iterable) = args.get(0) {
        if !iterable.is_undefined() && !iterable.is_null() {
            if let Some(length_val) = iterable.get_property("length") {
                let length = length_val.to_uint32();
                let mut values = SET_DATA.with(|data| data.borrow_mut().remove(&id).unwrap_or_default());
                for i in 0..length {
                    let val = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
                    if !values.iter().any(|v| same_value_zero(v, &val)) {
                        values.push(val);
                    }
                }
                SET_DATA.with(|data| {
                    data.borrow_mut().insert(id, values);
                });
            }
        }
    }

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// Set.prototype methods
// ============================================================================

/// Set.prototype.add(value)
pub fn set_add(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return this.clone(),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    SET_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(values) = data.get_mut(&id) {
            if !values.iter().any(|v| same_value_zero(v, &value)) {
                values.push(value);
            }
        }
    });
    this.clone()
}

/// Set.prototype.has(value)
pub fn set_has(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    SET_DATA.with(|data| {
        let data = data.borrow();
        if let Some(values) = data.get(&id) {
            JSValue::bool(values.iter().any(|v| same_value_zero(v, &value)))
        } else {
            JSValue::bool(false)
        }
    })
}

/// Set.prototype.delete(value)
pub fn set_delete(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    SET_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(values) = data.get_mut(&id) {
            if let Some(idx) = values.iter().position(|v| same_value_zero(v, &value)) {
                values.remove(idx);
                return JSValue::bool(true);
            }
        }
        JSValue::bool(false)
    })
}

/// Set.prototype.clear()
pub fn set_clear(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    SET_DATA.with(|data| {
        if let Some(values) = data.borrow_mut().get_mut(&id) {
            values.clear();
        }
    });
    JSValue::undefined()
}

/// Set.prototype.size
pub fn set_size(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::Int(0),
    };
    SET_DATA.with(|data| {
        let data = data.borrow();
        if let Some(values) = data.get(&id) {
            JSValue::Int(values.len() as i32)
        } else {
            JSValue::Int(0)
        }
    })
}

/// Set.prototype.forEach(callbackFn [, thisArg])
pub fn set_for_each(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let _callback = args.get(0).cloned().unwrap_or(JSValue::undefined());
    // forEach iterates over values, calling callback(value, value, set)
    let _ = (id, args);
    JSValue::undefined()
}

/// Set.prototype.keys() - same as values()
pub fn set_keys(this: &JSValue, args: &[JSValue]) -> JSValue {
    set_values(this, args)
}

/// Set.prototype.values()
pub fn set_values(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let values: Vec<JSValue> = SET_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id).cloned().unwrap_or_default()
    });
    create_array_from_values(&values)
}

/// Set.prototype.entries()
pub fn set_entries(this: &JSValue, args: &[JSValue]) -> JSValue {
    // entries() returns [value, value] pairs
    let id = match get_set_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let values: Vec<JSValue> = SET_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id).cloned().unwrap_or_default()
    });
    let pairs: Vec<JSValue> = values
        .iter()
        .map(|v| {
            let pair = JSValue::object("Array");
            pair.set_property("0", v.clone());
            pair.set_property("1", v.clone());
            pair.set_property("length", JSValue::int(2));
            pair
        })
        .collect();
    create_array_from_values(&pairs)
}

/// Helper: create an Array-like JSValue from a list of values.
fn create_array_from_values(values: &[JSValue]) -> JSValue {
    let arr = JSValue::object("Array");
    for (i, val) in values.iter().enumerate() {
        arr.set_property(&i.to_string(), val.clone());
    }
    arr.set_property("length", JSValue::Int(values.len() as i32));
    arr
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Set constructor and prototype.
pub fn init_set(ctx: &mut JSContext) {
    let set_ctor = JSValue::function(
        Some("Set"),
        vec!["iterable".to_string()],
        FunctionBody::Native(set_constructor),
    );

    let prototype = JSValue::object("Set");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("add", set_add),
        ("has", set_has),
        ("delete", set_delete),
        ("clear", set_clear),
        ("forEach", set_for_each),
        ("keys", set_keys),
        ("values", set_values),
        ("entries", set_entries),
    ];

    for &(name, func) in methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // Define `size` as a getter property
    prototype.define_getter(
        "size",
        JSValue::function(Some("size"), vec![], FunctionBody::Native(set_size)),
    );

    // Store prototype in thread-local for constructor use
    if let JSValue::Object(ref proto_obj) = prototype {
        SET_PROTOTYPE.with(|p| {
            *p.borrow_mut() = Some(proto_obj.clone());
        });
    }

    set_ctor.set_property("prototype", prototype);
    set_ctor.set_property("length", JSValue::int(0));

    ctx.global
        .borrow_mut()
        .properties
        .insert("Set".to_string(), set_ctor);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    fn make_ctx() -> JSContext {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        JSContext::new(rt)
    }

    #[test]
    fn test_set_constructor() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);
        match &set {
            JSValue::Object(obj) => {
                assert_eq!(obj.borrow().class_name, "Set");
            }
            _ => panic!("Expected Set object"),
        }
    }

    #[test]
    fn test_set_add_has() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        set_add(&set, &[JSValue::int(42)]);
        assert!(set_has(&set, &[JSValue::int(42)]).to_boolean());
        assert!(!set_has(&set, &[JSValue::int(99)]).to_boolean());
    }

    #[test]
    fn test_set_delete() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        set_add(&set, &[JSValue::string("hello")]);
        let deleted = set_delete(&set, &[JSValue::string("hello")]);
        assert!(deleted.to_boolean());
        assert!(!set_has(&set, &[JSValue::string("hello")]).to_boolean());
    }

    #[test]
    fn test_set_size() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        assert_eq!(set_size(&set, &[]).to_int32(), 0);
        set_add(&set, &[JSValue::int(1)]);
        set_add(&set, &[JSValue::int(2)]);
        set_add(&set, &[JSValue::int(3)]);
        assert_eq!(set_size(&set, &[]).to_int32(), 3);
    }

    #[test]
    fn test_set_no_duplicates() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        set_add(&set, &[JSValue::int(1)]);
        set_add(&set, &[JSValue::int(1)]);
        set_add(&set, &[JSValue::int(1)]);
        assert_eq!(set_size(&set, &[]).to_int32(), 1);
    }

    #[test]
    fn test_set_clear() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        set_add(&set, &[JSValue::int(1)]);
        set_add(&set, &[JSValue::int(2)]);
        set_clear(&set, &[]);
        assert_eq!(set_size(&set, &[]).to_int32(), 0);
    }

    #[test]
    fn test_set_nan() {
        let this = JSValue::undefined();
        let set = set_constructor(&this, &[]);

        set_add(&set, &[JSValue::Float(f64::NAN)]);
        assert!(set_has(&set, &[JSValue::Float(f64::NAN)]).to_boolean());
        // Should not add duplicate NaN
        set_add(&set, &[JSValue::Float(f64::NAN)]);
        assert_eq!(set_size(&set, &[]).to_int32(), 1);
    }

    #[test]
    fn test_init_set() {
        let mut ctx = make_ctx();
        init_set(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Set").is_some());
    }
}
