#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! WeakSet built-in.
//!
//! Implements the JavaScript WeakSet constructor and its methods.
//! Uses a thread-local registry for WeakSet data storage.
//! Note: True weak references require GC integration; this implementation
//! uses strong references as a practical limitation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// WeakSet data registry
// ============================================================================

thread_local! {
    static WEAKSET_COUNTER: RefCell<usize> = RefCell::new(1);
    static WEAKSET_DATA: RefCell<HashMap<usize, Vec<JSValue>>> = RefCell::new(HashMap::new());
}

fn new_weakset_id() -> usize {
    WEAKSET_COUNTER.with(|c| {
        let mut counter = c.borrow_mut();
        let id = *counter;
        *counter += 1;
        id
    })
}

fn get_weakset_id(this: &JSValue) -> Option<usize> {
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

fn identity_eq(a: &JSValue, b: &JSValue) -> bool {
    match (a, b) {
        (JSValue::Object(a), JSValue::Object(b)) => Rc::ptr_eq(a, b),
        (JSValue::Function(a), JSValue::Function(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

// ============================================================================
// WeakSet constructor
// ============================================================================

/// WeakSet constructor - `new WeakSet(iterable?)`
pub fn weakset_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = new_weakset_id();
    WEAKSET_DATA.with(|data| {
        data.borrow_mut().insert(id, Vec::new());
    });

    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "WeakSet".to_string(),
    };
    obj.internal_slots.insert("id".to_string(), JSValue::Int(id as i32));

    // Populate from iterable if provided
    if let Some(iterable) = args.get(0) {
        if !iterable.is_undefined() && !iterable.is_null() {
            if let Some(length_val) = iterable.get_property("length") {
                let length = length_val.to_uint32();
                let mut values = WEAKSET_DATA.with(|data| data.borrow_mut().remove(&id).unwrap_or_default());
                for i in 0..length {
                    let val = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
                    if !values.iter().any(|v| identity_eq(v, &val)) {
                        values.push(val);
                    }
                }
                WEAKSET_DATA.with(|data| {
                    data.borrow_mut().insert(id, values);
                });
            }
        }
    }

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// WeakSet.prototype methods
// ============================================================================

/// WeakSet.prototype.add(value)
pub fn weakset_add(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakset_id(this) {
        Some(id) => id,
        None => return this.clone(),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKSET_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(values) = data.get_mut(&id) {
            if !values.iter().any(|v| identity_eq(v, &value)) {
                values.push(value);
            }
        }
    });
    this.clone()
}

/// WeakSet.prototype.has(value)
pub fn weakset_has(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakset_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKSET_DATA.with(|data| {
        let data = data.borrow();
        if let Some(values) = data.get(&id) {
            JSValue::bool(values.iter().any(|v| identity_eq(v, &value)))
        } else {
            JSValue::bool(false)
        }
    })
}

/// WeakSet.prototype.delete(value)
pub fn weakset_delete(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakset_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let value = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKSET_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(values) = data.get_mut(&id) {
            if let Some(idx) = values.iter().position(|v| identity_eq(v, &value)) {
                values.remove(idx);
                return JSValue::bool(true);
            }
        }
        JSValue::bool(false)
    })
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the WeakSet constructor and prototype.
pub fn init_weakset(ctx: &mut JSContext) {
    let weakset_ctor = JSValue::function(
        Some("WeakSet"),
        vec!["iterable".to_string()],
        FunctionBody::Native(weakset_constructor),
    );

    let prototype = JSValue::object("WeakSet");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("add", weakset_add),
        ("has", weakset_has),
        ("delete", weakset_delete),
    ];

    for &(name, func) in methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    weakset_ctor.set_property("prototype", prototype);

    ctx.global
        .borrow_mut()
        .properties
        .insert("WeakSet".to_string(), weakset_ctor);
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
    fn test_weakset_constructor() {
        let this = JSValue::undefined();
        let ws = weakset_constructor(&this, &[]);
        match &ws {
            JSValue::Object(obj) => assert_eq!(obj.borrow().class_name, "WeakSet"),
            _ => panic!("Expected WeakSet"),
        }
    }

    #[test]
    fn test_weakset_add_has() {
        let this = JSValue::undefined();
        let ws = weakset_constructor(&this, &[]);
        let obj = JSValue::object("Object");
        weakset_add(&ws, &[obj.clone()]);
        assert!(weakset_has(&ws, &[obj.clone()]).to_boolean());
        let other = JSValue::object("Object");
        assert!(!weakset_has(&ws, &[other]).to_boolean());
    }

    #[test]
    fn test_weakset_delete() {
        let this = JSValue::undefined();
        let ws = weakset_constructor(&this, &[]);
        let obj = JSValue::object("Object");
        weakset_add(&ws, &[obj.clone()]);
        let deleted = weakset_delete(&ws, &[obj.clone()]);
        assert!(deleted.to_boolean());
        assert!(!weakset_has(&ws, &[obj]).to_boolean());
    }

    #[test]
    fn test_weakset_no_duplicates() {
        let this = JSValue::undefined();
        let ws = weakset_constructor(&this, &[]);
        let obj = JSValue::object("Object");
        weakset_add(&ws, &[obj.clone()]);
        weakset_add(&ws, &[obj.clone()]);
        // size is not available on WeakSet, but we can verify has works
        assert!(weakset_has(&ws, &[obj]).to_boolean());
    }

    #[test]
    fn test_init_weakset() {
        let mut ctx = make_ctx();
        init_weakset(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("WeakSet").is_some());
    }
}
