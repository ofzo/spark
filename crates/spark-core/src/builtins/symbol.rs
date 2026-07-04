#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Symbol built-in.
//!
//! Implements the JavaScript Symbol constructor, Symbol.for(), Symbol.keyFor(),
//! and well-known symbols.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::{JSValue, JSSymbol, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// Global Symbol registry
// ============================================================================

thread_local! {
    static SYMBOL_REGISTRY: RefCell<HashMap<String, u64>> = RefCell::new(HashMap::new());
    static SYMBOL_DESCRIPTIONS: RefCell<HashMap<u64, String>> = RefCell::new(HashMap::new());
    static SYMBOL_COUNTER: RefCell<u64> = RefCell::new(1_000_000); // Well-known symbols use small IDs
}

/// Create a new unique symbol ID.
fn new_symbol_id() -> u64 {
    SYMBOL_COUNTER.with(|counter| {
        let mut c = counter.borrow_mut();
        let id = *c;
        *c += 1;
        id
    })
}

// ============================================================================
// Symbol constructor
// ============================================================================

/// Symbol(description) constructor.
pub fn symbol_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let desc = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let id = new_symbol_id();

    if !desc.is_empty() {
        SYMBOL_DESCRIPTIONS.with(|descs| {
            descs.borrow_mut().insert(id, desc.clone());
        });
    }

    JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
        description: if desc.is_empty() { None } else { Some(desc) },
        id,
    })))
}

// ============================================================================
// Symbol.for(key) - Returns a symbol from the global registry
// ============================================================================

/// Symbol.for(key) - Returns the Symbol for the given key from the global registry.
pub fn symbol_for(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let key = args.get(0).map(|v| v.to_string()).unwrap_or_default();

    SYMBOL_REGISTRY.with(|reg| {
        let mut registry = reg.borrow_mut();
        if let Some(&id) = registry.get(&key) {
            SYMBOL_DESCRIPTIONS.with(|descs| {
                let descs = descs.borrow();
                let desc = descs.get(&id).cloned();
                JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
                    description: desc,
                    id,
                })))
            })
        } else {
            let id = new_symbol_id();
            registry.insert(key.clone(), id);
            SYMBOL_DESCRIPTIONS.with(|descs| {
                descs.borrow_mut().insert(id, key.clone());
            });
            JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
                description: Some(key),
                id,
            })))
        }
    })
}

// ============================================================================
// Symbol.keyFor(sym) - Returns the key for a symbol in the global registry
// ============================================================================

/// Symbol.keyFor(sym) - Returns the key associated with the given global Symbol.
pub fn symbol_key_for(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let sym = match args.get(0) {
        Some(JSValue::Symbol(s)) => s.borrow().id,
        _ => return JSValue::undefined(),
    };

    SYMBOL_REGISTRY.with(|reg| {
        let registry = reg.borrow();
        for (key, &id) in registry.iter() {
            if id == sym {
                return JSValue::string(key);
            }
        }
        JSValue::undefined()
    })
}

// ============================================================================
// Symbol.prototype.description
// ============================================================================

/// Symbol.prototype.description getter.
pub fn symbol_prototype_description(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Symbol(s) => {
            let borrow = s.borrow();
            match &borrow.description {
                Some(desc) => JSValue::string(desc),
                None => JSValue::undefined(),
            }
        }
        _ => JSValue::undefined(),
    }
}

// ============================================================================
// Symbol.prototype.valueOf()
// ============================================================================

/// Symbol.prototype.valueOf() - Returns the Symbol itself.
pub fn symbol_prototype_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    this.clone()
}

// ============================================================================
// Symbol.prototype.toString()
// ============================================================================

/// Symbol.prototype.toString() - Returns "Symbol(description)".
pub fn symbol_prototype_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    match this {
        JSValue::Symbol(s) => {
            let borrow = s.borrow();
            match &borrow.description {
                Some(desc) => JSValue::string(&format!("Symbol({})", desc)),
                None => JSValue::string("Symbol()"),
            }
        }
        _ => JSValue::string("Symbol()"),
    }
}

// ============================================================================
// Well-known symbol IDs (using small reserved IDs)
// ============================================================================

/// Well-known symbol IDs
pub const SYMBOL_ITERATOR_ID: u64 = 1;
pub const SYMBOL_TO_PRIMITIVE_ID: u64 = 2;
pub const SYMBOL_TO_STRING_TAG_ID: u64 = 3;
pub const SYMBOL_HAS_INSTANCE_ID: u64 = 4;
pub const SYMBOL_IS_CONCAT_SPREADABLE_ID: u64 = 5;
pub const SYMBOL_ASYNC_ITERATOR_ID: u64 = 6;
pub const SYMBOL_SPECIES_ID: u64 = 7;
pub const SYMBOL_TO_JSON_ID: u64 = 8;

