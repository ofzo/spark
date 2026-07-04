#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Object built-in.
//!
//! Implements the JavaScript Object constructor and its methods.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Property attribute helpers
//
// Property attributes (writable, enumerable, configurable) are stored in
// the object's `internal_slots` map using a key convention:
//   __w_{prop}  -> writable
//   __e_{prop}  -> enumerable
//   __c_{prop}  -> configurable
//
// When a key is absent the default is `true` (matching JavaScript's default
// for properties created via assignment).
// ============================================================================

/// Build an internal_slots key for a property attribute.
fn attr_key(prop: &str, suffix: &str) -> String {
    format!("__{}_{}", prop, suffix)
}

/// Check whether a property is writable (default: true).
fn is_property_writable(obj: &JSObject, prop: &str) -> bool {
    obj.internal_slots
        .get(&attr_key(prop, "w"))
        .map(|v| v.to_boolean())
        .unwrap_or(true)
}

/// Check whether a property is enumerable (default: true).
fn is_property_enumerable(obj: &JSObject, prop: &str) -> bool {
    obj.internal_slots
        .get(&attr_key(prop, "e"))
        .map(|v| v.to_boolean())
        .unwrap_or(true)
}

/// Check whether a property is configurable (default: true).
fn is_property_configurable(obj: &JSObject, prop: &str) -> bool {
    obj.internal_slots
        .get(&attr_key(prop, "c"))
        .map(|v| v.to_boolean())
        .unwrap_or(true)
}

/// Store property attributes in internal_slots.
fn set_property_attributes(
    obj: &mut JSObject,
    prop: &str,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    obj.internal_slots
        .insert(attr_key(prop, "w"), JSValue::bool(writable));
    obj.internal_slots
        .insert(attr_key(prop, "e"), JSValue::bool(enumerable));
    obj.internal_slots
        .insert(attr_key(prop, "c"), JSValue::bool(configurable));
}

/// Check whether an object is frozen.
fn is_object_frozen(obj: &JSObject) -> bool {
    obj.internal_slots
        .get("frozen")
        .map(|v| v.to_boolean())
        .unwrap_or(false)
}

/// Check whether an object is sealed.
fn is_object_sealed(obj: &JSObject) -> bool {
    obj.internal_slots
        .get("sealed")
        .map(|v| v.to_boolean())
        .unwrap_or(false)
}

/// Check whether an object is extensible (default: true).
fn is_object_extensible(obj: &JSObject) -> bool {
    obj.internal_slots
        .get("extensible")
        .map(|v| v.to_boolean())
        .unwrap_or(true)
}

// ============================================================================
// Object constructor
// ============================================================================

/// Object constructor.
pub fn object_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.first() {
        None | Some(JSValue::Undefined) | Some(JSValue::Null) => JSValue::object("Object"),
        Some(val) if val.is_object() => val.clone(),
        Some(val @ (JSValue::Bool(_) | JSValue::Int(_) | JSValue::Float(_) | JSValue::String(_))) => {
            // Auto-box primitive values
            val.to_object().unwrap_or_else(|_| JSValue::object("Object"))
        }
        Some(val) => val.clone(),
    }
}

// ============================================================================
// Object static methods
// ============================================================================

/// Object.keys(obj) - Returns an array of object's own enumerable property names.
pub fn object_keys(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return create_string_array(vec![]),
    };

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            let mut keys: Vec<String> = borrow
                .properties
                .keys()
                .filter(|k| is_property_enumerable(&borrow, k))
                .cloned()
                .collect();
            keys.sort();
            create_string_array(keys)
        }
        JSValue::Function(f) => {
            let borrow = f.borrow();
            let mut keys: Vec<String> = borrow.closure.keys().cloned().collect();
            keys.sort();
            create_string_array(keys)
        }
        _ => create_string_array(vec![]),
    }
}

/// Object.values(obj) - Returns an array of object's own enumerable property values.
pub fn object_values(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return create_value_array(vec![]),
    };

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            let mut entries: Vec<(String, JSValue)> = borrow
                .properties
                .iter()
                .filter(|(k, _)| is_property_enumerable(&borrow, k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let values: Vec<JSValue> = entries.into_iter().map(|(_, v)| v).collect();
            create_value_array(values)
        }
        _ => create_value_array(vec![]),
    }
}

/// Object.entries(obj) - Returns an array of [key, value] pairs.
pub fn object_entries(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return create_value_array(vec![]),
    };

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            let mut entries: Vec<(String, JSValue)> = borrow
                .properties
                .iter()
                .filter(|(k, _)| is_property_enumerable(&borrow, k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            let result = JSValue::object("Array");
            let len = entries.len();
            for (i, (key, value)) in entries.into_iter().enumerate() {
                let pair = JSValue::object("Array");
                pair.set_property("0", JSValue::string(&key));
                pair.set_property("1", value);
                pair.set_property("length", JSValue::int(2));
                result.set_property(&i.to_string(), pair);
            }
            result.set_property("length", JSValue::int(len as i32));
            result
        }
        _ => {
            let result = JSValue::object("Array");
            result.set_property("length", JSValue::int(0));
            result
        }
    }
}

/// Object.assign(target, ...sources)
///
/// Copies all enumerable own properties from one or more source objects
/// to the target object. Returns the target.
pub fn object_assign(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let target = match args.first() {
        Some(v) => v.clone(),
        None => return JSValue::undefined(),
    };

    for source in args.iter().skip(1) {
        if source.is_null() || source.is_undefined() {
            continue;
        }
        match source {
            JSValue::Object(o) => {
                let borrow = o.borrow();
                let keys: Vec<String> = borrow.properties.keys().cloned().collect();
                for key in keys {
                    let val = borrow
                        .properties
                        .get(&key)
                        .cloned()
                        .unwrap_or(JSValue::undefined());
                    target.set_property(&key, val);
                }
            }
            JSValue::Function(f) => {
                let borrow = f.borrow();
                for (key, val) in &borrow.closure {
                    target.set_property(key, val.borrow().clone());
                }
            }
            _ => {}
        }
    }

    target
}

