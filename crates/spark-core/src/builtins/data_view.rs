#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! DataView built-in.
//!
//! Implements the JavaScript DataView constructor and its methods.

use crate::value::JSValue;
use crate::context::JSContext;

/// DataView constructor.
pub fn data_view_constructor(ctx: &mut JSContext, args: &[JSValue]) -> JSValue {
    // TODO: implement
    JSValue::undefined()
}

/// Initialize the DataView constructor and prototype.
pub fn init_data_view(ctx: &mut JSContext) {
    // TODO: implement
}
