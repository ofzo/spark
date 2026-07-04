#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! ArrayBuffer built-in.
//!
//! Implements the JavaScript ArrayBuffer constructor and its methods.

use crate::value::JSValue;
use crate::context::JSContext;

/// ArrayBuffer constructor.
pub fn array_buffer_constructor(ctx: &mut JSContext, args: &[JSValue]) -> JSValue {
    // TODO: implement
    JSValue::undefined()
}

/// Initialize the ArrayBuffer constructor and prototype.
pub fn init_array_buffer(ctx: &mut JSContext) {
    // TODO: implement
}