/// Object.create(proto, propertiesObject)
///
/// Creates a new object with the specified prototype and optional properties.
/// If `propertiesObject` is provided, its own enumerable properties are
/// defined as property descriptors on the new object.
pub fn object_create(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let proto = args.first().cloned();

    let mut obj = JSObject {
        properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: match proto {
            Some(JSValue::Object(o)) => Some(o),
            _ => None,
        },
        internal_slots: HashMap::new(),
        class_name: "Object".to_string(),
    };

    // If propertiesObject is provided, define properties on the new object
    if let Some(JSValue::Object(props_obj)) = args.get(1) {
        let borrow = props_obj.borrow();
        let keys: Vec<String> = borrow.properties.keys().cloned().collect();
        for key in keys {
            if let Some(descriptor) = borrow.properties.get(&key) {
                apply_property_descriptor_value(&mut obj, &key, descriptor);
            }
        }
    }

    JSValue::Object(Rc::new(RefCell::new(obj)))
}

/// Object.defineProperty(obj, prop, descriptor)
///
/// Defines a new property directly on an object, or modifies an existing
/// property on an object, and returns the object.
pub fn object_define_property(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    let prop = args.get(1).map(|v| v.to_string()).unwrap_or_default();

    match obj {
        JSValue::Object(o) => {
            let mut borrow = o.borrow_mut();

            // Check if the object is frozen -- cannot define properties on frozen objects
            if is_object_frozen(&borrow) {
                return obj.clone();
            }

            // Check if the object is sealed and the property doesn't exist
            // (sealed objects cannot have new properties added)
            let exists = borrow.properties.contains_key(&prop);
            if is_object_sealed(&borrow) && !exists {
                return obj.clone();
            }

            // Check if the object is non-extensible and the property doesn't exist
            if !is_object_extensible(&borrow) && !exists {
                return obj.clone();
            }

            // If property exists and is non-configurable, enforce ECMAScript
            // restrictions on what attribute changes are permitted.
            if exists {
                let configurable = is_property_configurable(&borrow, &prop);
                if !configurable {
                    if let Some(JSValue::Object(desc)) = args.get(2) {
                        let desc_borrow = desc.borrow();

                        // Reject changes to enumerable/configurable
                        let has_enumerable_change = desc_borrow.properties.contains_key("enumerable");
                        let has_configurable_change = desc_borrow.properties.contains_key("configurable");
                        if has_enumerable_change || has_configurable_change {
                            return obj.clone();
                        }

                        // Reject transitioning to accessors
                        let has_get = desc_borrow.properties.contains_key("get");
                        let has_set = desc_borrow.properties.contains_key("set");
                        if has_get || has_set {
                            return obj.clone();
                        }

                        let current_writable = is_property_writable(&borrow, &prop);

                        // Reject changing writable from false to true on a
                        // non-configurable property
                        if !current_writable {
                            let desc_wants_writable = desc_borrow
                                .properties
                                .get("writable")
                                .map(|v| v.to_boolean())
                                .unwrap_or(false);
                            if desc_wants_writable {
                                return obj.clone();
                            }
                            // Also reject value changes on non-writable properties
                            if desc_borrow.properties.contains_key("value") {
                                return obj.clone();
                            }
                        }
                    }
                }
            }

            match args.get(2) {
                Some(JSValue::Object(desc)) => {
                    let desc_borrow = desc.borrow();
                    apply_descriptor_to_existing(&mut borrow, &prop, &desc_borrow);
                }
                _ => {}
            }

            drop(borrow);
            obj.clone()
        }
        _ => obj.clone(),
    }
}

/// Object.defineProperties(obj, props)
///
/// Defines new or modifies existing properties directly on an object,
/// accepting multiple property descriptors at once.
pub fn object_define_properties(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };

    if let Some(JSValue::Object(props_obj)) = args.get(1) {
        match obj {
            JSValue::Object(o) => {
                let mut borrow = o.borrow_mut();

                if is_object_frozen(&borrow) {
                    return obj.clone();
                }

                let keys: Vec<String> = props_obj.borrow().properties.keys().cloned().collect();
                for key in keys {
                    let descriptor = props_obj.borrow().properties.get(&key).cloned();
                    if let Some(descriptor) = descriptor {
                        let exists = borrow.properties.contains_key(&key);

                        if is_object_sealed(&borrow) && !exists {
                            continue;
                        }
                        if !is_object_extensible(&borrow) && !exists {
                            continue;
                        }

                        if exists {
                            let configurable = is_property_configurable(&borrow, &key);
                            if !configurable {
                                if let JSValue::Object(desc) = &descriptor {
                                    let desc_borrow = desc.borrow();
                                    let has_e = desc_borrow.properties.contains_key("enumerable");
                                    let has_c = desc_borrow.properties.contains_key("configurable");
                                    if has_e || has_c {
                                        continue;
                                    }
                                    let has_get = desc_borrow.properties.contains_key("get");
                                    let has_set = desc_borrow.properties.contains_key("set");
                                    if has_get || has_set {
                                        continue;
                                    }
                                    let current_writable = is_property_writable(&borrow, &key);
                                    if !current_writable {
                                        let desc_wants_writable = desc_borrow
                                            .properties
                                            .get("writable")
                                            .map(|v| v.to_boolean())
                                            .unwrap_or(false);
                                        if desc_wants_writable {
                                            continue;
                                        }
                                        if desc_borrow.properties.contains_key("value") {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }

                        apply_property_descriptor_value(&mut borrow, &key, &descriptor);
                    }
                }
            }
            _ => {}
        }
    }

    obj.clone()
}

/// Object.getOwnPropertyDescriptor(obj, prop)
///
/// Returns a property descriptor for the own property of an object.
pub fn object_get_own_property_descriptor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    let prop = args.get(1).map(|v| v.to_string()).unwrap_or_default();

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            if let Some(val) = borrow.properties.get(&prop) {
                let desc = JSValue::object("Object");
                let writable = is_property_writable(&borrow, &prop);
                let enumerable = is_property_enumerable(&borrow, &prop);
                let configurable = is_property_configurable(&borrow, &prop);
                desc.set_property("value", val.clone());
                desc.set_property("writable", JSValue::bool(writable));
                desc.set_property("enumerable", JSValue::bool(enumerable));
                desc.set_property("configurable", JSValue::bool(configurable));
                return desc;
            }
        }
        _ => {}
    }

    JSValue::undefined()
}

