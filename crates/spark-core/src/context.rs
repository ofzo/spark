#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! JavaScript execution context.
//!
//! Represents a JavaScript execution context with its own scope chain,
//! variable environment, and this binding.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::runtime::JSRuntime;
use crate::value::{JSValue, JSObject};

/// Variable environment (scope).
#[derive(Debug, Clone)]
pub struct VarEnv {
    /// Variables in this scope
    pub variables: HashMap<String, JSValue>,
    /// Parent scope (for closure capture)
    pub parent: Option<Box<VarEnv>>,
}

impl VarEnv {
    /// Create a new empty variable environment.
    pub fn new() -> Self {
        VarEnv {
            variables: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new variable environment with a parent.
    pub fn with_parent(parent: VarEnv) -> Self {
        VarEnv {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Get a variable value.
    pub fn get(&self, name: &str) -> Option<&JSValue> {
        if let Some(val) = self.variables.get(name) {
            Some(val)
        } else if let Some(ref parent) = self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    /// Set a variable value.
    pub fn set(&mut self, name: &str, value: JSValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Check if a variable exists in this scope (not parent).
    pub fn has_own(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

/// JavaScript execution context.
pub struct JSContext {
    /// Runtime reference
    pub runtime: Rc<RefCell<JSRuntime>>,
    /// Global object
    pub global: Rc<RefCell<JSObject>>,
    /// Current variable environment
    pub var_env: VarEnv,
    /// This binding
    pub this: JSValue,
    /// Exception state
    pub exception: Option<JSValue>,
    /// Whether we're in strict mode
    pub strict_mode: bool,
}

impl JSContext {
    /// Create a new context with the given runtime.
    pub fn new(runtime: Rc<RefCell<JSRuntime>>) -> Self {
        let global = runtime.borrow().global.clone();
        JSContext {
            runtime,
            global,
            var_env: VarEnv::new(),
            this: JSValue::undefined(),
            exception: None,
            strict_mode: false,
        }
    }

    /// Get a variable value.
    pub fn get_var(&self, name: &str) -> JSValue {
        if let Some(val) = self.var_env.get(name) {
            val.clone()
        } else {
            // Check global object
            if let Some(val) = self.global.borrow().properties.get(name) {
                val.clone()
            } else {
                JSValue::undefined()
            }
        }
    }

    /// Set a variable value.
    pub fn set_var(&mut self, name: &str, value: JSValue) {
        if self.var_env.has_own(name) {
            self.var_env.set(name, value);
        } else {
            // Set on global object
            self.global.borrow_mut().properties.insert(name.to_string(), value);
        }
    }

    /// Declare a variable (let/const/var).
    pub fn declare_var(&mut self, name: &str, value: JSValue) {
        self.var_env.set(name, value);
    }

    /// Throw an exception.
    pub fn throw(&mut self, value: JSValue) {
        self.exception = Some(value);
    }

    /// Get and clear the current exception.
    pub fn take_exception(&mut self) -> Option<JSValue> {
        self.exception.take()
    }

    /// Check if there's an exception.
    pub fn has_exception(&self) -> bool {
        self.exception.is_some()
    }

    /// Create a new scope (for block scoping).
    pub fn push_scope(&mut self) {
        let parent = std::mem::replace(&mut self.var_env, VarEnv::new());
        self.var_env.parent = Some(Box::new(parent));
    }

    /// Pop the current scope.
    pub fn pop_scope(&mut self) {
        if let Some(parent) = self.var_env.parent.take() {
            self.var_env = *parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::JSRuntime;

    #[test]
    fn test_context_creation() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        assert!(!ctx.has_exception());
    }

    #[test]
    fn test_variable_operations() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);

        ctx.declare_var("x", JSValue::int(42));
        assert_eq!(ctx.get_var("x").to_int32(), 42);

        ctx.set_var("x", JSValue::int(100));
        assert_eq!(ctx.get_var("x").to_int32(), 100);
    }

    #[test]
    fn test_scope_operations() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);

        ctx.declare_var("x", JSValue::int(1));
        ctx.push_scope();
        ctx.declare_var("y", JSValue::int(2));

        assert_eq!(ctx.get_var("x").to_int32(), 1);
        assert_eq!(ctx.get_var("y").to_int32(), 2);

        ctx.pop_scope();
        assert_eq!(ctx.get_var("x").to_int32(), 1);
        assert!(ctx.get_var("y").is_undefined());
    }

    #[test]
    fn test_exception_handling() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);

        assert!(!ctx.has_exception());
        ctx.throw(JSValue::string("error"));
        assert!(ctx.has_exception());

        let exc = ctx.take_exception();
        assert!(exc.is_some());
        assert_eq!(exc.unwrap().to_string(), "error");
        assert!(!ctx.has_exception());
    }

    #[test]
    fn test_get_var_undefined() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let val = ctx.get_var("undeclared_var_xyz");
        assert!(val.is_undefined());
    }

    #[test]
    fn test_set_var_global() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        // Variable not in var_env, so set_var should set on global object
        ctx.set_var("globalProp", JSValue::int(99));
        assert_eq!(ctx.get_var("globalProp").to_int32(), 99);
        // Verify it's on the global object directly
        assert!(ctx.global.borrow().properties.contains_key("globalProp"));
    }

    #[test]
    fn test_get_var_from_global() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        // Declare a property on the global object directly
        ctx.global
            .borrow_mut()
            .properties
            .insert("fromGlobal".to_string(), JSValue::string("hello"));
        let val = ctx.get_var("fromGlobal");
        assert_eq!(val.to_string(), "hello");
    }

