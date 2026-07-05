//! JavaScript value representation.
//!
//! Uses Rust's enum system for tagged values, with `Rc<RefCell<...>>`
//! for reference-counted objects. No unsafe code needed.

#![allow(unused_variables, unused_imports, dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

// ============================================================================
// Bytecode types (shared between compiler and interpreter)
// ============================================================================

/// Bytecode opcodes.
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    // Stack operations
    PushUndefined,
    GetGlobal,
    GetGlobalVar(u32),
    PushNull,
    PushBool(bool),
    PushInt(i32),
    PushFloat(u32),
    PushString(u32),
    PushRegExp(u32, u32), // (pattern_idx, flags_idx) - push RegExp literal
    Pop,
    Dup,
    Swap,
    Rotate(u32), // Rotate top N elements: move bottom to top

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,
    Plus,
    Inc,
    Dec,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,
    UShr,

    // Comparison
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    LogicalAnd,
    LogicalOr,
    Not,

    // Control flow
    Jump(u32),
    JumpIfTrue(u32),
    JumpIfFalse(u32),
    Return,
    Throw,
    PushHandler(u32), // Push exception handler with catch_pc
    PopHandler,       // Pop exception handler

    // Variables
    GetVar(u32),
    SetVar(u32),
    DeclareVar(u32, VariableKind),
    This,  // Push the current function's `this` value

    // Closure variables
    GetClosure(u32),
    SetClosure(u32),

    // Properties
    GetProperty,
    SetProperty,
    GetPropertyByName(u32),
    GetField2(u32),
    SetPropertyByName(u32),
    SetProto,      // Set structural prototype: pops [proto, obj], sets obj.__proto__ = proto
    DefineProperty,

    // Calls
    Call(u32),
    CallMethod(u32),  // Like Call but pops `this` from stack (used with GetField2)
    New(u32),
    SuperCall(u32),

    // Objects
    CreateObject,
    CreateArray,

    // Functions
    CreateClosure(u32),
    CreateAsync(u32),
    CreateGenerator(u32),

    // Iterators
    GetIterator,
    IteratorNext,
    IteratorClose,

    // Modules
    Import(u32),
    Export(u32),

    // Other
    Typeof,
    Delete,
    Void,
    Instanceof,
    In,
    Yield,
    Await,
    CopyProperties, // Copy all properties from source to target: pops [target, source], pushes target
    Nop,
}

/// Variable kind for declarations.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum VariableKind {
    Var,
    Let,
    Const,
}

/// Constant value.
#[derive(Debug, Clone)]
pub enum Constant {
    Float(f64),
    String(String),
    BigInt(String),
    RegExp {
        pattern: String,
        flags: String,
    },
}

/// Variable information.
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub kind: VariableKind,
    pub scope_level: u32,
    pub is_captured: bool,
    pub is_parameter: bool,
}

/// Bytecode function.
#[derive(Debug, Clone)]
pub struct BytecodeFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub bytecode: Vec<Opcode>,
    pub constants: Vec<Constant>,
    pub variables: Vec<Variable>,
    pub functions: Vec<BytecodeFunction>,
    pub line_numbers: Vec<(u32, u32)>,
    pub filename: Option<String>,
    pub is_generator: bool,
    pub is_async: bool,
    pub is_arrow: bool,
    pub is_module: bool,
    pub strict_mode: bool,
    /// Index of the rest parameter (if any)
    pub rest_param_index: Option<usize>,
    /// Expected closure variable names, in the order matching closure indices.
    /// Used by CreateClosure to capture only needed variables in the correct order.
    pub closure_vars: Vec<String>,
}

impl BytecodeFunction {
    /// Create a new empty bytecode function.
    pub fn new() -> Self {
        BytecodeFunction {
            name: None,
            params: Vec::new(),
            bytecode: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            line_numbers: Vec::new(),
            filename: None,
            is_generator: false,
            is_async: false,
            is_module: false,
            strict_mode: false,
            rest_param_index: None,
            is_arrow: false,
            closure_vars: Vec::new(),
        }
    }

    /// Get the line number for a given program counter.
    pub fn line_number(&self, pc: u32) -> Option<u32> {
        self.line_numbers
            .iter()
            .rev()
            .find(|&&(line_pc, _)| line_pc <= pc)
            .map(|&(_, line)| line)
    }
}

// ============================================================================
// JavaScript value types
// ============================================================================

/// A JavaScript value.
#[derive(Clone)]
pub enum JSValue {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    String(Rc<RefCell<JSString>>),
    Object(Rc<RefCell<JSObject>>),
    Symbol(Rc<RefCell<JSSymbol>>),
    BigInt(Rc<RefCell<JSBigInt>>),
    Function(Rc<RefCell<JSFunction>>),
}

/// A JavaScript string.
#[derive(Debug, Clone, PartialEq)]
pub struct JSString {
    pub data: String,
    pub hash: u32,
}

/// Property descriptor for getter/setter support.
#[derive(Debug, Clone)]
pub enum PropertyDescriptor {
    /// Data property with value, writable, enumerable, configurable
    Data {
        value: JSValue,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    },
    /// Accessor property with get/set functions
    Accessor {
        get: Option<JSValue>,
        set: Option<JSValue>,
        enumerable: bool,
        configurable: bool,
    },
}

/// A JavaScript object.
#[derive(Debug, Clone)]
pub struct JSObject {
    pub properties: HashMap<String, JSValue>,
    /// Property descriptors for accessor properties (getters/setters)
    pub descriptors: HashMap<String, PropertyDescriptor>,
    pub prototype: Option<Rc<RefCell<JSObject>>>,
    pub internal_slots: HashMap<String, JSValue>,
    pub class_name: String,
}

impl PartialEq for JSObject {
    fn eq(&self, other: &Self) -> bool {
        // Object identity comparison (same pointer)
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

/// A JavaScript symbol.
#[derive(Debug, Clone)]
pub struct JSSymbol {
    pub description: Option<String>,
    pub id: u64,
}

impl PartialEq for JSSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// A JavaScript BigInt.
#[derive(Debug, Clone)]
pub struct JSBigInt {
    pub value: String,
}

impl PartialEq for JSBigInt {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// A JavaScript function.
#[derive(Debug, Clone)]
pub struct JSFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub body: FunctionBody,
    /// Closure variables and function properties.
    /// Uses Rc<RefCell<>> so captured variables can be mutated by inner functions.
    pub closure: HashMap<String, Rc<RefCell<JSValue>>>,
    pub is_constructor: bool,
    pub is_async: bool,
    pub is_generator: bool,
}

impl PartialEq for JSFunction {
    fn eq(&self, other: &Self) -> bool {
        // Function identity comparison (same pointer)
        std::ptr::eq(self as *const Self, other as *const Self)
    }
}

/// Function body representation.
/// State for a suspended generator.
#[derive(Clone)]
pub struct GeneratorState {
    /// The generator's bytecode function
    pub func: BytecodeFunction,
    /// Closure variables captured by the generator
    pub closure: std::collections::HashMap<String, Rc<RefCell<JSValue>>>,
    /// Arguments passed to the generator
    pub args: Vec<JSValue>,
    /// The `this` binding
    pub this: JSValue,
    /// Saved stack frame (from last yield), None if not yet started
    pub saved_frame: Option<BytecodeFunction>,
    /// Saved program counter
    pub saved_pc: u32,
    /// Saved locals
    pub saved_locals: Vec<Rc<RefCell<JSValue>>>,
    /// Saved operand stack
    pub saved_stack: Vec<JSValue>,
    /// Saved closure
    pub saved_closure: Vec<Rc<RefCell<JSValue>>>,
    /// Whether the generator has finished (returned)
    pub done: bool,
    /// Whether the generator has been started
    pub started: bool,
}

#[derive(Clone)]
pub enum FunctionBody {
    /// Native Rust function pointer
    Native(NativeFunction),
    /// Native Rust closure (can capture state)
    Closure(Rc<dyn Fn(&JSValue, &[JSValue]) -> JSValue>),
    /// JavaScript bytecode function reference
    Bytecode(BytecodeFunction),
    /// JavaScript source code
    Source(String),
    /// Generator function (stores bytecode + collected yields)
    Generator {
        func: BytecodeFunction,
        /// Pre-computed yielded values (populated on first next() call)
        yields: Rc<RefCell<Vec<JSValue>>>,
    },
    /// Generator next() method - resumes a suspended generator
    GeneratorNext {
        state: Rc<RefCell<GeneratorState>>,
    },
}

impl fmt::Debug for FunctionBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FunctionBody::Native(_) => write!(f, "Native(..)"),
            FunctionBody::Closure(_) => write!(f, "Closure(..)"),
            FunctionBody::Bytecode(bc) => write!(f, "Bytecode({:?})", bc),
            FunctionBody::Source(s) => write!(f, "Source({:?})", s),
            FunctionBody::Generator { .. } => write!(f, "Generator(..)"),
            FunctionBody::GeneratorNext { .. } => write!(f, "GeneratorNext(..)"),
        }
    }
}

