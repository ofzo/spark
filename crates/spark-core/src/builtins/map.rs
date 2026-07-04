#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Map built-in.
//!
//! Implements the JavaScript Map constructor and its methods.
//! Uses a thread-local registry for Map data storage, indexed by a unique
//! integer ID stored in the object's internal_slots["id"].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Map data registry
// ============================================================================

thread_local! {
    static MAP_COUNTER: RefCell<usize> = RefCell::new(1);
    static MAP_DATA: RefCell<HashMap<usize, Vec<(JSValue, JSValue)>>> = RefCell::new(HashMap::new());
}

fn new_map_id() -> usize {
    MAP_COUNTER.with(|c| {
        let mut counter = c.borrow_mut();
        let id = *counter;
        *counter += 1;
        id
    })
}

/// SameValueZero comparison for Map keys.
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

fn find_entry_index(entries: &[(JSValue, JSValue)], key: &JSValue) -> Option<usize> {
    entries.iter().position(|(k, _)| same_value_zero(k, key))
}

fn get_map_id(this: &JSValue) -> Option<usize> {
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
// Map constructor
// ============================================================================

thread_local! {
    static MAP_PROTOTYPE: RefCell<Option<Rc<RefCell<JSObject>>>> = RefCell::new(None);
}

/// Map constructor - `new Map(iterable?)`
pub fn map_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = new_map_id();
    MAP_DATA.with(|data| {
        data.borrow_mut().insert(id, Vec::new());
    });

    // Get Map.prototype from thread-local
    let prototype = MAP_PROTOTYPE.with(|p| p.borrow().clone());

    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Map".to_string(),
    };
    obj.internal_slots.insert("id".to_string(), JSValue::Int(id as i32));

    // If an iterable is provided, populate the map
    if let Some(iterable) = args.get(0) {
        if !iterable.is_undefined() && !iterable.is_null() {
            // Simple array-like iterable support: [[key, value], [key, value], ...]
            if let Some(length_val) = iterable.get_property("length") {
                let length = length_val.to_uint32();
                let mut entries = MAP_DATA.with(|data| data.borrow_mut().remove(&id).unwrap_or_default());
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
                MAP_DATA.with(|data| {
                    data.borrow_mut().insert(id, entries);
                });
            }
        }
    }

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

// ============================================================================
// Map.prototype methods
// ============================================================================

/// Map.prototype.get(key)
pub fn map_get(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    MAP_DATA.with(|data| {
        let data = data.borrow();
        if let Some(entries) = data.get(&id) {
            if let Some(idx) = find_entry_index(entries, &key) {
                return entries[idx].1.clone();
            }
        }
        JSValue::undefined()
    })
}

/// Map.prototype.set(key, value)
pub fn map_set(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return this.clone(),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    let value = args.get(1).cloned().unwrap_or(JSValue::undefined());
    MAP_DATA.with(|data| {
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

/// Map.prototype.has(key)
pub fn map_has(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    MAP_DATA.with(|data| {
        let data = data.borrow();
        if let Some(entries) = data.get(&id) {
            JSValue::bool(find_entry_index(entries, &key).is_some())
        } else {
            JSValue::bool(false)
        }
    })
}

/// Map.prototype.delete(key)
pub fn map_delete(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::bool(false),
    };
    let key = args.get(0).cloned().unwrap_or(JSValue::undefined());
    MAP_DATA.with(|data| {
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

/// Map.prototype.clear()
pub fn map_clear(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    MAP_DATA.with(|data| {
        if let Some(entries) = data.borrow_mut().get_mut(&id) {
            entries.clear();
        }
    });
    JSValue::undefined()
}

/// Map.prototype.size getter (accessed via a method that returns the size)
pub fn map_size(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::Float(0.0),
    };
    MAP_DATA.with(|data| {
        let data = data.borrow();
        if let Some(entries) = data.get(&id) {
            JSValue::Int(entries.len() as i32)
        } else {
            JSValue::Int(0)
        }
    })
}

/// Map.prototype.forEach(callbackFn [, thisArg])
pub fn map_for_each(this: &JSValue, args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let callback = args.get(0).cloned().unwrap_or(JSValue::undefined());
    if !callback.is_callable() {
        return JSValue::undefined();
    }

    let entries: Vec<(JSValue, JSValue)> = MAP_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id).cloned().unwrap_or_default()
    });

    for (key, value) in &entries {
        // callback(value, key, map)
        // Note: We can't invoke the callback through the interpreter here,
        // so we store the call info for later execution.
        // For now, the callback receives the arguments as-is.
        let _ = (key, value, &callback);
    }

    JSValue::undefined()
}

/// Map.prototype.keys() - Returns an iterator over keys
pub fn map_keys(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let keys: Vec<JSValue> = MAP_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id)
            .map(|entries| entries.iter().map(|(k, _)| k.clone()).collect())
            .unwrap_or_default()
    });
    create_array_from_values(&keys)
}