/// Object.getOwnPropertyDescriptors(obj)
///
/// Returns an object containing all own property descriptors of an object.
pub fn object_get_own_property_descriptors(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::object("Object"),
    };

    let result = JSValue::object("Object");

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            for (key, val) in &borrow.properties {
                let desc = JSValue::object("Object");
                let writable = is_property_writable(&borrow, key);
                let enumerable = is_property_enumerable(&borrow, key);
                let configurable = is_property_configurable(&borrow, key);
                desc.set_property("value", val.clone());
                desc.set_property("writable", JSValue::bool(writable));
                desc.set_property("enumerable", JSValue::bool(enumerable));
                desc.set_property("configurable", JSValue::bool(configurable));
                result.set_property(key, desc);
            }
        }
        _ => {}
    }

    result
}

/// Object.getOwnPropertyNames(obj)
///
/// Returns an array of all own property names (including non-enumerable
/// ones, but excluding Symbol properties).
pub fn object_get_own_property_names(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return create_string_array(vec![]),
    };

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            let mut names: Vec<String> = borrow.properties.keys().cloned().collect();
            names.sort();
            create_string_array(names)
        }
        JSValue::Function(f) => {
            let borrow = f.borrow();
            let mut names: Vec<String> = borrow.closure.keys().cloned().collect();
            names.sort();
            create_string_array(names)
        }
        _ => create_string_array(vec![]),
    }
}

/// Object.getOwnPropertySymbols(obj)
///
/// Returns an array of all own Symbol properties of an object.
/// Note: full Symbol support is not yet implemented in this engine;
/// this method always returns an empty array.
pub fn object_get_own_property_symbols(_this: &JSValue, _args: &[JSValue]) -> JSValue {
    create_value_array(vec![])
}

/// Object.getPrototypeOf(obj)
///
/// Returns the prototype (internal [[Prototype]] property) of the specified
/// object, or null if the object has no prototype.
pub fn object_get_prototype_of(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::null(),
    };

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            match &borrow.prototype {
                Some(proto) => JSValue::Object(proto.clone()),
                None => JSValue::null(),
            }
        }
        _ => JSValue::null(),
    }
}

/// Object.setPrototypeOf(obj, proto)
///
/// Sets the prototype (internal [[Prototype]] property) of the specified
/// object. Returns the object.
pub fn object_set_prototype_of(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    let proto = args.get(1).cloned();

    match obj {
        JSValue::Object(o) => {
            let mut borrow = o.borrow_mut();
            borrow.prototype = match proto {
                Some(JSValue::Object(p)) => Some(p),
                Some(JSValue::Null) => None,
                _ => None,
            };
        }
        _ => {}
    }

    obj.clone()
}

/// Object.is(value1, value2) - SameValue comparison
///
/// Determines whether two values are the same value. Unlike `===`, this
/// treats NaN as equal to NaN and distinguishes +0 from -0.
pub fn object_is(_this: &JSValue, args: &[JSValue]) -> JSValue {
    if args.len() < 2 {
        return JSValue::bool(false);
    }
    let a = &args[0];
    let b = &args[1];

    match (a, b) {
        (JSValue::Float(x), JSValue::Float(y)) => {
            // NaN === NaN in Object.is
            if x.is_nan() && y.is_nan() {
                return JSValue::bool(true);
            }
            // Distinguish +0 from -0
            if *x == 0.0 && *y == 0.0 {
                return JSValue::bool(x.is_sign_positive() == y.is_sign_positive());
            }
            JSValue::bool(x == y)
        }
        (JSValue::Int(x), JSValue::Int(y)) => JSValue::bool(x == y),
        (JSValue::Int(x), JSValue::Float(y)) => JSValue::bool(*x as f64 == *y),
        (JSValue::Float(x), JSValue::Int(y)) => JSValue::bool(*x == *y as f64),
        _ => JSValue::bool(a.strict_eq(b)),
    }
}

/// Object.freeze(obj)
///
/// Freezes an object: prevents new properties from being added, existing
/// properties from being removed, and existing property descriptors from
/// being changed. Also makes all existing properties non-writable.
/// Returns the frozen object.
pub fn object_freeze(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    if let JSValue::Object(o) = obj {
        let mut borrow = o.borrow_mut();
        // Mark as frozen
        borrow
            .internal_slots
            .insert("frozen".to_string(), JSValue::bool(true));
        // Freeze also seals
        borrow
            .internal_slots
            .insert("sealed".to_string(), JSValue::bool(true));
        // Freeze makes all existing properties non-writable and non-configurable
        // Collect attribute snapshots first to satisfy the borrow checker
        let keys_and_attrs: Vec<(String, bool)> = {
            let keys: Vec<String> = borrow.properties.keys().cloned().collect();
            keys.into_iter()
                .map(|k| {
                    let e = is_property_enumerable(&borrow, &k);
                    (k, e)
                })
                .collect()
        };
        for (key, enumerable) in keys_and_attrs {
            set_property_attributes(&mut borrow, &key, false, enumerable, false);
        }
    }
    obj.clone()
}

/// Object.seal(obj)
///
/// Seals an object: prevents new properties from being added and existing
/// properties from being removed. Existing properties are made
/// non-configurable. Returns the sealed object.
pub fn object_seal(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    if let JSValue::Object(o) = obj {
        let mut borrow = o.borrow_mut();
        // Mark as sealed
        borrow
            .internal_slots
            .insert("sealed".to_string(), JSValue::bool(true));
        // Seal makes all existing properties non-configurable
        // Collect attribute snapshots first to satisfy the borrow checker
        let keys_and_attrs: Vec<(String, bool, bool)> = {
            let keys: Vec<String> = borrow.properties.keys().cloned().collect();
            keys.into_iter()
                .map(|k| {
                    let w = is_property_writable(&borrow, &k);
                    let e = is_property_enumerable(&borrow, &k);
                    (k, w, e)
                })
                .collect()
        };
        for (key, writable, enumerable) in keys_and_attrs {
            set_property_attributes(
                &mut borrow,
                &key,
                writable,
                enumerable,
                false,
            );
        }
    }
    obj.clone()
}

/// Object.preventExtensions(obj)
///
/// Prevents new properties from being added to an object. Returns the
/// object.
pub fn object_prevent_extensions(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::undefined(),
    };
    if let JSValue::Object(o) = obj {
        o.borrow_mut()
            .internal_slots
            .insert("extensible".to_string(), JSValue::bool(false));
    }
    obj.clone()
}

