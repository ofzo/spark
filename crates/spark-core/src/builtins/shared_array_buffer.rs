#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! SharedArrayBuffer built-in.
//!
//! Implements the JavaScript SharedArrayBuffer constructor.

use crate::value::JSValue;
use crate::context::JSContext;

/// SharedArrayBuffer constructor.
pub fn shared_array_buffer_constructor(ctx: &mut JSContext, args: &[JSValue]) -> JSValue {
    // TODO: implement
    JSValue::undefined()
}

/// Initialize the SharedArrayBuffer constructor and prototype.
pub fn init_shared_array_buffer(ctx: &mut JSContext) {
    // TODO: implement
}