    #[test]
    fn test_pop_scope_no_parent() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        // Should not panic when popping the root scope
        ctx.pop_scope();
        // Context should still be usable
        assert!(!ctx.has_exception());
    }

    #[test]
    fn test_has_own() {
        let mut env = VarEnv::new();
        assert!(!env.has_own("foo"));
        env.set("foo", JSValue::int(1));
        assert!(env.has_own("foo"));
        assert!(!env.has_own("bar"));
    }

    #[test]
    fn test_var_env_with_parent() {
        let mut parent_env = VarEnv::new();
        parent_env.set("parent_var", JSValue::int(10));
        let child_env = VarEnv::with_parent(parent_env);
        // child should find parent's variable via scope chain
        assert_eq!(child_env.get("parent_var").unwrap().to_int32(), 10);
        // child should not find its own nonexistent var
        assert!(child_env.get("missing").is_none());
    }

    #[test]
    fn test_var_env_get_nonexistent() {
        let env = VarEnv::new();
        assert!(env.get("anything").is_none());
    }

    #[test]
    fn test_context_strict_mode() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);
        assert!(!ctx.strict_mode);
        ctx.strict_mode = true;
        assert!(ctx.strict_mode);
    }

    #[test]
    fn test_context_this_binding() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        assert!(ctx.this.is_undefined());
    }

    #[test]
    fn test_multiple_scopes() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let mut ctx = JSContext::new(rt);

        // Root scope
        ctx.declare_var("root", JSValue::int(1));

        // Push first child scope
        ctx.push_scope();
        ctx.declare_var("child1", JSValue::int(2));
        assert_eq!(ctx.get_var("root").to_int32(), 1);
        assert_eq!(ctx.get_var("child1").to_int32(), 2);

        // Push second child scope
        ctx.push_scope();
        ctx.declare_var("child2", JSValue::int(3));
        assert_eq!(ctx.get_var("root").to_int32(), 1);
        assert_eq!(ctx.get_var("child1").to_int32(), 2);
        assert_eq!(ctx.get_var("child2").to_int32(), 3);

        // Pop back to first child scope
        ctx.pop_scope();
        assert_eq!(ctx.get_var("root").to_int32(), 1);
        assert_eq!(ctx.get_var("child1").to_int32(), 2);
        assert!(ctx.get_var("child2").is_undefined());

        // Pop back to root scope
        ctx.pop_scope();
        assert_eq!(ctx.get_var("root").to_int32(), 1);
        assert!(ctx.get_var("child1").is_undefined());
        assert!(ctx.get_var("child2").is_undefined());
    }
}