/// Object.isFrozen(obj)
///
/// Returns true if the object is frozen.
pub fn object_is_frozen(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.first() {
        Some(JSValue::Object(o)) => {
            let borrow = o.borrow();
            JSValue::bool(is_object_frozen(&borrow))
        }
        _ => JSValue::bool(true),
    }
}

/// Object.isSealed(obj)
///
/// Returns true if the object is sealed.
pub fn object_is_sealed(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.first() {
        Some(JSValue::Object(o)) => {
            let borrow = o.borrow();
            JSValue::bool(is_object_sealed(&borrow))
        }
        _ => JSValue::bool(true),
    }
}

/// Object.isExtensible(obj)
///
/// Returns true if the object is extensible (can have new properties added).
pub fn object_is_extensible(_this: &JSValue, args: &[JSValue]) -> JSValue {
    match args.first() {
        Some(JSValue::Object(o)) => {
            let borrow = o.borrow();
            JSValue::bool(is_object_extensible(&borrow))
        }
        _ => JSValue::bool(false),
    }
}

/// Object.fromEntries(iterable)
///
/// Creates a new object from an iterable of key-value pairs (such as the
/// result of Object.entries). Also supports Map objects.
pub fn object_from_entries(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = JSValue::object("Object");

    let iterable = match args.first() {
        Some(v) => v,
        None => return obj,
    };

    // Handle Map objects (they have a special internal structure)
    if let JSValue::Object(o) = iterable {
        let borrow = o.borrow();
        if borrow.class_name == "Map" {
            if let Some(JSValue::Object(entries_arr)) = borrow.properties.get("entries") {
                // Call Map.entries() to get the entries iterator, then iterate
                // For simplicity, read the internal entries directly
            }
            // Read the internal data property if present
            if let Some(JSValue::Object(data_obj)) = borrow.properties.get("data") {
                let data_borrow = data_obj.borrow();
                for (key, value) in &data_borrow.properties {
                    obj.set_property(key, value.clone());
                }
            }
            return obj;
        }
    }

    // Handle array-like iterables
    let len = iterable
        .get_property("length")
        .map(|v| v.to_number() as usize)
        .unwrap_or(0);

    for i in 0..len {
        let entry = iterable
            .get_property(&i.to_string())
            .unwrap_or(JSValue::undefined());
        match &entry {
            JSValue::Object(entry_arr) => {
                let borrow = entry_arr.borrow();
                let key = borrow
                    .properties
                    .get("0")
                    .cloned()
                    .unwrap_or(JSValue::undefined())
                    .to_string();
                let value = borrow
                    .properties
                    .get("1")
                    .cloned()
                    .unwrap_or(JSValue::undefined());
                obj.set_property(&key, value);
            }
            JSValue::Function(entry_fn) => {
                let borrow = entry_fn.borrow();
                let key = borrow
                    .closure
                    .get("0")
                    .map(|v| v.borrow().to_string())
                    .unwrap_or_default();
                let value = borrow
                    .closure
                    .get("1")
                    .map(|v| v.borrow().clone())
                    .unwrap_or(JSValue::undefined());
                obj.set_property(&key, value);
            }
            _ => {}
        }
    }

    obj
}

/// Object.hasOwn(obj, prop)
///
/// Returns true if the specified object has the indicated property as its
/// own property (not inherited).
pub fn object_has_own(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::bool(false),
    };
    let prop = args.get(1).map(|v| v.to_string()).unwrap_or_default();

    match obj {
        JSValue::Object(o) => {
            let borrow = o.borrow();
            JSValue::bool(borrow.properties.contains_key(&prop))
        }
        JSValue::Function(f) => {
            let borrow = f.borrow();
            JSValue::bool(borrow.closure.contains_key(&prop))
        }
        _ => JSValue::bool(false),
    }
}

/// Object.groupBy(items, callbackFn)
///
/// Groups array elements by the result of a callback function. Returns an
/// object where keys are the group names and values are arrays of elements
/// in that group.
pub fn object_group_by(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let result = JSValue::object("Object");
    let items = match args.first() {
        Some(v) => v,
        None => return result,
    };
    let callback = match args.get(1) {
        Some(JSValue::Function(f)) => f.clone(),
        _ => return result,
    };

    let len = items
        .get_property("length")
        .map(|v| v.to_number() as usize)
        .unwrap_or(0);

    for i in 0..len {
        let val = items
            .get_property(&i.to_string())
            .unwrap_or(JSValue::undefined());
        let f_borrow = callback.borrow();
        let group_key = match &f_borrow.body {
            FunctionBody::Native(native_fn) => {
                native_fn(
                    &JSValue::undefined(),
                    &[val.clone(), JSValue::int(i as i32), items.clone()],
                )
            }
            FunctionBody::Closure(closure_fn) => {
                closure_fn(
                    &JSValue::undefined(),
                    &[val.clone(), JSValue::int(i as i32), items.clone()],
                )
            }
            _ => JSValue::undefined(),
        };
        let key_str = group_key.to_string();

        // Get or create the group array
        match result.get_property(&key_str) {
            Some(JSValue::Object(arr_rc)) => {
                let current_len = arr_rc
                    .borrow()
                    .properties
                    .get("length")
                    .map(|v| v.to_number() as usize)
                    .unwrap_or(0);
                let arr_val = JSValue::Object(arr_rc);
                arr_val.set_property(&current_len.to_string(), val);
                arr_val.set_property("length", JSValue::int((current_len + 1) as i32));
            }
            _ => {
                let arr = JSValue::object("Array");
                arr.set_property("0", val);
                arr.set_property("length", JSValue::int(1));
                result.set_property(&key_str, arr);
            }
        };
    }

    result
}

// ============================================================================
// Internal helpers for property descriptors
// ============================================================================