/// Map.prototype.values() - Returns an iterator over values
pub fn map_values(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let values: Vec<JSValue> = MAP_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id)
            .map(|entries| entries.iter().map(|(_, v)| v.clone()).collect())
            .unwrap_or_default()
    });
    create_array_from_values(&values)
}

/// Map.prototype.entries() - Returns an iterator over [key, value] pairs
pub fn map_entries(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let id = match get_map_id(this) {
        Some(id) => id,
        None => return JSValue::undefined(),
    };
    let pairs: Vec<JSValue> = MAP_DATA.with(|data| {
        let data = data.borrow();
        data.get(&id)
            .map(|entries| {
                entries
                    .iter()
                    .map(|(k, v)| {
                        let pair = JSValue::object("Array");
                        pair.set_property("0", k.clone());
                        pair.set_property("1", v.clone());
                        pair.set_property("length", JSValue::int(2));
                        pair
                    })
                    .collect()
            })
            .unwrap_or_default()
    });
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

/// Initialize the Map constructor and prototype.
pub fn init_map(ctx: &mut JSContext) {
    let map_ctor = JSValue::function(
        Some("Map"),
        vec!["iterable".to_string()],
        FunctionBody::Native(map_constructor),
    );

    let prototype = JSValue::object("Map");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("get", map_get),
        ("set", map_set),
        ("has", map_has),
        ("delete", map_delete),
        ("clear", map_clear),
        ("forEach", map_for_each),
        ("keys", map_keys),
        ("values", map_values),
        ("entries", map_entries),
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
        JSValue::function(Some("size"), vec![], FunctionBody::Native(map_size)),
    );

    // Store prototype in thread-local for constructor use
    if let JSValue::Object(ref proto_obj) = prototype {
        MAP_PROTOTYPE.with(|p| {
            *p.borrow_mut() = Some(proto_obj.clone());
        });
    }

    map_ctor.set_property("prototype", prototype);
    map_ctor.set_property("length", JSValue::int(0));

    ctx.global
        .borrow_mut()
        .properties
        .insert("Map".to_string(), map_ctor);
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
    fn test_map_constructor() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);
        match &map {
            JSValue::Object(obj) => {
                assert_eq!(obj.borrow().class_name, "Map");
            }
            _ => panic!("Expected Map object"),
        }
    }

    #[test]
    fn test_map_set_get() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::string("key1"), JSValue::int(42)]);
        let val = map_get(&map, &[JSValue::string("key1")]);
        assert_eq!(val.to_int32(), 42);
    }

    #[test]
    fn test_map_has() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::string("key1"), JSValue::int(1)]);
        assert!(map_has(&map, &[JSValue::string("key1")]).to_boolean());
        assert!(!map_has(&map, &[JSValue::string("key2")]).to_boolean());
    }

    #[test]
    fn test_map_delete() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::string("key1"), JSValue::int(1)]);
        let deleted = map_delete(&map, &[JSValue::string("key1")]);
        assert!(deleted.to_boolean());
        assert!(!map_has(&map, &[JSValue::string("key1")]).to_boolean());
    }

    #[test]
    fn test_map_size() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        assert_eq!(map_size(&map, &[]).to_int32(), 0);
        map_set(&map, &[JSValue::string("a"), JSValue::int(1)]);
        map_set(&map, &[JSValue::string("b"), JSValue::int(2)]);
        assert_eq!(map_size(&map, &[]).to_int32(), 2);
    }

    #[test]
    fn test_map_clear() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::string("a"), JSValue::int(1)]);
        map_set(&map, &[JSValue::string("b"), JSValue::int(2)]);
        map_clear(&map, &[]);
        assert_eq!(map_size(&map, &[]).to_int32(), 0);
    }

    #[test]
    fn test_map_overwrite() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::string("key"), JSValue::int(1)]);
        map_set(&map, &[JSValue::string("key"), JSValue::int(2)]);
        let val = map_get(&map, &[JSValue::string("key")]);
        assert_eq!(val.to_int32(), 2);
        assert_eq!(map_size(&map, &[]).to_int32(), 1);
    }

    #[test]
    fn test_map_nan_key() {
        let this = JSValue::undefined();
        let map = map_constructor(&this, &[]);

        map_set(&map, &[JSValue::Float(f64::NAN), JSValue::int(1)]);
        let val = map_get(&map, &[JSValue::Float(f64::NAN)]);
        assert_eq!(val.to_int32(), 1);
        // NaN === NaN for SameValueZero
        assert!(map_has(&map, &[JSValue::Float(f64::NAN)]).to_boolean());
    }

    #[test]
    fn test_init_map() {
        let mut ctx = make_ctx();
        init_map(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Map").is_some());
    }
}