/// A native Rust function. Receives `this` and the arguments.
pub type NativeFunction = fn(&JSValue, &[JSValue]) -> JSValue;

// ============================================================================
// PartialEq for JSValue
// ============================================================================

impl PartialEq for JSValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (JSValue::Undefined, JSValue::Undefined) => true,
            (JSValue::Null, JSValue::Null) => true,
            (JSValue::Bool(a), JSValue::Bool(b)) => a == b,
            (JSValue::Int(a), JSValue::Int(b)) => a == b,
            (JSValue::Float(a), JSValue::Float(b)) => a == b,
            (JSValue::Int(a), JSValue::Float(b)) => (*a as f64) == *b,
            (JSValue::Float(a), JSValue::Int(b)) => *a == (*b as f64),
            (JSValue::String(a), JSValue::String(b)) => Rc::ptr_eq(a, b),
            (JSValue::Object(a), JSValue::Object(b)) => Rc::ptr_eq(a, b),
            (JSValue::Function(a), JSValue::Function(b)) => Rc::ptr_eq(a, b),
            (JSValue::Symbol(a), JSValue::Symbol(b)) => a.borrow().id == b.borrow().id,
            (JSValue::BigInt(a), JSValue::BigInt(b)) => a.borrow().value == b.borrow().value,
            _ => false,
        }
    }
}

// ============================================================================
// JSValue implementation
// ============================================================================

impl JSValue {
    /// Create a new undefined value.
    pub fn undefined() -> Self {
        JSValue::Undefined
    }

    /// Create a new null value.
    pub fn null() -> Self {
        JSValue::Null
    }

    /// Create a new boolean value.
    pub fn bool(val: bool) -> Self {
        JSValue::Bool(val)
    }

    /// Create a new integer value.
    pub fn int(val: i32) -> Self {
        JSValue::Int(val)
    }

    /// Create a new float value.
    pub fn float(val: f64) -> Self {
        JSValue::Float(val)
    }

    /// Create a new string value.
    pub fn string(s: &str) -> Self {
        JSValue::String(Rc::new(RefCell::new(JSString {
            data: s.to_string(),
            hash: Self::hash_string(s),
        })))
    }