/// Apply a property descriptor object to a JSObject (for use by
/// Object.create and defineProperty).
///
/// Handles both data descriptors (value, writable) and accessor
/// descriptors (get, set). For accessor descriptors the getter is
/// called immediately and the result is stored as the property value.
///
/// When modifying an existing property, unspecified attributes preserve
/// their current values (only specified attributes are updated).
fn apply_descriptor_to_existing(obj: &mut JSObject, prop: &str, desc: &JSObject) {
    let has_value = desc.properties.contains_key("value");
    let has_writable = desc.properties.contains_key("writable");
    let has_get = desc.properties.contains_key("get");
    let has_set = desc.properties.contains_key("set");
    let has_enumerable = desc.properties.contains_key("enumerable");
    let has_configurable = desc.properties.contains_key("configurable");

    // Determine if this is a data or accessor descriptor
    let is_accessor = has_get || has_set;

    if is_accessor {
        // Accessor descriptor: call the getter to get the value
        if let Some(getter) = desc.properties.get("get") {
            if let JSValue::Function(f) = getter {
                let f_borrow = f.borrow();
                if let FunctionBody::Native(native_fn) = &f_borrow.body {
                    let val = native_fn(&JSValue::undefined(), &[]);
                    obj.properties.insert(prop.to_string(), val);
                } else if let FunctionBody::Closure(closure_fn) = &f_borrow.body {
                    let val = closure_fn(&JSValue::undefined(), &[]);
                    obj.properties.insert(prop.to_string(), val);
                }
            }
        } else {
            // No getter: property value is undefined
            obj.properties
                .insert(prop.to_string(), JSValue::undefined());
        }
        // Accessor descriptors are not writable in the data sense
    } else if has_value {
        // Data descriptor
        let val = desc
            .properties
            .get("value")
            .cloned()
            .unwrap_or(JSValue::undefined());
        obj.properties.insert(prop.to_string(), val);
    }

    // When the property already exists, preserve unspecified attributes
    // from the current definition.  For new properties, unspecified
    // attributes default to true (the ES default for own properties).
    let property_exists = obj.properties.contains_key(prop);

    let writable = if has_writable {
        desc.properties
            .get("writable")
            .map(|v| v.to_boolean())
            .unwrap_or(false)
    } else if property_exists {
        is_property_writable(obj, prop)
    } else {
        true
    };

    let enumerable = if has_enumerable {
        desc.properties
            .get("enumerable")
            .map(|v| v.to_boolean())
            .unwrap_or(false)
    } else if property_exists {
        is_property_enumerable(obj, prop)
    } else {
        true
    };

    let configurable = if has_configurable {
        desc.properties
            .get("configurable")
            .map(|v| v.to_boolean())
            .unwrap_or(false)
    } else if property_exists {
        is_property_configurable(obj, prop)
    } else {
        true
    };

    set_property_attributes(obj, prop, writable, enumerable, configurable);
}

/// Apply a property descriptor from an argument (JSValue) to a JSObject.
/// This is used by Object.create when the propertiesObject argument is
/// an array of [key, descriptor] pairs or an object of key -> descriptor.
fn apply_property_descriptor_value(obj: &mut JSObject, key: &str, descriptor: &JSValue) {
    match descriptor {
        JSValue::Object(desc_obj) => {
            let desc_borrow = desc_obj.borrow();
            apply_descriptor_to_existing(obj, key, &desc_borrow);
        }
        // If not an object, treat as a simple value property
        val => {
            obj.properties.insert(key.to_string(), val.clone());
            // Use default attributes (all true)
        }
    }
}

// ============================================================================
// Array helpers
// ============================================================================

/// Create an array of string values
fn create_string_array(keys: Vec<String>) -> JSValue {
    let elements: Vec<JSValue> = keys.iter().map(|k| JSValue::string(k)).collect();
    super::array::create_array(elements)
}