fn create_well_known_symbol(id: u64, desc: &str) -> JSValue {
    SYMBOL_DESCRIPTIONS.with(|descs| {
        descs.borrow_mut().insert(id, desc.to_string());
    });
    JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
        description: Some(desc.to_string()),
        id,
    })))
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Symbol constructor and well-known symbols.
pub fn init_symbol(ctx: &mut JSContext) {
    // Symbol.length = 1
    let symbol_constructor_fn = JSValue::function(
        Some("Symbol"),
        vec!["description".to_string()],
        FunctionBody::Native(symbol_constructor),
    );
    symbol_constructor_fn.set_property("length", JSValue::int(1));

    // Symbol static methods
    symbol_constructor_fn.set_property(
        "for",
        JSValue::function(Some("for"), vec!["key".to_string()], FunctionBody::Native(symbol_for)),
    );
    symbol_constructor_fn.set_property(
        "keyFor",
        JSValue::function(
            Some("keyFor"),
            vec!["sym".to_string()],
            FunctionBody::Native(symbol_key_for),
        ),
    );

    // Symbol.prototype
    let prototype = JSValue::object("Symbol");
    prototype.set_property(
        "description",
        JSValue::function(
            Some("description"),
            vec![],
            FunctionBody::Native(symbol_prototype_description),
        ),
    );
    prototype.set_property(
        "valueOf",
        JSValue::function(
            Some("valueOf"),
            vec![],
            FunctionBody::Native(symbol_prototype_value_of),
        ),
    );
    prototype.set_property(
        "toString",
        JSValue::function(
            Some("toString"),
            vec![],
            FunctionBody::Native(symbol_prototype_to_string),
        ),
    );

    symbol_constructor_fn.set_property("prototype", prototype);

    // Well-known symbols
    symbol_constructor_fn.set_property(
        "iterator",
        create_well_known_symbol(SYMBOL_ITERATOR_ID, "Symbol.iterator"),
    );
    symbol_constructor_fn.set_property(
        "toPrimitive",
        create_well_known_symbol(SYMBOL_TO_PRIMITIVE_ID, "Symbol.toPrimitive"),
    );
    symbol_constructor_fn.set_property(
        "toStringTag",
        create_well_known_symbol(SYMBOL_TO_STRING_TAG_ID, "Symbol.toStringTag"),
    );
    symbol_constructor_fn.set_property(
        "hasInstance",
        create_well_known_symbol(SYMBOL_HAS_INSTANCE_ID, "Symbol.hasInstance"),
    );
    symbol_constructor_fn.set_property(
        "isConcatSpreadable",
        create_well_known_symbol(
            SYMBOL_IS_CONCAT_SPREADABLE_ID,
            "Symbol.isConcatSpreadable",
        ),
    );
    symbol_constructor_fn.set_property(
        "asyncIterator",
        create_well_known_symbol(SYMBOL_ASYNC_ITERATOR_ID, "Symbol.asyncIterator"),
    );
    symbol_constructor_fn.set_property(
        "species",
        create_well_known_symbol(SYMBOL_SPECIES_ID, "Symbol.species"),
    );
    symbol_constructor_fn.set_property(
        "toJSON",
        create_well_known_symbol(SYMBOL_TO_JSON_ID, "Symbol.toJSON"),
    );

    // Install on global
    ctx.global
        .borrow_mut()
        .properties
        .insert("Symbol".to_string(), symbol_constructor_fn);
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
    fn test_symbol_constructor() {
        let this = JSValue::undefined();
        let sym = symbol_constructor(&this, &[JSValue::string("mySymbol")]);
        match &sym {
            JSValue::Symbol(s) => {
                let borrow = s.borrow();
                assert_eq!(borrow.description, Some("mySymbol".to_string()));
            }
            _ => panic!("Expected Symbol"),
        }
    }

    #[test]
    fn test_symbol_no_description() {
        let this = JSValue::undefined();
        let sym = symbol_constructor(&this, &[]);
        match &sym {
            JSValue::Symbol(s) => {
                let borrow = s.borrow();
                assert_eq!(borrow.description, None);
            }
            _ => panic!("Expected Symbol"),
        }
    }

    #[test]
    fn test_symbol_for_and_key_for() {
        let this = JSValue::undefined();
        let sym1 = symbol_for(&this, &[JSValue::string("key1")]);
        let sym2 = symbol_for(&this, &[JSValue::string("key1")]);
        // Same symbol from registry
        if let (JSValue::Symbol(a), JSValue::Symbol(b)) = (&sym1, &sym2) {
            assert_eq!(a.borrow().id, b.borrow().id);
        }

        let key = symbol_key_for(&this, &[sym1.clone()]);
        assert_eq!(key.to_string(), "key1");
    }

    #[test]
    fn test_symbol_to_string() {
        let this = JSValue::undefined();
        let sym = symbol_constructor(&this, &[JSValue::string("test")]);
        let result = symbol_prototype_to_string(&sym, &[]);
        assert_eq!(result.to_string(), "Symbol(test)");
    }

    #[test]
    fn test_init_symbol() {
        let mut ctx = make_ctx();
        init_symbol(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Symbol").is_some());
        let sym_ctor = global.properties.get("Symbol").unwrap();
        assert!(sym_ctor.get_property("iterator").is_some());
        assert!(sym_ctor.get_property("toPrimitive").is_some());
        assert!(sym_ctor.get_property("for").is_some());
        assert!(sym_ctor.get_property("keyFor").is_some());
    }
}