    /// Create a new object value.
    pub fn object(class_name: &str) -> Self {
        JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: class_name.to_string(),
        })))
    }

    /// Create a new function value.
    pub fn function(name: Option<&str>, params: Vec<String>, body: FunctionBody) -> Self {
        JSValue::Function(Rc::new(RefCell::new(JSFunction {
            name: name.map(|s| s.to_string()),
            params,
            body,
            closure: HashMap::new(),
            is_constructor: false,
            is_async: false,
            is_generator: false,
        })))
    }

    /// Get the type of this value as a string.
    pub fn type_of(&self) -> &'static str {
        match self {
            JSValue::Undefined => "undefined",
            JSValue::Null => "object",
            JSValue::Bool(_) => "boolean",
            JSValue::Int(_) | JSValue::Float(_) => "number",
            JSValue::String(_) => "string",
            JSValue::Symbol(_) => "symbol",
            JSValue::BigInt(_) => "bigint",
            JSValue::Object(_) => "object",
            JSValue::Function(_) => "function",
        }
    }

    /// Check if this value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(self, JSValue::Undefined)
    }

    /// Check if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, JSValue::Null)
    }

    /// Check if this value is a number.
    pub fn is_number(&self) -> bool {
        matches!(self, JSValue::Int(_) | JSValue::Float(_))
    }

    /// Check if this value is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, JSValue::String(_))
    }

    /// Check if this value is an object (including functions).
    pub fn is_object(&self) -> bool {
        matches!(self, JSValue::Object(_) | JSValue::Function(_))
    }

    /// Check if this value is a callable object.
    pub fn is_callable(&self) -> bool {
        matches!(self, JSValue::Function(_))
    }

    /// Convert this value to a boolean (ToBoolean abstract operation).
    pub fn to_boolean(&self) -> bool {
        match self {
            JSValue::Undefined | JSValue::Null => false,
            JSValue::Bool(b) => *b,
            JSValue::Int(i) => *i != 0,
            JSValue::Float(f) => *f != 0.0 && !f.is_nan(),
            JSValue::String(s) => !s.borrow().data.is_empty(),
            JSValue::BigInt(b) => b.borrow().value != "0",
            JSValue::Object(_) | JSValue::Function(_) => true,
            JSValue::Symbol(_) => true,
        }
    }

    /// Convert this value to a number (ToNumber abstract operation).
    pub fn to_number(&self) -> f64 {
        match self {
            JSValue::Undefined => f64::NAN,
            JSValue::Null => 0.0,
            JSValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            JSValue::Int(i) => *i as f64,
            JSValue::Float(f) => *f,
            JSValue::String(s) => {
                let trimmed = s.borrow().data.trim().to_string();
                if trimmed.is_empty() {
                    0.0
                } else if trimmed == "Infinity" || trimmed == "+Infinity" {
                    f64::INFINITY
                } else if trimmed == "-Infinity" {
                    f64::NEG_INFINITY
                } else if trimmed == "NaN" {
                    f64::NAN
                } else {
                    trimmed.parse::<f64>().unwrap_or(f64::NAN)
                }
            }
            JSValue::BigInt(_) => f64::NAN,
            JSValue::Object(_) | JSValue::Function(_) => f64::NAN,
            JSValue::Symbol(_) => f64::NAN,
        }
    }

    /// Convert this value to an int32 (ToInt32 abstract operation).
    pub fn to_int32(&self) -> i32 {
        let n = self.to_number();
        if n.is_nan() || n.is_infinite() || n == 0.0 {
            0
        } else {
            let two32 = 4294967296.0_f64; // 2^32
            let int_part = n.trunc();
            let int32_mod = int_part.rem_euclid(two32);
            if int32_mod >= 2147483648.0 {
                (int32_mod - two32) as i32
            } else {
                int32_mod as i32
            }
        }
    }

    /// Convert this value to a uint32 (ToUint32 abstract operation).
    pub fn to_uint32(&self) -> u32 {
        let n = self.to_number();
        if n.is_nan() || n.is_infinite() || n == 0.0 {
            0
        } else {
            let two32 = 4294967296.0_f64;
            let int_part = n.trunc();
            let uint32_mod = int_part.rem_euclid(two32);
            uint32_mod as u32
        }
    }

    /// Convert this value to a string (ToString abstract operation).
    pub fn to_string(&self) -> String {
        match self {
            JSValue::Undefined => "undefined".to_string(),
            JSValue::Null => "null".to_string(),
            JSValue::Bool(b) => b.to_string(),
            JSValue::Int(i) => i.to_string(),
            JSValue::Float(f) => {
                if f.is_nan() {
                    "NaN".to_string()
                } else if f.is_infinite() {
                    if *f > 0.0 {
                        "Infinity".to_string()
                    } else {
                        "-Infinity".to_string()
                    }
                } else if *f == 0.0 {
                    "0".to_string()
                } else {
                    // JavaScript number-to-string: remove trailing zeros
                    let s = format!("{}", f);
                    if s.contains('.') {
                        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                        trimmed.to_string()
                    } else {
                        s
                    }
                }
            }
            JSValue::String(s) => s.borrow().data.clone(),
            JSValue::BigInt(b) => b.borrow().value.clone() + "n",
            JSValue::Object(obj) => {
                let borrow = obj.borrow();
                if borrow.class_name == "Array" {
                    // Array.prototype.toString: join elements with commas
                    let len = borrow.properties.get("length")
                        .map(|v| v.to_number() as usize)
                        .unwrap_or(0);
                    let parts: Vec<String> = (0..len)
                        .map(|i| {
                            borrow.properties.get(&i.to_string())
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "undefined".to_string())
                        })
                        .collect();
                    parts.join(",")
                } else {
                    "[object Object]".to_string()
                }
            }
            JSValue::Function(f) => {
                let name = f.borrow().name.clone().unwrap_or_default();
                if name.is_empty() {
                    "function() { [native code] }".to_string()
                } else {
                    format!("function {}() {{ [native code] }}", name)
                }
            }
            JSValue::Symbol(s) => {
                let desc = s.borrow().description.clone().unwrap_or_default();
                if desc.is_empty() {
                    "Symbol()".to_string()
                } else {
                    format!("Symbol({})", desc)
                }
            }
        }
    }

    /// Convert this value to an object (ToObject abstract operation).
    pub fn to_object(&self) -> Result<JSValue, String> {
        match self {
            JSValue::Undefined | JSValue::Null => {
                Err("Cannot convert undefined or null to object".to_string())
            }
            JSValue::Bool(b) => {
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "Boolean".to_string(),
                };
                obj.internal_slots
                    .insert("PrimitiveValue".to_string(), JSValue::bool(*b));
                Ok(JSValue::Object(Rc::new(RefCell::new(obj))))
            }
            JSValue::Int(_) | JSValue::Float(_) => {
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "Number".to_string(),
                };
                obj.internal_slots
                    .insert("PrimitiveValue".to_string(), self.clone());
                Ok(JSValue::Object(Rc::new(RefCell::new(obj))))
            }
            JSValue::String(_) => {
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "String".to_string(),
                };
                obj.internal_slots
                    .insert("PrimitiveValue".to_string(), self.clone());
                Ok(JSValue::Object(Rc::new(RefCell::new(obj))))
            }
            JSValue::Object(_) | JSValue::Function(_) | JSValue::Symbol(_) => Ok(self.clone()),
            JSValue::BigInt(_) => {
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "BigInt".to_string(),
                };
                obj.internal_slots
                    .insert("PrimitiveValue".to_string(), self.clone());
                Ok(JSValue::Object(Rc::new(RefCell::new(obj))))
            }
        }
    }

    /// ToPrimitive abstract operation.
    pub fn to_primitive(&self, hint: &str) -> Result<JSValue, String> {
        match self {
            JSValue::Object(_) | JSValue::Function(_) => {
                // For objects, call valueOf() or toString()
                let obj = match self {
                    JSValue::Object(o) => o.clone(),
                    JSValue::Function(f) => {
                        // Functions have valueOf from Object.prototype
                        return Ok(self.clone());
                    }
                    _ => unreachable!(),
                };

                // Try valueOf first (for "number" hint, try valueOf first; for "string", try toString first)
                let (first_method, second_method) = if hint == "string" {
                    ("toString", "valueOf")
                } else {
                    ("valueOf", "toString")
                };

                if let Some(method) = obj.borrow().properties.get(first_method) {
                    if let JSValue::Function(f) = method {
                        let this_ref = self.clone();
                        let result = {
                            let func = f.borrow();
                            match &func.body {
                                FunctionBody::Native(native_fn) => {
                                    native_fn(&this_ref, &[])
                                }
                                _ => JSValue::undefined(),
                            }
                        };
                        if result.is_object() {
                            // Try second method
                            if let Some(method2) = obj.borrow().properties.get(second_method) {
                                if let JSValue::Function(f2) = method2 {
                                    let this_ref2 = self.clone();
                                    let result2 = {
                                        let func2 = f2.borrow();
                                        match &func2.body {
                                            FunctionBody::Native(native_fn) => {
                                                native_fn(&this_ref2, &[])
                                            }
                                            _ => JSValue::undefined(),
                                        }
                                    };
                                    if !result2.is_object() {
                                        return Ok(result2);
                                    }
                                }
                            }
                            return Err("Cannot convert object to primitive".to_string());
                        }
                        return Ok(result);
                    }
                }

                if let Some(method) = obj.borrow().properties.get(second_method) {
                    if let JSValue::Function(f) = method {
                        let this_ref = self.clone();
                        let result = {
                            let func = f.borrow();
                            match &func.body {
                                FunctionBody::Native(native_fn) => {
                                    native_fn(&this_ref, &[])
                                }
                                _ => JSValue::undefined(),
                            }
                        };
                        if !result.is_object() {
                            return Ok(result);
                        }
                    }
                }

                Err("Cannot convert object to primitive".to_string())
            }
            _ => Ok(self.clone()),
        }
    }

    /// Get the hash of a string.
    pub fn hash_string(s: &str) -> u32 {
        let mut hash: u32 = 0;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }

    /// Check if this value is truthy.
    pub fn is_truthy(&self) -> bool {
        self.to_boolean()
    }

    /// Check if this value is falsy.
    pub fn is_falsy(&self) -> bool {
        !self.to_boolean()
    }

    /// Strict equality comparison (===).
    pub fn strict_eq(&self, other: &JSValue) -> bool {
        match (self, other) {
            (JSValue::Undefined, JSValue::Undefined) => true,
            (JSValue::Null, JSValue::Null) => true,
            (JSValue::Bool(a), JSValue::Bool(b)) => a == b,
            (JSValue::Int(a), JSValue::Int(b)) => a == b,
            (JSValue::Float(a), JSValue::Float(b)) => a == b,
            (JSValue::Int(a), JSValue::Float(b)) => (*a as f64) == *b,
            (JSValue::Float(a), JSValue::Int(b)) => *a == (*b as f64),
            (JSValue::String(a), JSValue::String(b)) => a.borrow().data == b.borrow().data,
            (JSValue::BigInt(a), JSValue::BigInt(b)) => a.borrow().value == b.borrow().value,
            (JSValue::Object(a), JSValue::Object(b)) => Rc::ptr_eq(a, b),
            (JSValue::Function(a), JSValue::Function(b)) => Rc::ptr_eq(a, b),
            (JSValue::Symbol(a), JSValue::Symbol(b)) => a.borrow().id == b.borrow().id,
            _ => false,
        }
    }

    /// Abstract equality comparison (==).
    pub fn abstract_eq(&self, other: &JSValue) -> bool {
        // null == undefined
        match (self, other) {
            (JSValue::Null, JSValue::Undefined) | (JSValue::Undefined, JSValue::Null) => {
                return true;
            }
            _ => {}
        }

        // If same type (and both primitives), use strict equality
        let type_disc = std::mem::discriminant(self);
        let type_disc2 = std::mem::discriminant(other);
        if type_disc == type_disc2 {
            return self.strict_eq(other);
        }

        // Both are numeric types (Int or Float) - compare as numbers
        if matches!(self, JSValue::Int(_) | JSValue::Float(_))
            && matches!(other, JSValue::Int(_) | JSValue::Float(_))
        {
            return self.to_number() == other.to_number();
        }

        // Boolean: coerce to number and retry
        if matches!(self, JSValue::Bool(_)) {
            return JSValue::Float(self.to_number()).abstract_eq(other);
        }
        if matches!(other, JSValue::Bool(_)) {
            return self.abstract_eq(&JSValue::Float(other.to_number()));
        }

        // Number == String: coerce string to number
        if matches!(self, JSValue::Int(_) | JSValue::Float(_))
            && matches!(other, JSValue::String(_))
        {
            let a = self.to_number();
            let b = other.to_number();
            return a == b;
        }
        if matches!(self, JSValue::String(_))
            && matches!(other, JSValue::Int(_) | JSValue::Float(_))
        {
            let a = self.to_number();
            let b = other.to_number();
            return a == b;
        }

        // Object == Primitive: call ToPrimitive
        if matches!(self, JSValue::Object(_) | JSValue::Function(_)) {
            if let Ok(primitive) = self.to_primitive("number") {
                return primitive.abstract_eq(other);
            }
            return false;
        }
        if matches!(other, JSValue::Object(_) | JSValue::Function(_)) {
            if let Ok(primitive) = other.to_primitive("number") {
                return self.abstract_eq(&primitive);
            }
            return false;
        }

        // BigInt == String
        if (matches!(self, JSValue::BigInt(_)) && matches!(other, JSValue::String(_)))
            || (matches!(self, JSValue::String(_)) && matches!(other, JSValue::BigInt(_)))
        {
            let a_str = self.to_string();
            let b_str = other.to_string();
            if let (Ok(a_big), Ok(b_big)) = (
                a_str.trim_end_matches('n').parse::<i64>(),
                b_str.trim_end_matches('n').parse::<i64>(),
            ) {
                return a_big == b_big;
            }
            return false;
        }

        // BigInt == Number
        if (matches!(self, JSValue::BigInt(_))
            && matches!(other, JSValue::Int(_) | JSValue::Float(_)))
            || (matches!(self, JSValue::Int(_) | JSValue::Float(_))
                && matches!(other, JSValue::BigInt(_)))
        {
            let a = self.to_number();
            let b = other.to_number();
            return a == b;
        }

        false
    }

    /// Get a property by name, traversing the prototype chain.
    pub fn get_property(&self, name: &str) -> Option<JSValue> {
        let mut visited = std::collections::HashSet::new();
        self.get_property_inner(name, self, &mut visited)
    }

    /// Internal property lookup that tracks the original receiver for getter calls.
    fn get_property_inner(&self, name: &str, receiver: &JSValue, visited: &mut std::collections::HashSet<usize>) -> Option<JSValue> {
        match self {
            JSValue::Function(f) => {
                let borrow = f.borrow();
                if let Some(val) = borrow.closure.get(name) {
                    return Some(val.borrow().clone());
                }
                if name == "length" {
                    return Some(JSValue::Int(borrow.params.len() as i32));
                }
                if name == "name" {
                    return Some(JSValue::string(
                        borrow.name.as_deref().unwrap_or(""),
                    ));
                }
                None
            }
            JSValue::Object(obj) => {
                let borrow = obj.borrow();

                // Check for accessor descriptor (getter)
                if let Some(desc) = borrow.descriptors.get(name) {
                    match desc {
                        PropertyDescriptor::Accessor { get: Some(getter), .. } => {
                            let getter_clone = getter.clone();
                            drop(borrow);
                            // Call the getter with the original receiver as `this`
                            if let JSValue::Function(f) = &getter_clone {
                                let func = f.borrow();
                                match &func.body {
                                    FunctionBody::Native(native_fn) => {
                                        return Some(native_fn(receiver, &[]));
                                    }
                                    _ => {}
                                }
                            }
                            return None;
                        }
                        PropertyDescriptor::Data { value, .. } => {
                            return Some(value.clone());
                        }
                        _ => {}
                    }
                }

                if let Some(val) = borrow.properties.get(name) {
                    return Some(val.clone());
                }

                // Traverse prototype chain (pass original receiver through)
                if let Some(ref proto) = borrow.prototype {
                    let proto_ptr = Rc::as_ptr(proto) as usize;
                    if visited.contains(&proto_ptr) {
                        return None; // Cycle detected
                    }
                    visited.insert(proto_ptr);
                    let proto_clone = proto.clone();
                    drop(borrow);
                    let proto_val = JSValue::Object(proto_clone);
                    return proto_val.get_property_inner(name, receiver, visited);
                }

                None
            }
            JSValue::String(s) => {
                if name == "length" {
                    Some(JSValue::Int(s.borrow().data.len() as i32))
                } else if let Ok(index) = name.parse::<usize>() {
                    let data = s.borrow().data.clone();
                    data.chars().nth(index).map(|c| JSValue::string(&c.to_string()))
                } else {
                    None
                }
            }
            JSValue::Int(_) | JSValue::Float(_) => None,
            _ => None,
        }
    }

    /// Define a getter property on this object.
    pub fn define_getter(&self, name: &str, getter: JSValue) {
        if let JSValue::Object(obj) = self {
            let mut borrow = obj.borrow_mut();
            borrow.descriptors.insert(
                name.to_string(),
                PropertyDescriptor::Accessor {
                    get: Some(getter),
                    set: None,
                    enumerable: true,
                    configurable: true,
                },
            );
        }
    }

    /// Define a setter property on this object.
    pub fn define_setter(&self, name: &str, setter: JSValue) {
        if let JSValue::Object(obj) = self {
            let mut borrow = obj.borrow_mut();
            // Check if there's already a getter descriptor
            if let Some(PropertyDescriptor::Accessor { get, .. }) = borrow.descriptors.get(name) {
                let getter = get.clone();
                borrow.descriptors.insert(
                    name.to_string(),
                    PropertyDescriptor::Accessor {
                        get: getter,
                        set: Some(setter),
                        enumerable: true,
                        configurable: true,
                    },
                );
            } else {
                borrow.descriptors.insert(
                    name.to_string(),
                    PropertyDescriptor::Accessor {
                        get: None,
                        set: Some(setter),
                        enumerable: true,
                        configurable: true,
                    },
                );
            }
        }
    }

    /// Set a property by name.
    pub fn set_property(&self, name: &str, value: JSValue) {
        match self {
            JSValue::Object(obj) => {
                // Check for setter descriptor
                let has_setter = {
                    let borrow = obj.borrow();
                    matches!(borrow.descriptors.get(name), Some(PropertyDescriptor::Accessor { set: Some(_), .. }))
                };
                if has_setter {
                    let borrow = obj.borrow();
                    if let Some(PropertyDescriptor::Accessor { set: Some(setter), .. }) = borrow.descriptors.get(name) {
                        let setter_clone = setter.clone();
                        drop(borrow);
                        // Call the setter
                        if let JSValue::Function(f) = &setter_clone {
                            let func = f.borrow();
                            match &func.body {
                                FunctionBody::Native(native_fn) => {
                                    native_fn(self, &[value]);
                                }
                                _ => {}
                            }
                        }
                        return;
                    }
                }
                obj.borrow_mut()
                    .properties
                    .insert(name.to_string(), value);
            }
            JSValue::Function(func) => {
                func.borrow_mut()
                    .closure
                    .insert(name.to_string(), Rc::new(RefCell::new(value)));
            }
            _ => {}
        }
    }

    /// Check if a property exists.
    pub fn has_property(&self, name: &str) -> bool {
        match self {
            JSValue::Object(obj) => {
                let borrow = obj.borrow();
                if borrow.properties.contains_key(name) {
                    return true;
                }
                if let Some(ref proto) = borrow.prototype {
                    let proto_val = JSValue::Object(proto.clone());
                    return proto_val.has_property(name);
                }
                false
            }
            JSValue::Function(func) => func.borrow().closure.contains_key(name),
            _ => false,
        }
    }
}