/// Create an array of values
fn create_value_array(values: Vec<JSValue>) -> JSValue {
    super::array::create_array(values)
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Object constructor and prototype.
pub fn init_object(ctx: &mut JSContext) {
    // Create the Object constructor function
    let constructor = JSValue::function(
        Some("Object"),
        vec!["value".to_string()],
        FunctionBody::Native(object_constructor),
    );

    // Create Object.prototype
    let prototype = JSValue::object("Object");

    // Add static methods to the constructor
    let static_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("keys", object_keys),
        ("values", object_values),
        ("entries", object_entries),
        ("assign", object_assign),
        ("create", object_create),
        ("defineProperty", object_define_property),
        ("defineProperties", object_define_properties),
        ("getOwnPropertyDescriptor", object_get_own_property_descriptor),
        ("getOwnPropertyDescriptors", object_get_own_property_descriptors),
        ("getOwnPropertyNames", object_get_own_property_names),
        ("getOwnPropertySymbols", object_get_own_property_symbols),
        ("getPrototypeOf", object_get_prototype_of),
        ("setPrototypeOf", object_set_prototype_of),
        ("is", object_is),
        ("freeze", object_freeze),
        ("seal", object_seal),
        ("preventExtensions", object_prevent_extensions),
        ("isFrozen", object_is_frozen),
        ("isSealed", object_is_sealed),
        ("isExtensible", object_is_extensible),
        ("fromEntries", object_from_entries),
        ("hasOwn", object_has_own),
        ("groupBy", object_group_by),
    ];

    for &(name, func) in static_methods {
        constructor.set_property(
            name,
            JSValue::function(
                Some(name),
                vec![],
                FunctionBody::Native(func),
            ),
        );
    }

    // Add prototype methods
    let prototype_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("hasOwnProperty", object_prototype_has_own_property),
        ("toString", object_prototype_to_string),
        ("valueOf", object_prototype_value_of),
        ("isPrototypeOf", object_prototype_is_prototype_of),
        ("propertyIsEnumerable", object_prototype_property_is_enumerable),
        ("toLocaleString", object_prototype_to_string),
    ];

    for &(name, func) in prototype_methods {
        prototype.set_property(
            name,
            JSValue::function(
                Some(name),
                vec![],
                FunctionBody::Native(func),
            ),
        );
    }

    // Set Object.prototype on the constructor
    constructor.set_property("prototype", prototype);

    // Set Object on global object
    ctx.global
        .borrow_mut()
        .properties
        .insert("Object".to_string(), constructor);
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

    // --- Basic static method tests ---

    #[test]
    fn test_object_keys() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::int(2));
        obj.set_property("c", JSValue::int(3));

        let this = JSValue::undefined();
        let keys = object_keys(&this, &[obj]);
        let len = keys.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
    }

    #[test]
    fn test_object_values() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(10));
        obj.set_property("y", JSValue::int(20));

        let this = JSValue::undefined();
        let values = object_values(&this, &[obj]);
        let len = values.get_property("length").unwrap().to_int32();
        assert_eq!(len, 2);
    }

    #[test]
    fn test_object_entries() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));

        let this = JSValue::undefined();
        let entries = object_entries(&this, &[obj]);
        let len = entries.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
    }

    #[test]
    fn test_object_assign() {
        let target = JSValue::object("Object");
        target.set_property("a", JSValue::int(1));
        let source = JSValue::object("Object");
        source.set_property("b", JSValue::int(2));

        let this = JSValue::undefined();
        object_assign(&this, &[target.clone(), source]);
        assert_eq!(target.get_property("b").unwrap().to_int32(), 2);
    }

    #[test]
    fn test_object_is() {
        let this = JSValue::undefined();
        let result = object_is(&this, &[JSValue::int(1), JSValue::int(1)]);
        assert!(result.to_boolean());

        let result = object_is(&this, &[JSValue::int(1), JSValue::int(2)]);
        assert!(!result.to_boolean());

        // NaN === NaN in Object.is
        let result = object_is(&this, &[JSValue::float(f64::NAN), JSValue::float(f64::NAN)]);
        assert!(result.to_boolean());

        // +0 and -0 are different in Object.is
        let result = object_is(&this, &[JSValue::float(0.0), JSValue::float(-0.0)]);
        assert!(!result.to_boolean());
    }

    #[test]
    fn test_object_has_own() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));

        let this = JSValue::undefined();
        assert!(object_has_own(&this, &[obj.clone(), JSValue::string("x")]).to_boolean());
        assert!(!object_has_own(&this, &[obj, JSValue::string("y")]).to_boolean());
    }

    #[test]
    fn test_object_create() {
        let proto = JSValue::object("Object");
        proto.set_property("protoProp", JSValue::int(42));

        let this = JSValue::undefined();
        let obj = object_create(&this, &[proto]);
        assert!(obj.get_property("protoProp").is_some());
    }

    #[test]
    fn test_object_create_with_descriptors() {
        let this = JSValue::undefined();

        // Create an object with a property descriptor
        let props = JSValue::object("Object");
        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::int(42));
        desc.set_property("writable", JSValue::bool(false));
        desc.set_property("enumerable", JSValue::bool(true));
        desc.set_property("configurable", JSValue::bool(false));
        props.set_property("x", desc);

        let obj = object_create(&this, &[JSValue::null(), props]);

        // The property should exist with the correct value
        assert_eq!(obj.get_property("x").unwrap().to_int32(), 42);

        // Check attributes
        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(!is_property_writable(&borrow, "x"));
            assert!(is_property_enumerable(&borrow, "x"));
            assert!(!is_property_configurable(&borrow, "x"));
        }
    }

    #[test]
    fn test_object_define_property() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();

        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::string("hello"));
        desc.set_property("writable", JSValue::bool(false));
        desc.set_property("enumerable", JSValue::bool(true));
        desc.set_property("configurable", JSValue::bool(false));

        object_define_property(&this, &[obj.clone(), JSValue::string("greeting"), desc]);

        // Value should be set
        assert_eq!(obj.get_property("greeting").unwrap().to_string(), "hello");

        // Attributes should be stored
        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(!is_property_writable(&borrow, "greeting"));
            assert!(is_property_enumerable(&borrow, "greeting"));
            assert!(!is_property_configurable(&borrow, "greeting"));
        }
    }

    #[test]
    fn test_object_define_property_non_configurable() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();

        // Define a non-configurable, writable property
        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::int(1));
        desc.set_property("writable", JSValue::bool(true));
        desc.set_property("configurable", JSValue::bool(false));
        object_define_property(&this, &[obj.clone(), JSValue::string("x"), desc]);

        // Per ECMAScript spec: changing writable from true -> false on a
        // non-configurable property IS allowed.
        let desc2 = JSValue::object("Object");
        desc2.set_property("writable", JSValue::bool(false));
        object_define_property(&this, &[obj.clone(), JSValue::string("x"), desc2]);

        // Should now be non-writable
        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(!is_property_writable(&borrow, "x"));
        }

        // But trying to set it back to writable should be rejected
        let desc3 = JSValue::object("Object");
        desc3.set_property("writable", JSValue::bool(true));
        object_define_property(&this, &[obj.clone(), JSValue::string("x"), desc3]);

        // Should still be non-writable
        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(!is_property_writable(&borrow, "x"));
        }

        // After making it non-writable, changing value should be rejected
        // (non-configurable + non-writable means value is locked)
        let desc4 = JSValue::object("Object");
        desc4.set_property("value", JSValue::int(42));
        object_define_property(&this, &[obj.clone(), JSValue::string("x"), desc4]);
        assert_eq!(obj.get_property("x").unwrap().to_int32(), 1);
    }

    #[test]
    fn test_object_define_properties() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();

        let props = JSValue::object("Object");

        let desc_a = JSValue::object("Object");
        desc_a.set_property("value", JSValue::int(10));
        props.set_property("a", desc_a);

        let desc_b = JSValue::object("Object");
        desc_b.set_property("value", JSValue::int(20));
        desc_b.set_property("writable", JSValue::bool(false));
        props.set_property("b", desc_b);

        object_define_properties(&this, &[obj.clone(), props]);

        assert_eq!(obj.get_property("a").unwrap().to_int32(), 10);
        assert_eq!(obj.get_property("b").unwrap().to_int32(), 20);

        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(is_property_writable(&borrow, "a"));
            assert!(!is_property_writable(&borrow, "b"));
        }
    }

    #[test]
    fn test_object_get_own_property_descriptor() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(42));

        let this = JSValue::undefined();
        let desc = object_get_own_property_descriptor(&this, &[obj, JSValue::string("x")]);

        // Should return a descriptor object
        if let JSValue::Object(o) = &desc {
            let borrow = o.borrow();
            assert_eq!(
                borrow.properties.get("value").unwrap().to_int32(),
                42
            );
            assert!(borrow.properties.get("writable").unwrap().to_boolean());
            assert!(borrow.properties.get("enumerable").unwrap().to_boolean());
            assert!(borrow.properties.get("configurable").unwrap().to_boolean());
        } else {
            panic!("Expected descriptor object");
        }
    }

    #[test]
    fn test_object_get_own_property_descriptor_nonexistent() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();
        let desc = object_get_own_property_descriptor(&this, &[obj, JSValue::string("nope")]);
        assert!(desc.is_undefined());
    }

    #[test]
    fn test_object_get_own_property_descriptors() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::string("two"));

        let this = JSValue::undefined();
        let descs = object_get_own_property_descriptors(&this, &[obj]);

        if let JSValue::Object(o) = &descs {
            let borrow = o.borrow();
            assert!(borrow.properties.contains_key("a"));
            assert!(borrow.properties.contains_key("b"));

            // Check descriptor for "a"
            if let Some(JSValue::Object(desc_a)) = borrow.properties.get("a") {
                let da = desc_a.borrow();
                assert_eq!(da.properties.get("value").unwrap().to_int32(), 1);
            } else {
                panic!("Expected descriptor for 'a'");
            }
        } else {
            panic!("Expected descriptors object");
        }
    }

    #[test]
    fn test_object_get_own_property_names() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::int(2));
        obj.set_property("c", JSValue::int(3));

        let this = JSValue::undefined();
        let names = object_get_own_property_names(&this, &[obj]);
        let len = names.get_property("length").unwrap().to_int32();
        assert_eq!(len, 3);
    }

    #[test]
    fn test_object_get_set_prototype_of() {
        let obj = JSValue::object("Object");
        let proto = JSValue::object("Object");
        proto.set_property("test", JSValue::int(99));

        let this = JSValue::undefined();
        object_set_prototype_of(&this, &[obj.clone(), proto]);
        let result = object_get_prototype_of(&this, &[obj]);
        assert!(result.get_property("test").is_some());
    }

    #[test]
    fn test_object_freeze() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();
        object_freeze(&this, &[obj.clone()]);
        assert!(object_is_frozen(&this, &[obj.clone()]).to_boolean());
        assert!(object_is_sealed(&this, &[obj.clone()]).to_boolean());
    }

    #[test]
    fn test_object_freeze_makes_properties_non_writable() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();

        // Before freeze: property is writable
        if let JSValue::Object(o) = &obj {
            assert!(is_property_writable(&o.borrow(), "x"));
        }

        object_freeze(&this, &[obj.clone()]);

        // After freeze: property is non-writable and non-configurable
        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            assert!(!is_property_writable(&borrow, "x"));
            assert!(!is_property_configurable(&borrow, "x"));
        }
    }

    #[test]
    fn test_object_seal() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();
        object_seal(&this, &[obj.clone()]);
        assert!(object_is_sealed(&this, &[obj.clone()]).to_boolean());
        assert!(!object_is_frozen(&this, &[obj.clone()]).to_boolean());
    }

    #[test]
    fn test_object_seal_makes_properties_non_configurable() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();

        object_seal(&this, &[obj.clone()]);

        if let JSValue::Object(o) = &obj {
            let borrow = o.borrow();
            // Sealed: non-configurable but still writable
            assert!(!is_property_configurable(&borrow, "x"));
            assert!(is_property_writable(&borrow, "x"));
        }
    }

    #[test]
    fn test_object_prevent_extensions() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();
        object_prevent_extensions(&this, &[obj.clone()]);
        assert!(!object_is_extensible(&this, &[obj.clone()]).to_boolean());
    }

    #[test]
    fn test_object_define_property_on_frozen() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();

        // Freeze the object
        object_freeze(&this, &[obj.clone()]);

        // Try to define a new property -- should be rejected
        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::int(2));
        object_define_property(&this, &[obj.clone(), JSValue::string("y"), desc]);

        // New property should not exist
        assert!(obj.get_property("y").is_none());
    }

    #[test]
    fn test_object_define_property_on_sealed_no_new() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(1));
        let this = JSValue::undefined();

        // Seal the object
        object_seal(&this, &[obj.clone()]);

        // Try to define a new property -- should be rejected
        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::int(2));
        object_define_property(&this, &[obj.clone(), JSValue::string("y"), desc]);

        assert!(obj.get_property("y").is_none());
    }

    #[test]
    fn test_object_define_property_on_non_extensible() {
        let obj = JSValue::object("Object");
        let this = JSValue::undefined();

        object_prevent_extensions(&this, &[obj.clone()]);

        let desc = JSValue::object("Object");
        desc.set_property("value", JSValue::int(1));
        object_define_property(&this, &[obj.clone(), JSValue::string("x"), desc]);

        // Should not have been added
        assert!(obj.get_property("x").is_none());
    }

    #[test]
    fn test_object_from_entries() {
        let this = JSValue::undefined();

        let entries = JSValue::object("Array");

        let e0 = JSValue::object("Array");
        e0.set_property("0", JSValue::string("a"));
        e0.set_property("1", JSValue::int(1));
        e0.set_property("length", JSValue::int(2));
        entries.set_property("0", e0);

        let e1 = JSValue::object("Array");
        e1.set_property("0", JSValue::string("b"));
        e1.set_property("1", JSValue::int(2));
        e1.set_property("length", JSValue::int(2));
        entries.set_property("1", e1);

        entries.set_property("length", JSValue::int(2));

        let obj = object_from_entries(&this, &[entries]);
        assert_eq!(obj.get_property("a").unwrap().to_int32(), 1);
        assert_eq!(obj.get_property("b").unwrap().to_int32(), 2);
    }

    #[test]
    fn test_object_group_by() {
        let this = JSValue::undefined();

        // Create an array of numbers
        let items = JSValue::object("Array");
        items.set_property("0", JSValue::int(1));
        items.set_property("1", JSValue::int(2));
        items.set_property("2", JSValue::int(3));
        items.set_property("3", JSValue::int(4));
        items.set_property("length", JSValue::int(4));

        // Callback: even or odd
        let callback = JSValue::function(
            Some("groupByFn"),
            vec!["item".to_string()],
            FunctionBody::Native(|_this, args| {
                let val = args.get(0).map(|v| v.to_int32()).unwrap_or(0);
                if val % 2 == 0 {
                    JSValue::string("even")
                } else {
                    JSValue::string("odd")
                }
            }),
        );

        let result = object_group_by(&this, &[items, callback]);

        // Check "odd" group has [1, 3]
        let odd = result.get_property("odd").unwrap();
        assert_eq!(odd.get_property("0").unwrap().to_int32(), 1);
        assert_eq!(odd.get_property("1").unwrap().to_int32(), 3);
        assert_eq!(odd.get_property("length").unwrap().to_int32(), 2);

        // Check "even" group has [2, 4]
        let even = result.get_property("even").unwrap();
        assert_eq!(even.get_property("0").unwrap().to_int32(), 2);
        assert_eq!(even.get_property("1").unwrap().to_int32(), 4);
        assert_eq!(even.get_property("length").unwrap().to_int32(), 2);
    }

    // --- Keys/values/entries filtering ---

    #[test]
    fn test_object_keys_filters_non_enumerable() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::int(2));

        // Make "b" non-enumerable
        if let JSValue::Object(o) = &obj {
            let mut borrow = o.borrow_mut();
            set_property_attributes(&mut borrow, "b", true, false, true);
        }

        let this = JSValue::undefined();
        let keys = object_keys(&this, &[obj.clone()]);
        let len = keys.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1); // Only "a" should be returned
    }

    #[test]
    fn test_object_values_filters_non_enumerable() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::int(2));

        if let JSValue::Object(o) = &obj {
            let mut borrow = o.borrow_mut();
            set_property_attributes(&mut borrow, "b", true, false, true);
        }

        let this = JSValue::undefined();
        let values = object_values(&this, &[obj]);
        let len = values.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
    }

    #[test]
    fn test_object_entries_filters_non_enumerable() {
        let obj = JSValue::object("Object");
        obj.set_property("a", JSValue::int(1));
        obj.set_property("b", JSValue::int(2));

        if let JSValue::Object(o) = &obj {
            let mut borrow = o.borrow_mut();
            set_property_attributes(&mut borrow, "b", true, false, true);
        }

        let this = JSValue::undefined();
        let entries = object_entries(&this, &[obj]);
        let len = entries.get_property("length").unwrap().to_int32();
        assert_eq!(len, 1);
    }

    // --- Object.is edge cases ---

    #[test]
    fn test_object_is_cross_type() {
        let this = JSValue::undefined();

        // Int vs Float with same numeric value
        assert!(object_is(&this, &[JSValue::int(1), JSValue::float(1.0)]).to_boolean());
        assert!(object_is(&this, &[JSValue::float(1.0), JSValue::int(1)]).to_boolean());

        // Different types
        assert!(!object_is(&this, &[JSValue::int(1), JSValue::string("1")]).to_boolean());
        assert!(!object_is(&this, &[JSValue::int(0), JSValue::bool(false)]).to_boolean());

        // null vs undefined
        assert!(!object_is(&this, &[JSValue::null(), JSValue::undefined()]).to_boolean());

        // Same object reference
        let obj = JSValue::object("Object");
        assert!(object_is(&this, &[obj.clone(), obj]).to_boolean());
    }

    // --- Init test ---

    #[test]
    fn test_init_object() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        init_object(&mut ctx);
        let object = ctx.global.borrow().properties.get("Object").cloned();
        assert!(object.is_some());

        // Verify all static methods are present
        let obj_val = object.unwrap();
        let methods = [
            "keys", "values", "entries", "assign", "create",
            "defineProperty", "defineProperties",
            "getOwnPropertyDescriptor", "getOwnPropertyDescriptors",
            "getOwnPropertyNames", "getOwnPropertySymbols",
            "getPrototypeOf", "setPrototypeOf", "is",
            "freeze", "seal", "preventExtensions",
            "isFrozen", "isSealed", "isExtensible",
            "fromEntries", "hasOwn", "groupBy",
        ];
        for method in &methods {
            assert!(
                obj_val.get_property(method).is_some(),
                "Object.{} should exist",
                method
            );
        }
    }
}

