#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! WeakMap built-in.
//!
//! Implements the JavaScript WeakMap constructor and its methods.
//! Uses a thread-local registry for WeakMap data storage.
//! Note: True weak references require GC integration; this implementation
//! uses strong references as a practical limitation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// WeakMap data registry
// ============================================================================

thread_local! {
    static WEAKMAP_COUNTER: RefCell<usize> = RefCell::new(1);
    static WEAKMAP_DATA: RefCell<HashMap<usize, Vec<(JSValue, JSValue)>>> = RefCell::new(HashMap::new());
}

fn new_weakmap_id() -> usize {
    WEAKMAP_COUNTER.with(|c| {
        let mut counter = c.borrow_mut();
        let id = *counter;
        *counter += 1;
        id
    })
}

fn get_weakmap_id(this: &JSValue) -> Option<usize> {
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

fn find_entry_index(entries: &[(JSValue, JSValue)], key: &JSValue) -> Option<usize> {
    entries.iter().position(|(k, _)| match (k, key) {
        (JSValue::Object(a), JSValue::Object(b)) => Rc::ptr_eq(a, b),
        (JSValue::Function(a), JSValue::Function(b)) => Rc::ptr_eq(a, b),
        _ => false,
    })
}

// ============================================================================
// WeakMap constructor
// ============================================================================

/// WeakMap constructor - `new WeakMap(iterable?)`
pub fn weakmap_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = new_weakmap_id();
    WEAKMAP_DATA.with(|data| {
        data.borrow_mut().insert(id, Vec::new());
    });

    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "WeakMap".to_string(),
    };
    obj.internal_slots.insert("id".to_string(), JSValue::Int(id as i32));

    // Populate from iterable if provided
    if let Some(iterable) = args.get(0) {
        if !iterable.is_undefined() && !iterable.is_null() {
            if let Some(length_val) = iterable.get_property("length") {
                let length = length_val.to_uint32();
                let mut entries = WEAKMAP_DATA.with(|data| data.borrow_mut().remove(&id).unwrap_or_default());
                for i in 0..length {
                    let item = iterable.get_property(&i.to_string()).unwrap_or(JSValue::undefined());
                    if let JSValue::Object(entry_obj) = &item {
                        let key = entry_obj.borrow().properties.get("0").cloned().unwrap_or(JSValue::undefined());
                        let val = entry_obj.borrow().properties.get("1").cloned().unwrap_or(JSValue::undefined());
                        if let Some(idx) = find_entry_index(&entries, &key) {
                            entries[idx].1 = val;
                        } else {
                            entries.push((key, val));
                        }
                    }
                }
                WEAKMAP_DATA.with(|data| {
                    data.borrow_mut().insert(id, entries);
                });
            }
        }
    }

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// WeakMap.prototype methods
// ============================================================================

/// WeakMap.prototype.get(key)
pub fn weakmap_get(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakmap_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKMAP_DATA.with(|data| {
        let data = data.borrow();
        if let Some(entries) = data.get(&id) {
            if let Some(idx) = find_entry_index(entries, &key) {
                return entries[idx].1.clone();
            }
        }
        JSValue::undefined()
    })
}

/// WeakMap.prototype.set(key, value)
pub fn weakmap_set(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakmap_id(this) {
        Some(id) => id,
        None => return this.clone(),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let value = args.get(1).cloned().unwrap_or(JSValue::undefined());
    WEAKMAP_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(entries) = data.get_mut(&id) {
            if let Some(idx) = find_entry_index(entries, &key) {
                entries[idx].1 = value;
            } else {
                entries.push((key, value));
            }
        }
    });
    this.clone()
}

/// WeakMap.prototype.has(key)
pub fn weakmap_has(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakmap_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKMAP_DATA.with(|data| {
        let data = data.borrow();
        if let Some(entries) = data.get(&id) {
            JSValue::bool(find_entry_index(entries, &key).is_some())
        } else {
            JSValue::bool(false)
        }
    })
}

/// WeakMap.prototype.delete(key)
pub fn weakmap_delete(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_weakmap_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    WEAKMAP_DATA.with(|data| {
        let mut data = data.borrow_mut();
        if let Some(entries) = data.get_mut(&id) {
            if let Some(idx) = find_entry_index(entries, &key) {
                entries.remove(idx);
                return JSValue::bool(true);
            }
        }
        JSValue::bool(false)
    })
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the WeakMap constructor and prototype.
pub fn init_weakmap(ctx: &mut JSContext) {
    let weakmap_ctor = JSValue::function(
        Some("WeakMap"),
        vec!["iterable".to_string()],
        FunctionBody::Native(weakmap_constructor),
    );

    let prototype = JSValue::object("WeakMap");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("get", weakmap_get),
        ("set", weakmap_set),
        ("has", weakmap_has),
        ("delete", weakmap_delete),
    ];

    for &(name, func) in methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    weakmap_ctor.set_property("prototype", prototype);

    ctx.global
        .borrow_mut()
        .properties
        .insert("WeakMap".to_string(), weakmap_ctor);
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
    fn test_weakmap_constructor() {
        let this = JSValue::undefined();
        let wm = weakmap_constructor(&this, &[]);
        match &wm {
            JSValue::Object(obj) => assert_eq!(obj.borrow().class_name, "WeakMap"),
            _ => panic!("Expected WeakMap"),
        }
    }

    #[test]
    fn test_weakmap_set_get() {
        let this = JSValue::undefined();
        let wm = weakmap_constructor(&this, &[]);
        let key = JSValue::object("Object");
        weakmap_set(&wm, &[key.clone(), JSValue::int(42)]);
        let val = weakmap_get(&wm, &[key.clone()]);
        assert_eq!(val.to_int32(), 42);
    }

    #[test]
    fn test_weakmap_has() {
        let this = JSValue::undefined();
        let wm = weakmap_constructor(&this, &[]);
        let key = JSValue::object("Object");
        weakmap_set(&wm, &[key.clone(), JSValue::int(1)]);
        assert!(weakmap_has(&wm, &[key.clone()]).to_boolean());
        let other_key = JSValue::object("Object");
        assert!(!weakmap_has(&wm, &[other_key]).to_boolean());
    }

    #[test]
    fn test_weakmap_delete() {
        let this = JSValue::undefined();
        let wm = weakmap_constructor(&this, &[]);
        let key = JSValue::object("Object");
        weakmap_set(&wm, &[key.clone(), JSValue::int(1)]);
        let deleted = weakmap_delete(&wm, &[key.clone()]);
        assert!(deleted.to_boolean());
        assert!(!weakmap_has(&wm, &[key]).to_boolean());
    }

    #[test]
    fn test_init_weakmap() {
        let mut ctx = make_ctx();
        init_weakmap(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("WeakMap").is_some());
    }
}