impl fmt::Debug for JSValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JSValue::Undefined => write!(f, "undefined"),
            JSValue::Null => write!(f, "null"),
            JSValue::Bool(b) => write!(f, "{}", b),
            JSValue::Int(i) => write!(f, "{}", i),
            JSValue::Float(fl) => write!(f, "{}", fl),
            JSValue::String(s) => write!(f, "\"{}\"", s.borrow().data),
            JSValue::Object(o) => write!(f, "[object {}]", o.borrow().class_name),
            JSValue::Function(func) => {
                let name = func.borrow().name.clone().unwrap_or_default();
                write!(f, "function {}() {{ ... }}", name)
            }
            JSValue::Symbol(s) => {
                let desc = s.borrow().description.clone().unwrap_or_default();
                write!(f, "Symbol({})", desc)
            }
            JSValue::BigInt(b) => write!(f, "{}n", b.borrow().value),
        }
    }
}

impl fmt::Display for JSValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        assert!(JSValue::undefined().is_undefined());
        assert!(JSValue::null().is_null());
        assert!(JSValue::bool(true).to_boolean());
        assert!(!JSValue::bool(false).to_boolean());
        assert_eq!(JSValue::int(42).to_int32(), 42);
        assert_eq!(JSValue::float(3.14).to_number(), 3.14);
    }

    #[test]
    fn test_type_of() {
        assert_eq!(JSValue::undefined().type_of(), "undefined");
        assert_eq!(JSValue::null().type_of(), "object");
        assert_eq!(JSValue::bool(true).type_of(), "boolean");
        assert_eq!(JSValue::int(42).type_of(), "number");
        assert_eq!(JSValue::float(3.14).type_of(), "number");
        assert_eq!(JSValue::string("hello").type_of(), "string");
    }

    #[test]
    fn test_to_boolean() {
        assert!(!JSValue::undefined().to_boolean());
        assert!(!JSValue::null().to_boolean());
        assert!(!JSValue::bool(false).to_boolean());
        assert!(JSValue::bool(true).to_boolean());
        assert!(!JSValue::int(0).to_boolean());
        assert!(JSValue::int(1).to_boolean());
        assert!(!JSValue::float(0.0).to_boolean());
        assert!(JSValue::float(1.0).to_boolean());
        assert!(!JSValue::string("").to_boolean());
        assert!(JSValue::string("hello").to_boolean());
    }

    #[test]
    fn test_to_number() {
        assert!(JSValue::undefined().to_number().is_nan());
        assert_eq!(JSValue::null().to_number(), 0.0);
        assert_eq!(JSValue::bool(true).to_number(), 1.0);
        assert_eq!(JSValue::bool(false).to_number(), 0.0);
        assert_eq!(JSValue::int(42).to_number(), 42.0);
        assert_eq!(JSValue::float(3.14).to_number(), 3.14);
        assert_eq!(JSValue::string("42").to_number(), 42.0);
        assert!(JSValue::string("abc").to_number().is_nan());
    }

    #[test]
    fn test_to_string() {
        assert_eq!(JSValue::undefined().to_string(), "undefined");
        assert_eq!(JSValue::null().to_string(), "null");
        assert_eq!(JSValue::bool(true).to_string(), "true");
        assert_eq!(JSValue::int(42).to_string(), "42");
        assert_eq!(JSValue::string("hello").to_string(), "hello");
    }

    #[test]
    fn test_strict_eq() {
        assert!(JSValue::undefined().strict_eq(&JSValue::undefined()));
        assert!(JSValue::null().strict_eq(&JSValue::null()));
        assert!(!JSValue::undefined().strict_eq(&JSValue::null()));
        assert!(JSValue::int(42).strict_eq(&JSValue::int(42)));
        assert!(!JSValue::int(42).strict_eq(&JSValue::int(43)));
        assert!(JSValue::string("hello").strict_eq(&JSValue::string("hello")));
    }

    #[test]
    fn test_abstract_eq() {
        // null == undefined
        assert!(JSValue::null().abstract_eq(&JSValue::undefined()));
        assert!(JSValue::undefined().abstract_eq(&JSValue::null()));

        // Number == String
        assert!(JSValue::int(1).abstract_eq(&JSValue::string("1")));
        assert!(JSValue::string("1").abstract_eq(&JSValue::int(1)));

        // Boolean == Number
        assert!(JSValue::bool(true).abstract_eq(&JSValue::int(1)));
        assert!(JSValue::bool(false).abstract_eq(&JSValue::int(0)));
    }

    #[test]
    fn test_object_creation() {
        let obj = JSValue::object("Object");
        assert!(obj.is_object());
        assert_eq!(obj.type_of(), "object");
    }

    #[test]
    fn test_function_creation() {
        let func = JSValue::function(
            Some("test"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this: &JSValue, args: &[JSValue]| JSValue::int(args.len() as i32)),
        );
        assert!(func.is_object());
        assert_eq!(func.type_of(), "function");
    }

    #[test]
    fn test_property_access() {
        let obj = JSValue::object("Object");
        obj.set_property("x", JSValue::int(42));
        assert_eq!(
            obj.get_property("x"),
            Some(JSValue::int(42))
        );
        assert!(obj.has_property("x"));
        assert!(!obj.has_property("y"));
    }

    #[test]
    fn test_prototype_chain() {
        let proto = JSValue::object("Object");
        proto.set_property("protoProp", JSValue::int(1));

        let obj = JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: match proto {
                JSValue::Object(o) => Some(o),
                _ => unreachable!(),
            },
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        })));

        assert_eq!(
            obj.get_property("protoProp"),
            Some(JSValue::int(1))
        );
    }

    #[test]
    fn test_to_int32_edge_cases() {
        // NaN -> 0
        assert_eq!(JSValue::Float(f64::NAN).to_int32(), 0);
        // Infinity -> 0
        assert_eq!(JSValue::Float(f64::INFINITY).to_int32(), 0);
        // -Infinity -> 0
        assert_eq!(JSValue::Float(f64::NEG_INFINITY).to_int32(), 0);
        // 0.0 -> 0
        assert_eq!(JSValue::Float(0.0).to_int32(), 0);
        assert_eq!(JSValue::Float(-0.0).to_int32(), 0);
        // Large number that wraps: 2^32 + 5 = 4294967301 -> wraps to 5
        assert_eq!(JSValue::Float(4294967301.0).to_int32(), 5);
        // Negative number: -1 -> 2^32 - 1 = 4294967295 as i32 = -1
        assert_eq!(JSValue::Float(-1.0).to_int32(), -1);
        // Exactly 2^31 = 2147483648 -> wraps to -2147483648
        assert_eq!(JSValue::Float(2147483648.0).to_int32(), -2147483648i32);
        // 2^32 - 1 = 4294967295 -> wraps to -1
        assert_eq!(JSValue::Float(4294967295.0).to_int32(), -1);
        // Int values pass through directly
        assert_eq!(JSValue::Int(100).to_int32(), 100);
        assert_eq!(JSValue::Int(-100).to_int32(), -100);
        // String "5" -> 5
        assert_eq!(JSValue::string("5").to_int32(), 5);
        // Empty string -> NaN -> 0
        assert_eq!(JSValue::string("").to_int32(), 0);
    }

    #[test]
    fn test_to_uint32_edge_cases() {
        // NaN -> 0
        assert_eq!(JSValue::Float(f64::NAN).to_uint32(), 0);
        // Infinity -> 0
        assert_eq!(JSValue::Float(f64::INFINITY).to_uint32(), 0);
        // -Infinity -> 0
        assert_eq!(JSValue::Float(f64::NEG_INFINITY).to_uint32(), 0);
        // 0.0 -> 0
        assert_eq!(JSValue::Float(0.0).to_uint32(), 0);
        // Negative wraps: -1 -> 2^32 - 1 = 4294967295
        assert_eq!(JSValue::Float(-1.0).to_uint32(), 4294967295);
        // Large number: 2^32 + 7 -> 7
        assert_eq!(JSValue::Float(4294967303.0).to_uint32(), 7);
        // Exactly 2^32 -> 0
        assert_eq!(JSValue::Float(4294967296.0).to_uint32(), 0);
        // Int values
        assert_eq!(JSValue::Int(42).to_uint32(), 42);
        assert_eq!(JSValue::Int(-42).to_uint32(), 4294967254);
    }

    #[test]
    fn test_to_string_float() {
        assert_eq!(JSValue::Float(f64::NAN).to_string(), "NaN");
        assert_eq!(JSValue::Float(f64::INFINITY).to_string(), "Infinity");
        assert_eq!(JSValue::Float(f64::NEG_INFINITY).to_string(), "-Infinity");
        assert_eq!(JSValue::Float(0.0).to_string(), "0");
        assert_eq!(JSValue::Float(-0.0).to_string(), "0");
        assert_eq!(JSValue::Float(1.5).to_string(), "1.5");
        // Trailing zeros removed: 1.10 -> "1.1"
        assert_eq!(JSValue::Float(1.10).to_string(), "1.1");
        // Integer-valued float
        assert_eq!(JSValue::Float(100.0).to_string(), "100");
    }

    #[test]
    fn test_to_string_object() {
        // Array toString: join with commas
        let arr = JSValue::object("Array");
        arr.set_property("length", JSValue::Int(3));
        arr.set_property("0", JSValue::string("a"));
        arr.set_property("1", JSValue::string("b"));
        arr.set_property("2", JSValue::string("c"));
        assert_eq!(arr.to_string(), "a,b,c");

        // Empty array
        let empty_arr = JSValue::object("Array");
        empty_arr.set_property("length", JSValue::Int(0));
        assert_eq!(empty_arr.to_string(), "");

        // Regular object -> "[object Object]"
        let obj = JSValue::object("Object");
        assert_eq!(obj.to_string(), "[object Object]");
    }

    #[test]
    fn test_to_string_function() {
        // Named function
        let named = JSValue::function(
            Some("myFunc"),
            vec![],
            FunctionBody::Native(|_, _| JSValue::undefined()),
        );
        assert_eq!(named.to_string(), "function myFunc() { [native code] }");

        // Anonymous function (empty name)
        let anon = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
        assert_eq!(anon.to_string(), "function() { [native code] }");
    }

    #[test]
    fn test_to_string_symbol() {
        // With description
        let sym_desc = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
            description: Some("mySymbol".to_string()),
            id: 1,
        })));
        assert_eq!(sym_desc.to_string(), "Symbol(mySymbol)");

        // Without description
        let sym_no_desc = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
            description: None,
            id: 2,
        })));
        assert_eq!(sym_no_desc.to_string(), "Symbol()");

        // Empty description
        let sym_empty = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol {
            description: Some("".to_string()),
            id: 3,
        })));
        assert_eq!(sym_empty.to_string(), "Symbol()");
    }

    #[test]
    fn test_to_string_bigint() {
        let bi = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt {
            value: "12345".to_string(),
        })));
        assert_eq!(bi.to_string(), "12345n");

        let bi_zero = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt {
            value: "0".to_string(),
        })));
        assert_eq!(bi_zero.to_string(), "0n");

        let bi_neg = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt {
            value: "-999".to_string(),
        })));
        assert_eq!(bi_neg.to_string(), "-999n");
    }

    #[test]
    fn test_to_object() {
        // Bool -> Boolean wrapper with PrimitiveValue
        let bool_obj = JSValue::bool(true).to_object().unwrap();
        if let JSValue::Object(obj) = &bool_obj {
            let borrow = obj.borrow();
            assert_eq!(borrow.class_name, "Boolean");
            assert_eq!(
                borrow.internal_slots.get("PrimitiveValue"),
                Some(&JSValue::bool(true))
            );
        } else {
            panic!("Expected Object for bool to_object");
        }

        // Int -> Number wrapper
        let int_obj = JSValue::int(42).to_object().unwrap();
        if let JSValue::Object(obj) = &int_obj {
            let borrow = obj.borrow();
            assert_eq!(borrow.class_name, "Number");
            assert_eq!(
                borrow.internal_slots.get("PrimitiveValue"),
                Some(&JSValue::int(42))
            );
        } else {
            panic!("Expected Object for int to_object");
        }

        // Float -> Number wrapper
        let float_obj = JSValue::float(3.14).to_object().unwrap();
        if let JSValue::Object(obj) = &float_obj {
            let borrow = obj.borrow();
            assert_eq!(borrow.class_name, "Number");
        } else {
            panic!("Expected Object for float to_object");
        }

        // String -> String wrapper
        let str_obj = JSValue::string("hello").to_object().unwrap();
        if let JSValue::Object(obj) = &str_obj {
            let borrow = obj.borrow();
            assert_eq!(borrow.class_name, "String");
            let pv = borrow.internal_slots.get("PrimitiveValue").unwrap();
            assert!(pv.strict_eq(&JSValue::string("hello")));
        } else {
            panic!("Expected Object for string to_object");
        }

        // BigInt -> BigInt wrapper
        let bi_obj = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt {
            value: "42".to_string(),
        })))
        .to_object()
        .unwrap();
        if let JSValue::Object(obj) = &bi_obj {
            let borrow = obj.borrow();
            assert_eq!(borrow.class_name, "BigInt");
        } else {
            panic!("Expected Object for BigInt to_object");
        }

        // Object -> self
        let obj = JSValue::object("Foo");
        let result = obj.to_object().unwrap();
        assert!(Rc::ptr_eq(
            match &obj { JSValue::Object(o) => o, _ => panic!() },
            match &result { JSValue::Object(o) => o, _ => panic!() },
        ));

        // Function -> self
        let func = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
        let result = func.to_object().unwrap();
        assert!(result.strict_eq(&func));

        // Symbol -> self
        let sym = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: None, id: 10 })));
        let result = sym.to_object().unwrap();
        assert!(result.strict_eq(&sym));

        // Undefined -> error
        assert!(JSValue::undefined().to_object().is_err());

        // Null -> error
        assert!(JSValue::null().to_object().is_err());
    }

    #[test]
    fn test_to_primitive() {
        // On an object with valueOf returning a primitive
        let mut obj = JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        };
        let value_of_fn = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|this, _| JSValue::int(42)),
        );
        obj.properties
            .insert("valueOf".to_string(), value_of_fn.clone());
        let obj_val = JSValue::Object(Rc::new(RefCell::new(obj)));
        let prim = obj_val.to_primitive("number").unwrap();
        assert_eq!(prim, JSValue::int(42));

        // On an object with toString returning a primitive
        let mut obj2 = JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        };
        let to_string_fn = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|this, _| JSValue::string("hello")),
        );
        obj2.properties
            .insert("toString".to_string(), to_string_fn.clone());
        let obj2_val = JSValue::Object(Rc::new(RefCell::new(obj2)));
        let prim2 = obj2_val.to_primitive("string").unwrap();
        assert!(prim2.strict_eq(&JSValue::string("hello")));

        // On a function -> returns self
        let func = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
        let prim3 = func.to_primitive("number").unwrap();
        assert!(prim3.strict_eq(&func));

        // On a primitive -> returns self
        let prim4 = JSValue::int(100).to_primitive("number").unwrap();
        assert_eq!(prim4, JSValue::int(100));

        let prim5 = JSValue::string("test").to_primitive("string").unwrap();
        assert!(prim5.strict_eq(&JSValue::string("test")));
    }

    #[test]
    fn test_hash_string() {
        // Deterministic: same input gives same hash
        let h1 = JSValue::hash_string("hello");
        let h2 = JSValue::hash_string("hello");
        assert_eq!(h1, h2);

        // Different strings give different hashes (very likely)
        let h3 = JSValue::hash_string("world");
        assert_ne!(h1, h3);

        // Empty string
        let h_empty = JSValue::hash_string("");
        let h_empty2 = JSValue::hash_string("");
        assert_eq!(h_empty, h_empty2);

        // Single character strings should differ
        let h_a = JSValue::hash_string("a");
        let h_b = JSValue::hash_string("b");
        assert_ne!(h_a, h_b);
    }

    #[test]
    fn test_is_truthy_falsy() {
        // Verify is_truthy and is_falsy match to_boolean
        let values: Vec<JSValue> = vec![
            JSValue::Undefined,
            JSValue::Null,
            JSValue::bool(false),
            JSValue::bool(true),
            JSValue::int(0),
            JSValue::int(1),
            JSValue::int(-1),
            JSValue::float(0.0),
            JSValue::float(1.0),
            JSValue::float(f64::NAN),
            JSValue::string(""),
            JSValue::string("hello"),
            JSValue::object("Object"),
            JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined())),
            JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: None, id: 1 }))),
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "0".to_string() }))),
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "1".to_string() }))),
        ];

        for val in &values {
            assert_eq!(val.is_truthy(), val.to_boolean(), "is_truthy mismatch for {:?}", val);
            assert_eq!(val.is_falsy(), !val.to_boolean(), "is_falsy mismatch for {:?}", val);
            assert_eq!(val.is_truthy(), !val.is_falsy(), "truthy/falsy inconsistency for {:?}", val);
        }
    }

    #[test]
    fn test_is_number_is_string_is_object_is_callable() {
        assert!(JSValue::int(5).is_number());
        assert!(JSValue::float(5.0).is_number());
        assert!(!JSValue::string("5").is_number());
        assert!(!JSValue::bool(true).is_number());

        assert!(JSValue::string("hi").is_string());
        assert!(!JSValue::int(5).is_string());
        assert!(!JSValue::object("Object").is_string());

        assert!(JSValue::object("Object").is_object());
        assert!(JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined())).is_object());
        assert!(!JSValue::int(5).is_object());
        assert!(!JSValue::string("hi").is_object());
        assert!(!JSValue::bool(true).is_object());

        assert!(JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined())).is_callable());
        assert!(!JSValue::object("Object").is_callable());
        assert!(!JSValue::int(5).is_callable());
    }

    #[test]
    fn test_type_of_all_variants() {
        assert_eq!(JSValue::Undefined.type_of(), "undefined");
        assert_eq!(JSValue::Null.type_of(), "object");
        assert_eq!(JSValue::Bool(false).type_of(), "boolean");
        assert_eq!(JSValue::Int(0).type_of(), "number");
        assert_eq!(JSValue::Float(0.0).type_of(), "number");
        assert_eq!(JSValue::String(Rc::new(RefCell::new(JSString { data: String::new(), hash: 0 }))).type_of(), "string");
        assert_eq!(
            JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: None, id: 0 }))).type_of(),
            "symbol"
        );
        assert_eq!(
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "0".to_string() }))).type_of(),
            "bigint"
        );
        assert_eq!(JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }))).type_of(), "object");
        assert_eq!(
            JSValue::Function(Rc::new(RefCell::new(JSFunction {
                name: None,
                params: vec![],
                body: FunctionBody::Native(|_, _| JSValue::undefined()),
                closure: HashMap::new(),
                is_constructor: false,
                is_async: false,
                is_generator: false,
            }))).type_of(),
            "function"
        );
    }

    #[test]
    fn test_abstract_eq_more_paths() {
        // BigInt == String (numeric string matches)
        let bi = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })));
        let s = JSValue::string("42");
        assert!(bi.abstract_eq(&s));
        assert!(s.abstract_eq(&bi));

        // BigInt == Number: to_number() for BigInt returns NaN, so NaN==NaN is false
        let bi2 = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "100".to_string() })));
        let num = JSValue::Int(100);
        // Both to_number() return NaN, NaN == NaN is false
        assert!(!bi2.abstract_eq(&num));
        assert!(!num.abstract_eq(&bi2));

        // BigInt == String that doesn't parse -> false
        let bi3 = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })));
        let bad_str = JSValue::string("abc");
        assert!(!bi3.abstract_eq(&bad_str));

        // Object == Primitive (with toPrimitive returning number)
        let mut obj = JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        };
        let value_of_fn = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|this, _| JSValue::int(10)),
        );
        obj.properties.insert("valueOf".to_string(), value_of_fn);
        let obj_val = JSValue::Object(Rc::new(RefCell::new(obj)));
        assert!(obj_val.abstract_eq(&JSValue::Int(10)));

        // Bool coercion: false == 0
        assert!(JSValue::Bool(false).abstract_eq(&JSValue::Int(0)));
        // Bool coercion: true == 2 -> false
        assert!(!JSValue::Bool(true).abstract_eq(&JSValue::Int(2)));
        // Bool on right side
        assert!(JSValue::Int(1).abstract_eq(&JSValue::Bool(true)));
        assert!(JSValue::Int(0).abstract_eq(&JSValue::Bool(false)));

        // Different types that don't match
        assert!(!JSValue::Int(1).abstract_eq(&JSValue::Bool(false)));
    }

    #[test]
    fn test_strict_eq_more() {
        // Int/Float cross comparison
        assert!(JSValue::Int(5).strict_eq(&JSValue::Float(5.0)));
        assert!(JSValue::Float(5.0).strict_eq(&JSValue::Int(5)));
        assert!(!JSValue::Int(5).strict_eq(&JSValue::Float(5.1)));

        // String content equality (different Rc allocations)
        let s1 = JSValue::string("hello");
        let s2 = JSValue::string("hello");
        // Note: PartialEq for String uses ptr_eq, but strict_eq uses data comparison
        assert!(s1.strict_eq(&s2));

        // BigInt equality
        let bi1 = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })));
        let bi2 = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })));
        let bi3 = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "99".to_string() })));
        assert!(bi1.strict_eq(&bi2));
        assert!(!bi1.strict_eq(&bi3));

        // Symbol id equality
        let sym1 = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: Some("a".to_string()), id: 1 })));
        let sym2 = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: Some("b".to_string()), id: 1 })));
        let sym3 = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: Some("a".to_string()), id: 2 })));
        assert!(sym1.strict_eq(&sym2)); // same id, different description
        assert!(!sym1.strict_eq(&sym3)); // different id

        // Cross-type returns false
        assert!(!JSValue::Int(0).strict_eq(&JSValue::Null));
        assert!(!JSValue::Int(0).strict_eq(&JSValue::Bool(false)));
        assert!(!JSValue::Int(1).strict_eq(&JSValue::Bool(true)));
        assert!(!JSValue::string("1").strict_eq(&JSValue::Int(1)));
        assert!(!JSValue::Undefined.strict_eq(&JSValue::Null));
    }

    #[test]
    fn test_define_getter_setter() {
        // Define getter and verify get_property calls it
        let obj = JSValue::object("Object");
        let getter = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|_this, _args| JSValue::int(99)),
        );
        obj.define_getter("x", getter);
        let result = obj.get_property("x");
        assert_eq!(result, Some(JSValue::int(99)));

        // Define setter and verify set_property routes through the setter descriptor.
        // The setter path stores via descriptor; the value should NOT appear in the
        // plain properties map (because the setter dispatch runs instead).
        // We use a Native setter that does nothing (no-op) to confirm the path is taken.
        let setter = JSValue::function(
            None,
            vec!["val".to_string()],
            FunctionBody::Native(|_this, _args| JSValue::undefined()),
        );
        obj.define_setter("y", setter);
        obj.set_property("y", JSValue::int(42));
        // The plain properties map should NOT contain "y" -- the setter intercepts.
        {
            let borrow = match &obj {
                JSValue::Object(o) => o.borrow(),
                _ => panic!(),
            };
            assert!(
                !borrow.properties.contains_key("y"),
                "value should be intercepted by setter, not stored in properties"
            );
        }

        // Also verify define_setter preserves an existing getter
        let getter2 = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|_this, _args| JSValue::string("from_getter")),
        );
        obj.define_getter("z", getter2);
        let setter2 = JSValue::function(
            None,
            vec!["val".to_string()],
            FunctionBody::Native(|_this, _args| JSValue::undefined()),
        );
        obj.define_setter("z", setter2);
        // getter should still work (compare by string content, not Rc pointer)
        let z_val = obj.get_property("z").unwrap();
        assert_eq!(z_val.to_string(), "from_getter");
    }

    #[test]
    fn test_set_property_function() {
        // Set property on a Function value (goes into closure map)
        let func = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|_, _| JSValue::undefined()),
        );
        func.set_property("myVar", JSValue::int(123));
        assert!(func.has_property("myVar"));
        assert_eq!(func.get_property("myVar"), Some(JSValue::int(123)));
    }

    #[test]
    fn test_has_property_on_function() {
        let func = JSValue::function(
            None,
            vec![],
            FunctionBody::Native(|_, _| JSValue::undefined()),
        );
        assert!(!func.has_property("x"));
        func.set_property("x", JSValue::int(1));
        assert!(func.has_property("x"));
        assert!(!func.has_property("y"));

        // has_property on non-object/function returns false
        assert!(!JSValue::int(5).has_property("x"));
        assert!(!JSValue::string("hi").has_property("length")); // not handled by has_property
    }

    #[test]
    fn test_get_property_string() {
        let s = JSValue::string("hello");

        // "length" returns length
        assert_eq!(s.get_property("length"), Some(JSValue::Int(5)));

        // Index returns character
        let c0 = s.get_property("0").unwrap();
        assert!(c0.strict_eq(&JSValue::string("h")));
        let c1 = s.get_property("1").unwrap();
        assert!(c1.strict_eq(&JSValue::string("e")));
        let c4 = s.get_property("4").unwrap();
        assert!(c4.strict_eq(&JSValue::string("o")));

        // Out of range returns None
        assert_eq!(s.get_property("5"), None);
        assert_eq!(s.get_property("100"), None);

        // Non-numeric property returns None
        assert_eq!(s.get_property("foo"), None);

        // Empty string length
        assert_eq!(JSValue::string("").get_property("length"), Some(JSValue::Int(0)));
    }

    #[test]
    fn test_get_property_function() {
        let func = JSValue::function(
            Some("myFunc"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            FunctionBody::Native(|_, _| JSValue::undefined()),
        );

        // "length" returns param count
        assert_eq!(func.get_property("length"), Some(JSValue::Int(3)));

        // "name" returns function name
        let name = func.get_property("name").unwrap();
        assert!(name.strict_eq(&JSValue::string("myFunc")));

        // Anonymous function name
        let anon = JSValue::function(None, vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
        let anon_name = anon.get_property("name").unwrap();
        assert!(anon_name.strict_eq(&JSValue::string("")));

        // Closure variable access
        func.set_property("closureVar", JSValue::string("captured"));
        let closure_val = func.get_property("closureVar").unwrap();
        assert!(closure_val.strict_eq(&JSValue::string("captured")));

        // Non-existent property returns None
        assert_eq!(func.get_property("nonexistent"), None);
    }

    #[test]
    fn test_prototype_cycle_detection() {
        // Create circular prototype chain: A.__proto__ = B, B.__proto__ = A
        let obj_a = JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        })));
        let obj_b = JSValue::Object(Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        })));

        // Set A.__proto__ = B
        if let (JSValue::Object(a), JSValue::Object(b)) = (&obj_a, &obj_b) {
            a.borrow_mut().prototype = Some(b.clone());
            b.borrow_mut().prototype = Some(a.clone());
        }

        // get_property should not infinite loop, should return None
        let result = obj_a.get_property("nonexistent");
        assert_eq!(result, None);
    }

    #[test]
    fn test_partial_eq() {
        // Same type
        assert_eq!(JSValue::Int(5), JSValue::Int(5));
        assert_ne!(JSValue::Int(5), JSValue::Int(6));

        // Int/Float cross
        assert_eq!(JSValue::Int(5), JSValue::Float(5.0));
        assert_eq!(JSValue::Float(5.0), JSValue::Int(5));
        assert_ne!(JSValue::Int(5), JSValue::Float(5.1));

        // Float/Float
        assert_eq!(JSValue::Float(1.5), JSValue::Float(1.5));
        assert_ne!(JSValue::Float(1.5), JSValue::Float(2.5));

        // NaN != NaN
        assert_ne!(JSValue::Float(f64::NAN), JSValue::Float(f64::NAN));

        // Bool
        assert_eq!(JSValue::Bool(true), JSValue::Bool(true));
        assert_ne!(JSValue::Bool(true), JSValue::Bool(false));

        // Undefined/Null
        assert_eq!(JSValue::Undefined, JSValue::Undefined);
        assert_eq!(JSValue::Null, JSValue::Null);
        assert_ne!(JSValue::Undefined, JSValue::Null);

        // Cross-type always false
        assert_ne!(JSValue::Int(0), JSValue::Bool(false));
        assert_ne!(JSValue::Int(0), JSValue::Null);
        assert_ne!(JSValue::Int(1), JSValue::Bool(true));

        // BigInt value equality
        assert_eq!(
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() }))),
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })))
        );
        assert_ne!(
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() }))),
            JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "99".to_string() })))
        );
    }

    #[test]
    fn test_debug_format() {
        assert_eq!(format!("{:?}", JSValue::Undefined), "undefined");
        assert_eq!(format!("{:?}", JSValue::Null), "null");
        assert_eq!(format!("{:?}", JSValue::Bool(true)), "true");
        assert_eq!(format!("{:?}", JSValue::Bool(false)), "false");
        assert_eq!(format!("{:?}", JSValue::Int(42)), "42");
        assert_eq!(format!("{:?}", JSValue::Float(3.14)), "3.14");
        assert_eq!(format!("{:?}", JSValue::string("hello")), "\"hello\"");

        let obj = JSValue::object("Array");
        let debug = format!("{:?}", obj);
        assert!(debug.contains("object Array"));

        let func = JSValue::function(Some("fn"), vec![], FunctionBody::Native(|_, _| JSValue::undefined()));
        let debug = format!("{:?}", func);
        assert!(debug.contains("function fn()"));

        let sym = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: Some("desc".to_string()), id: 1 })));
        assert_eq!(format!("{:?}", sym), "Symbol(desc)");

        let sym_no_desc = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: None, id: 2 })));
        assert_eq!(format!("{:?}", sym_no_desc), "Symbol()");

        let bi = JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "123".to_string() })));
        assert_eq!(format!("{:?}", bi), "123n");
    }

    #[test]
    fn test_display_format() {
        assert_eq!(format!("{}", JSValue::Undefined), "undefined");
        assert_eq!(format!("{}", JSValue::Null), "null");
        assert_eq!(format!("{}", JSValue::Bool(true)), "true");
        assert_eq!(format!("{}", JSValue::Int(42)), "42");
        assert_eq!(format!("{}", JSValue::float(1.5)), "1.5");
        assert_eq!(format!("{}", JSValue::string("hello")), "hello");
        assert_eq!(
            format!("{}", JSValue::BigInt(Rc::new(RefCell::new(JSBigInt { value: "42".to_string() })))),
            "42n"
        );
        assert_eq!(
            format!("{}", JSValue::function(Some("f"), vec![], FunctionBody::Native(|_, _| JSValue::undefined()))),
            "function f() { [native code] }"
        );
        let sym = JSValue::Symbol(Rc::new(RefCell::new(JSSymbol { description: Some("x".to_string()), id: 1 })));
        assert_eq!(format!("{}", sym), "Symbol(x)");
    }

    #[test]
    fn test_bytecode_function_new() {
        let bf = BytecodeFunction::new();
        assert_eq!(bf.name, None);
        assert!(bf.params.is_empty());
        assert!(bf.bytecode.is_empty());
        assert!(bf.constants.is_empty());
        assert!(bf.variables.is_empty());
        assert!(bf.functions.is_empty());
        assert!(bf.line_numbers.is_empty());
        assert_eq!(bf.filename, None);
        assert!(!bf.is_generator);
        assert!(!bf.is_async);
        assert!(!bf.is_arrow);
        assert!(!bf.is_module);
        assert!(!bf.strict_mode);
        assert_eq!(bf.rest_param_index, None);
        assert!(bf.closure_vars.is_empty());
    }

    #[test]
    fn test_bytecode_function_line_number() {
        let bf = BytecodeFunction {
            name: Some("test".to_string()),
            params: vec![],
            bytecode: vec![],
            constants: vec![],
            variables: vec![],
            functions: vec![],
            line_numbers: vec![(0, 10), (5, 20), (10, 30)],
            filename: None,
            is_generator: false,
            is_async: false,
            is_arrow: false,
            is_module: false,
            strict_mode: false,
            rest_param_index: None,
            closure_vars: vec![],
        };

        // PC 0 -> line 10
        assert_eq!(bf.line_number(0), Some(10));
        // PC 3 -> line 10 (first entry with pc <= 3 is (0, 10))
        assert_eq!(bf.line_number(3), Some(10));
        // PC 5 -> line 20
        assert_eq!(bf.line_number(5), Some(20));
        // PC 7 -> line 20 (first entry with pc <= 7 is (5, 20))
        assert_eq!(bf.line_number(7), Some(20));
        // PC 10 -> line 30
        assert_eq!(bf.line_number(10), Some(30));
        // PC 100 -> line 30 (last entry applies)
        assert_eq!(bf.line_number(100), Some(30));

        // Empty line_numbers -> None
        let empty_bf = BytecodeFunction::new();
        assert_eq!(empty_bf.line_number(0), None);
    }

    #[test]
    fn test_function_body_debug() {
        let native_body = FunctionBody::Native(|_, _| JSValue::undefined());
        let debug = format!("{:?}", native_body);
        assert_eq!(debug, "Native(..)");

        let closure_body = FunctionBody::Closure(Rc::new(|_, _| JSValue::undefined()));
        let debug = format!("{:?}", closure_body);
        assert_eq!(debug, "Closure(..)");

        let bc_body = FunctionBody::Bytecode(BytecodeFunction::new());
        let debug = format!("{:?}", bc_body);
        assert!(debug.starts_with("Bytecode("));

        let source_body = FunctionBody::Source("return 42;".to_string());
        let debug = format!("{:?}", source_body);
        assert_eq!(debug, "Source(\"return 42;\")");

        let gen_body = FunctionBody::Generator {
            func: BytecodeFunction::new(),
            yields: Rc::new(RefCell::new(vec![])),
        };
        let debug = format!("{:?}", gen_body);
        assert_eq!(debug, "Generator(..)");

        let gen_next_body = FunctionBody::GeneratorNext {
            state: Rc::new(RefCell::new(GeneratorState {
                func: BytecodeFunction::new(),
                closure: HashMap::new(),
                args: vec![],
                this: JSValue::undefined(),
                saved_frame: None,
                saved_pc: 0,
                saved_locals: vec![],
                saved_stack: vec![],
                saved_closure: vec![],
                done: false,
                started: false,
            })),
        };
        let debug = format!("{:?}", gen_next_body);
        assert_eq!(debug, "GeneratorNext(..)");
    }
}