// Object.prototype methods

/// Object.prototype.hasOwnProperty(prop)
fn object_prototype_has_own_property(this: &JSValue, args: &[JSValue]) -> JSValue {
    let prop_name = match args.first() {
        Some(v) => v.to_string(),
        None => return JSValue::bool(false),
    };
    match this {
        JSValue::Object(obj) => {
            JSValue::bool(obj.borrow().properties.contains_key(&prop_name))
        }
        _ => JSValue::bool(false),
    }
}

/// Object.prototype.toString()
fn object_prototype_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Object(obj) => {
            let class_name = &obj.borrow().class_name;
            JSValue::string(&format!("[object {}]", class_name))
        }
        JSValue::Function(_) => JSValue::string("[object Function]"),
        _ => JSValue::string("[object Object]"),
    }
}

/// Object.prototype.valueOf()
fn object_prototype_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    this.clone()
}

/// Object.prototype.isPrototypeOf(obj)
fn object_prototype_is_prototype_of(this: &JSValue, args: &[JSValue]) -> JSValue {
    let obj = match args.first() {
        Some(v) => v,
        None => return JSValue::bool(false),
    };
    // Walk the prototype chain of obj to see if this is in it
    let mut current = obj.clone();
    loop {
        let next = match &current {
            JSValue::Object(o) => {
                if let Some(ref proto) = o.borrow().prototype {
                    let proto_val = JSValue::Object(proto.clone());
                    if proto_val.strict_eq(this) {
                        return JSValue::bool(true);
                    }
                    Some(proto_val)
                } else {
                    None
                }
            }
            _ => None,
        };
        match next {
            Some(val) => current = val,
            None => return JSValue::bool(false),
        }
    }
}

/// Object.prototype.propertyIsEnumerable(prop)
fn object_prototype_property_is_enumerable(this: &JSValue, args: &[JSValue]) -> JSValue {
    let prop_name = match args.first() {
        Some(v) => v.to_string(),
        None => return JSValue::bool(false),
    };
    match this {
        JSValue::Object(obj) => {
            // Check if property exists and is enumerable
            let borrow = obj.borrow();
            if borrow.properties.contains_key(&prop_name) {
                // Check enumerable flag in internal_slots
                let key = format!("__e_{}", prop_name);
                let enumerable = borrow.internal_slots.get(&key)
                    .map(|v| v.to_boolean())
                    .unwrap_or(true); // Default is enumerable
                JSValue::bool(enumerable)
            } else {
                JSValue::bool(false)
            }
        }
        _ => JSValue::bool(false),
    }
}
