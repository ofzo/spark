//! Spark Core - Pure Rust JavaScript Engine
//!
//! A zero-dependency, zero-unsafe JavaScript engine written entirely in Rust.
//! Designed for embedding: the engine defines interfaces, the host provides implementations.
//!
//! # Architecture
//!
//! - **Lexer** → **Parser** (Pratt) → **Compiler** → **Bytecode** → **Trampoline Interpreter**
//! - Host traits: `Clock`, `Output`, `AsyncExecutor`, `ModuleLoader`
//! - JS-implemented builtins for array callbacks and Promise methods

#![allow(unused, dead_code, missing_docs)]

pub mod value;
pub mod runtime;
pub mod context;
pub mod parser;
pub mod lexer;
pub mod compiler;
pub mod interpreter;
pub mod builtins;
pub mod gc;
pub mod host;
