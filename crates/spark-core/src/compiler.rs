#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! JavaScript bytecode compiler.
//!
//! Compiles JavaScript AST into bytecode for the interpreter.

use crate::parser::{
    ASTNode, AssignmentOp, BinaryOp, CatchClause, ClassElement, ClassElementKind, ExportSpecifier,
    FunctionParam, ImportSpecifier, PropertyDefinition, PropertyKind, SwitchCase, UnaryOp,
    VariableDeclarator, VariableKind as ASTVariableKind,
};
pub use crate::value::{
    BytecodeFunction, Constant, Opcode, Variable, VariableKind,
};

/// Compile error.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
}

impl CompileError {
    pub fn new(message: &str) -> Self {
        CompileError {
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CompileError: {}", self.message)
    }
}

impl std::error::Error for CompileError {}

/// Scope for tracking variables.
#[derive(Debug, Clone)]
struct Scope {
    /// Variables in this scope (name -> index in BytecodeFunction.variables)
    variables: Vec<u32>,
    /// Parent scope index
    parent: Option<usize>,
    /// Whether this is a function scope
    is_function_scope: bool,
}

/// Break/continue label targets.
#[derive(Debug, Clone)]
struct LoopTargets {
    break_pc: Option<u32>,
    continue_pc: Option<u32>,
}

/// Bytecode compiler.
pub struct Compiler {
    /// Current bytecode function being built
    function: BytecodeFunction,
    /// Variable name -> index mapping for current scope
    var_map: Vec<std::collections::HashMap<String, u32>>,
    /// Break targets stack
    loop_stack: Vec<LoopTargets>,
    /// Try/catch return PC for rethrow
    try_depth: usize,
    /// Source filename
    filename: Option<String>,
    /// Whether in strict mode
    strict_mode: bool,
    /// Outer scope variable names (for closure capture)
    outer_scope_vars: Vec<String>,
    /// Closure variable indices for the current function
    closure_var_map: std::collections::HashMap<String, u32>,
}

impl Compiler {
    /// Create a new compiler.
    pub fn new() -> Self {
        Compiler {
            function: BytecodeFunction::new(),
            var_map: vec![std::collections::HashMap::new()],
            loop_stack: Vec::new(),
            try_depth: 0,
            filename: None,
            strict_mode: false,
            outer_scope_vars: Vec::new(),
            closure_var_map: std::collections::HashMap::new(),
        }
    }

    /// Set the source filename for error reporting.
    pub fn set_filename(&mut self, filename: String) {
        self.filename = Some(filename.clone());
        self.function.filename = Some(filename);
    }

    /// Set strict mode.
    pub fn set_strict(&mut self, strict: bool) {
        self.strict_mode = strict;
        self.function.strict_mode = strict;
    }

    /// Compile an AST node into a bytecode function.
    pub fn compile(&mut self, node: &ASTNode) -> Result<(), CompileError> {
        match node {
            ASTNode::Block(stmts) => {
                self.compile_block_inner(stmts, true)
            }
            _ => self.compile_statement_inner(node, true),
        }
    }

    /// Compile a block of statements, optionally keeping the last result.
    fn compile_block_inner(&mut self, stmts: &[ASTNode], keep_last: bool) -> Result<(), CompileError> {
        let len = stmts.len();
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == len - 1;
            self.compile_statement_inner(stmt, is_last && keep_last)?;
        }
        Ok(())
    }

    /// Get the compiled bytecode function.
    pub fn into_function(self) -> BytecodeFunction {
        self.function
    }

    /// Compile a single statement.
    fn compile_statement(&mut self, node: &ASTNode) -> Result<(), CompileError> {
        self.compile_statement_inner(node, false)
    }

    /// Compile a single statement with option to keep result on stack.
    fn compile_statement_inner(&mut self, node: &ASTNode, keep_result: bool) -> Result<(), CompileError> {
        self.add_line_number(node);
        match node {
            ASTNode::ExpressionStatement(expr) => {
                self.compile_expr(expr)?;
                // Pop the expression result if not keeping it
                if !keep_result {
                    self.emit(Opcode::Pop);
                }
                Ok(())
            }
            ASTNode::Block(stmts) => {
                self.compile_block_inner(stmts, keep_result)
            }
            ASTNode::VariableDeclaration {
                kind,
                declarations,
            } => self.compile_var_decl(kind, declarations),
            ASTNode::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } => self.compile_func_decl(name, params, body, *is_async, *is_generator),
            ASTNode::Return(expr) => {
                if let Some(val) = expr {
                    self.compile_expr(val)?;
                } else {
                    self.emit(Opcode::PushUndefined);
                }
                self.emit(Opcode::Return);
                Ok(())
            }
            ASTNode::If {
                test,
                consequent,
                alternate,
            } => self.compile_if(test, consequent, alternate.as_deref()),
            ASTNode::While { test, body } => self.compile_while(test, body),
            ASTNode::DoWhile { body, test } => self.compile_do_while(body, test),
            ASTNode::For {
                init,
                test,
                update,
                body,
            } => self.compile_for(init.as_deref(), test.as_deref(), update.as_deref(), body),
            ASTNode::ForIn {
                left,
                right,
                body,
            } => self.compile_for_in(left, right, body),
            ASTNode::ForOf {
                left,
                right,
                body,
                is_await,
            } => self.compile_for_of(left, right, body, *is_await),
            ASTNode::Break(label) => self.compile_break(label),
            ASTNode::Continue(label) => self.compile_continue(label),
            ASTNode::Switch {
                discriminant,
                cases,
            } => self.compile_switch(discriminant, cases),
            ASTNode::Throw(expr) => {
                self.compile_expr(expr)?;
                self.emit(Opcode::Throw);
                Ok(())
            }
            ASTNode::Try {
                block,
                catch,
                finally,
            } => self.compile_try(block, catch.as_ref(), finally.as_deref()),
            ASTNode::EmptyStatement => Ok(()),
            ASTNode::DebuggerStatement => Ok(()),
            ASTNode::ImportDeclaration {
                specifiers,
                source,
            } => self.compile_import(specifiers, source),
            ASTNode::ExportDeclaration {
                declaration,
                specifiers,
                source,
            } => self.compile_export(declaration.as_deref(), specifiers, source.as_deref()),
            ASTNode::ClassDeclaration {
                name,
                super_class,
                body,
            } => self.compile_class_decl(name, super_class.as_deref(), body),
            ASTNode::LabeledStatement { label, body } => {
                // Labels on statements: just compile the body
                self.compile_statement(body)
            }
            _ => {
                // Treat as expression statement
                self.compile_expr(node)?;
                self.emit(Opcode::Pop);
                Ok(())
            }
        }
    }

    /// Compile an expression.
    fn compile_expr(&mut self, node: &ASTNode) -> Result<(), CompileError> {
        match node {
            // Literals
            ASTNode::NumberLiteral(val) => {
                if *val as i32 as f64 == *val && *val >= i32::MIN as f64 && *val <= i32::MAX as f64 {
                    self.emit(Opcode::PushInt(*val as i32));
                } else {
                    let idx = self.add_constant(Constant::Float(*val));
                    self.emit(Opcode::PushFloat(idx));
                }
                Ok(())
            }
            ASTNode::StringLiteral(val) => {
                let idx = self.add_constant(Constant::String(val.clone()));
                self.emit(Opcode::PushString(idx));
                Ok(())
            }
            ASTNode::BoolLiteral(val) => {
                self.emit(Opcode::PushBool(*val));
                Ok(())
            }
            ASTNode::NullLiteral => {
                self.emit(Opcode::PushNull);
                Ok(())
            }
            ASTNode::UndefinedLiteral => {
                self.emit(Opcode::PushUndefined);
                Ok(())
            }
            ASTNode::RegExpLiteral { pattern, flags } => {
                let p_idx = self.add_constant(Constant::String(pattern.clone()));
                let f_idx = self.add_constant(Constant::String(flags.clone()));
                self.emit(Opcode::PushRegExp(p_idx, f_idx));
                Ok(())
            }

            // Identifier
            ASTNode::Identifier(name) => {
                if name == "undefined" {
                    self.emit(Opcode::PushUndefined);
                } else if name == "NaN" {
                    let idx = self.add_constant(Constant::Float(f64::NAN));
                    self.emit(Opcode::PushFloat(idx));
                } else if name == "Infinity" {
                    let idx = self.add_constant(Constant::Float(f64::INFINITY));
                    self.emit(Opcode::PushFloat(idx));
                } else if name == "this" {
                    self.emit(Opcode::This);
                } else if let Some(var_idx) = self.resolve_local_variable(name) {
                    // Local variable (including parameters) - use GetVar
                    self.emit(Opcode::GetVar(var_idx));
                } else if let Some(&closure_idx) = self.closure_var_map.get(name) {
                    // Variable from outer scope - use GetClosure
                    self.emit(Opcode::GetClosure(closure_idx));
                } else if let Some(var_idx) = self.resolve_variable_opt(name) {
                    // Variable in outer scope of current function
                    self.emit(Opcode::GetVar(var_idx));
                } else {
                    // Look up on global object
                    self.emit(Opcode::GetGlobal);
                    let idx = self.add_constant(Constant::String(name.to_string()));
                    self.emit(Opcode::GetPropertyByName(idx));
                }
                Ok(())
            }

            // Binary operations
            ASTNode::BinaryOp { op, left, right } => {
                self.compile_binary_op(op, left, right)
            }

            // Unary operations
            ASTNode::UnaryOp {
                op,
                operand,
                prefix,
            } => self.compile_unary_op(op, operand, *prefix),

            // Assignment
            ASTNode::Assignment {
                op,
                left,
                right,
            } => self.compile_assignment(op, left, right),

            // Call
            ASTNode::Call { callee, args } => self.compile_call(callee, args),
            ASTNode::SuperCall(args) => self.compile_super_call(args),

            // New
            ASTNode::New { callee, args } => self.compile_new(callee, args),

            // Member access
            ASTNode::Member {
                object,
                property,
                computed,
            } => self.compile_member(object, property, *computed),

            // Conditional (ternary)
            ASTNode::Conditional {
                test,
                consequent,
                alternate,
            } => {
                self.compile_expr(test)?;
                let else_label = self.emit(Opcode::JumpIfFalse(0));
                self.emit(Opcode::Pop); // Pop test value (condition was true)
                self.compile_expr(consequent)?;
                let done_label = self.emit(Opcode::Jump(0));
                self.patch_jump(else_label);
                self.emit(Opcode::Pop); // Pop test value (condition was false)
                self.compile_expr(alternate)?;
                self.patch_jump(done_label);
                Ok(())
            }

            // Array expression
            ASTNode::ArrayExpression(elements) => {
                // Use a simpler approach: build elements list, then create array
                let mut has_spread = false;
                for elem in elements {
                    if matches!(elem, ASTNode::SpreadElement(_)) {
                        has_spread = true;
                        break;
                    }
                }

                if has_spread {
                    // For spread: create array with runtime index tracking
                    self.emit(Opcode::CreateArray);

                    // Runtime index variable for position in result array
                    let out_idx_name = format!("__outi_{}", self.function.bytecode.len());
                    let out_idx = self.declare_variable(&out_idx_name, VariableKind::Var);
                    self.emit(Opcode::PushInt(0));
                    self.emit(Opcode::SetVar(out_idx));
                    self.emit(Opcode::Pop);

                    for elem in elements {
                        if let ASTNode::SpreadElement(inner) = elem {
                            self.compile_expr(inner)?;
                            let spread_name = format!("__sp_{}", self.function.bytecode.len());
                            let spread_var = self.declare_variable(&spread_name, VariableKind::Var);
                            self.emit(Opcode::SetVar(spread_var));
                            self.emit(Opcode::Pop);

                            let idx_name = format!("__si_{}", self.function.bytecode.len());
                            let idx_var = self.declare_variable(&idx_name, VariableKind::Var);
                            self.emit(Opcode::PushInt(0));
                            self.emit(Opcode::SetVar(idx_var));
                            self.emit(Opcode::Pop);

                            let loop_start = self.function.bytecode.len() as u32;
                            self.emit(Opcode::GetVar(idx_var));
                            self.emit(Opcode::GetVar(spread_var));
                            let len_k = self.add_constant(Constant::String("length".to_string()));
                            self.emit(Opcode::GetPropertyByName(len_k));
                            self.emit(Opcode::Lt);
                            let loop_end = self.emit(Opcode::JumpIfFalse(0));
                            self.emit(Opcode::Pop);

                            // arr[out_idx] = spread[idx_var]
                            self.emit(Opcode::Dup); // dup arr
                            self.emit(Opcode::GetVar(out_idx)); // push out_idx as property key
                            self.emit(Opcode::GetVar(spread_var));
                            self.emit(Opcode::GetVar(idx_var));
                            self.emit(Opcode::GetProperty); // spread[idx_var]
                            self.emit(Opcode::SetProperty); // arr[out_idx] = value
                            self.emit(Opcode::Pop);

                            // out_idx++
                            self.emit(Opcode::GetVar(out_idx));
                            self.emit(Opcode::Inc);
                            self.emit(Opcode::SetVar(out_idx));
                            self.emit(Opcode::Pop);
                            // idx_var++
                            self.emit(Opcode::GetVar(idx_var));
                            self.emit(Opcode::Inc);
                            self.emit(Opcode::SetVar(idx_var));
                            self.emit(Opcode::Pop);

                            self.emit(Opcode::Jump(loop_start));
                            self.patch_jump(loop_end);
                            self.emit(Opcode::Pop);
                        } else {
                            // arr[out_idx] = elem
                            self.emit(Opcode::Dup);
                            self.emit(Opcode::GetVar(out_idx));
                            self.compile_expr(elem)?;
                            self.emit(Opcode::SetProperty);
                            self.emit(Opcode::Pop);
                            // out_idx++
                            self.emit(Opcode::GetVar(out_idx));
                            self.emit(Opcode::Inc);
                            self.emit(Opcode::SetVar(out_idx));
                            self.emit(Opcode::Pop);
                        }
                    }
                    // Set length
                    self.emit(Opcode::Dup);
                    let len_k = self.add_constant(Constant::String("length".to_string()));
                    self.emit(Opcode::GetVar(out_idx));
                    self.emit(Opcode::SetPropertyByName(len_k));
                    self.emit(Opcode::Pop);
                } else {
                    // No spread: simple approach
                    self.emit(Opcode::CreateArray);
                    let len = elements.len();
                    for (i, elem) in elements.iter().enumerate() {
                        self.emit(Opcode::Dup);
                        self.compile_expr(elem)?;
                        let idx = self.add_constant(Constant::String(i.to_string()));
                        self.emit(Opcode::SetPropertyByName(idx));
                        self.emit(Opcode::Pop);
                    }
                    self.emit(Opcode::Dup);
                    let len_idx = self.add_constant(Constant::String("length".to_string()));
                    self.emit(Opcode::PushInt(len as i32));
                    self.emit(Opcode::SetPropertyByName(len_idx));
                    self.emit(Opcode::Pop);
                }
                Ok(())
            }

            // Object expression
            ASTNode::ObjectExpression(props) => {
                self.emit(Opcode::CreateObject);
                for prop in props {
                    self.compile_property_def(prop)?;
                }
                Ok(())
            }

            // Arrow function
            ASTNode::ArrowFunctionExpression {
                params,
                body,
                is_async,
            } => self.compile_arrow_function(params, body, *is_async),

            // Function expression
            ASTNode::FunctionDeclaration {
                name,
                params,
                body,
                is_async,
                is_generator,
            } => self.compile_func_expr(&Some(name.clone()), params, body, *is_async, *is_generator),

            // Sequence
            ASTNode::Sequence { expressions } => {
                for (i, expr) in expressions.iter().enumerate() {
                    self.compile_expr(expr)?;
                    if i < expressions.len() - 1 {
                        self.emit(Opcode::Pop);
                    }
                }
                Ok(())
            }

            // Spread
            ASTNode::SpreadElement(inner) => {
                self.compile_expr(inner)
            }

            // Template literal (simplified: just produce the cooked string)
            ASTNode::TemplateLiteral { quasis, expressions } => {
                // Compile template literal by concatenating parts
                // Push the first quasi string
                if !quasis.is_empty() {
                    let idx = self.add_constant(Constant::String(quasis[0].cooked.clone()));
                    self.emit(Opcode::PushString(idx));
                }

                for (i, expr) in expressions.iter().enumerate() {
                    // Compile the expression
                    self.compile_expr(expr)?;
                    // Convert to string using Add (which does string concatenation)
                    self.emit(Opcode::Add);

                    // Push the next quasi string
                    if i + 1 < quasis.len() {
                        let idx = self.add_constant(Constant::String(quasis[i + 1].cooked.clone()));
                        self.emit(Opcode::PushString(idx));
                        self.emit(Opcode::Add);
                    }
                }

                // If no expressions, push empty string
                if expressions.is_empty() && quasis.is_empty() {
                    let idx = self.add_constant(Constant::String(String::new()));
                    self.emit(Opcode::PushString(idx));
                }

                Ok(())
            }

            // Await expression
            ASTNode::AwaitExpression(inner) => {
                self.compile_expr(inner)?;
                self.emit(Opcode::Await);
                Ok(())
            }

            // Yield expression (simplified)
            ASTNode::YieldExpression { argument, delegate } => {
                if let Some(arg) = argument {
                    self.compile_expr(arg)?;
                } else {
                    self.emit(Opcode::PushUndefined);
                }
                self.emit(Opcode::Yield);
                Ok(())
            }

            _ => Err(CompileError::new(&format!(
                "Cannot compile expression: {:?}",
                std::mem::discriminant(node)
            ))),
        }
    }

    // ================================================================
    // Variable declarations
    // ================================================================

    /// Compile a recursive destructuring pattern.
    /// Stack has the source value on top. After compilation, the source is consumed.
    fn compile_destructuring_pattern(
        &mut self,
        pattern: &ASTNode,
        var_kind: &VariableKind,
    ) -> Result<(), CompileError> {
        match pattern {
            ASTNode::ObjectExpression(props) => {
                // Object destructuring: {a, b, c: {d}}
                for prop in props {
                    let key_name = match prop.key.as_ref() {
                        ASTNode::Identifier(name) => name.clone(),
                        ASTNode::StringLiteral(s) => s.clone(),
                        _ => continue,
                    };
                    self.emit(Opcode::Dup);
                    let prop_idx = self.add_constant(Constant::String(key_name.clone()));
                    self.emit(Opcode::GetPropertyByName(prop_idx));

                    if let Some(value) = &prop.value {
                        // Nested pattern: {a: {b}} or {a: [x, y]}
                        match value.as_ref() {
                            ASTNode::ObjectExpression(_) | ASTNode::ArrayExpression(_) => {
                                self.compile_destructuring_pattern(value, var_kind)?;
                            }
                            ASTNode::Identifier(name) => {
                                let var_idx = self.declare_variable(name, *var_kind);
                                self.emit(Opcode::SetVar(var_idx));
                                self.emit(Opcode::Pop);
                            }
                            _ => { self.emit(Opcode::Pop); }
                        }
                    } else {
                        // Shorthand: {a} means {a: a}
                        let var_idx = self.declare_variable(&key_name, *var_kind);
                        self.emit(Opcode::SetVar(var_idx));
                        self.emit(Opcode::Pop);
                    }
                }
                self.emit(Opcode::Pop); // pop source object
            }
            ASTNode::ArrayExpression(elems) => {
                // Array destructuring: [a, [b, c], d]
                for (i, elem) in elems.iter().enumerate() {
                    let idx = self.add_constant(Constant::String(i.to_string()));
                    self.emit(Opcode::Dup);
                    self.emit(Opcode::GetPropertyByName(idx));

                    match elem {
                        ASTNode::ObjectExpression(_) | ASTNode::ArrayExpression(_) => {
                            self.compile_destructuring_pattern(elem, var_kind)?;
                        }
                        ASTNode::Identifier(name) => {
                            let var_idx = self.declare_variable(name, *var_kind);
                            self.emit(Opcode::SetVar(var_idx));
                            self.emit(Opcode::Pop);
                        }
                        _ => { self.emit(Opcode::Pop); }
                    }
                }
                self.emit(Opcode::Pop); // pop source array
            }
            ASTNode::Identifier(name) => {
                let var_idx = self.declare_variable(name, *var_kind);
                self.emit(Opcode::SetVar(var_idx));
                self.emit(Opcode::Pop);
            }
            _ => {
                self.emit(Opcode::Pop);
            }
        }
        Ok(())
    }

    fn compile_var_decl(
        &mut self,
        kind: &ASTVariableKind,
        decls: &[VariableDeclarator],
    ) -> Result<(), CompileError> {
        let var_kind = match kind {
            ASTVariableKind::Var => VariableKind::Var,
            ASTVariableKind::Let => VariableKind::Let,
            ASTVariableKind::Const => VariableKind::Const,
        };

        for decl in decls {
            if let Some(ref pattern) = decl.pattern {
                if let Some(init) = &decl.init {
                    self.compile_expr(init)?;
                    self.compile_destructuring_pattern(pattern, &var_kind)?;
                }
            } else {
                // Normal variable declaration
                let var_idx = self.declare_variable(&decl.name, var_kind);
                if let Some(init) = &decl.init {
                    self.compile_expr(init)?;
                    self.emit(Opcode::SetVar(var_idx));
                    self.emit(Opcode::Pop);
                }
            }
        }
        Ok(())
    }

    // ================================================================
    // Function declarations
    // ================================================================

    fn compile_func_decl(
        &mut self,
        name: &str,
        params: &[FunctionParam],
        body: &ASTNode,
        is_async: bool,
        is_generator: bool,
    ) -> Result<(), CompileError> {
        let func_idx = self.function.functions.len() as u32;

        // Collect outer scope variable names for closure capture.
        // Include both locally declared variables AND variables captured from
        // outer scopes, so they propagate to further nested functions.
        let mut outer_vars: Vec<String> = self.var_map.iter()
            .flat_map(|scope| scope.keys().cloned())
            .chain(self.closure_var_map.keys().cloned())
            .collect();
        outer_vars.sort();
        outer_vars.dedup();

        // Mark outer variables as captured
        for var_name in &outer_vars {
            for var in &mut self.function.variables {
                if &var.name == var_name {
                    var.is_captured = true;
                }
            }
        }

        // Create a nested compiler for the function body
        let mut inner = Compiler::new();
        inner.function.name = Some(name.to_string());
        inner.function.is_async = is_async;
        inner.function.is_generator = is_generator;
        inner.function.filename = self.filename.clone();
        inner.strict_mode = self.strict_mode;
        inner.function.strict_mode = self.strict_mode;

        // Set up closure variable mapping for the inner compiler
        for (i, name) in outer_vars.iter().enumerate() {
            inner.closure_var_map.insert(name.clone(), i as u32);
        }

        // Add parameters as variables
        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for (pi, param) in params.iter().enumerate() {
            if param.is_rest {
                inner.function.rest_param_index = Some(pi);
            }
            let idx = inner.declare_parameter(&param.name);
            if !param.is_rest {
                if let Some(default_val) = &param.default {
                    let arg_idx = inner.function.variables.len() as u32 - 1;
                    inner.emit(Opcode::GetVar(arg_idx));
                    inner.emit(Opcode::PushUndefined);
                    inner.emit(Opcode::StrictEq);
                    let else_label = inner.emit(Opcode::JumpIfFalse(0));
                    inner.compile_expr(default_val)?;
                    inner.emit(Opcode::SetVar(idx));
                    inner.emit(Opcode::Pop);
                    inner.patch_jump(else_label);
                }
            }
        }

        // Declare 'arguments' variable for regular functions (not arrow)
        inner.declare_variable("arguments", VariableKind::Var);

        // Compile the function body
        inner.compile(body)?;
        // Ensure the function returns undefined if no explicit return
        if let Some(Opcode::Return) = inner.function.bytecode.last() {
            // Already has a return
        } else {
            inner.emit(Opcode::PushUndefined);
            inner.emit(Opcode::Return);
        }

        inner.function.params = param_names;
        inner.finalize_closure();
        self.function.functions.push(inner.function);

        // Create closure from the nested function
        if is_generator {
            self.emit(Opcode::CreateGenerator(func_idx));
        } else {
            self.emit(Opcode::CreateClosure(func_idx));
        }

        // If this is a named function declaration, set the variable
        if !name.is_empty() {
            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
            self.emit(Opcode::Dup);
            self.emit(Opcode::SetVar(var_idx));
            self.emit(Opcode::Pop);
        }

        Ok(())
    }

    fn compile_func_expr(
        &mut self,
        name: &Option<String>,
        params: &[FunctionParam],
        body: &ASTNode,
        is_async: bool,
        is_generator: bool,
    ) -> Result<(), CompileError> {
        let func_name = name.clone().unwrap_or_default();
        let func_idx = self.function.functions.len() as u32;

        // Collect outer scope variable names for closure capture.
        // Include both locally declared variables AND variables captured from
        // outer scopes, so they propagate to further nested functions.
        let mut outer_vars: Vec<String> = self.var_map.iter()
            .flat_map(|scope| scope.keys().cloned())
            .chain(self.closure_var_map.keys().cloned())
            .collect();
        outer_vars.sort();
        outer_vars.dedup();

        // Mark outer variables as captured
        for var_name in &outer_vars {
            for var in &mut self.function.variables {
                if &var.name == var_name {
                    var.is_captured = true;
                }
            }
        }

        let mut inner = Compiler::new();
        inner.function.name = if func_name.is_empty() {
            None
        } else {
            Some(func_name.clone())
        };
        inner.function.is_async = is_async;
        inner.function.is_generator = is_generator;
        inner.function.filename = self.filename.clone();
        inner.strict_mode = self.strict_mode;
        inner.function.strict_mode = self.strict_mode;

        // Set up closure variable mapping for the inner compiler
        for (i, name) in outer_vars.iter().enumerate() {
            inner.closure_var_map.insert(name.clone(), i as u32);
        }

        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for param in params {
            let _idx = inner.declare_parameter(&param.name);
        }

        inner.compile(body)?;
        if !matches!(inner.function.bytecode.last(), Some(Opcode::Return)) {
            inner.emit(Opcode::PushUndefined);
            inner.emit(Opcode::Return);
        }

        inner.function.params = param_names;
        inner.finalize_closure();
        self.function.functions.push(inner.function);
        if is_generator {
            self.emit(Opcode::CreateGenerator(func_idx));
        } else {
            self.emit(Opcode::CreateClosure(func_idx));
        }
        Ok(())
    }

    fn compile_arrow_function(
        &mut self,
        params: &[FunctionParam],
        body: &ASTNode,
        is_async: bool,
    ) -> Result<(), CompileError> {
        let func_idx = self.function.functions.len() as u32;

        // Collect outer scope variable names for closure capture.
        // Include both locally declared variables AND variables captured from
        // outer scopes, so they propagate to further nested functions.
        let mut outer_vars: Vec<String> = self.var_map.iter()
            .flat_map(|scope| scope.keys().cloned())
            .chain(self.closure_var_map.keys().cloned())
            .collect();
        outer_vars.sort();
        outer_vars.dedup();

        // Mark outer variables as captured
        for var_name in &outer_vars {
            for var in &mut self.function.variables {
                if &var.name == var_name {
                    var.is_captured = true;
                }
            }
        }

        let mut inner = Compiler::new();
        inner.function.name = Some("arrow".to_string());
        inner.function.is_async = is_async;
        inner.function.is_arrow = true;
        inner.function.filename = self.filename.clone();
        inner.strict_mode = self.strict_mode;
        inner.function.strict_mode = self.strict_mode;

        // Set up closure variable mapping for the inner compiler
        for (i, name) in outer_vars.iter().enumerate() {
            inner.closure_var_map.insert(name.clone(), i as u32);
        }

        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for param in params {
            let _idx = inner.declare_parameter(&param.name);
        }

        // Arrow function body
        match body {
            ASTNode::Block(_) => {
                inner.compile(body)?;
                if !matches!(inner.function.bytecode.last(), Some(Opcode::Return)) {
                    inner.emit(Opcode::PushUndefined);
                    inner.emit(Opcode::Return);
                }
            }
            _ => {
                let expr_body = match body {
                    ASTNode::ExpressionStatement(e) => e.as_ref(),
                    other => other,
                };
                inner.compile_expr(expr_body)?;
                inner.emit(Opcode::Return);
            }
        }

        inner.function.params = param_names;
        inner.finalize_closure();
        self.function.functions.push(inner.function);
        self.emit(Opcode::CreateClosure(func_idx));
        Ok(())
    }

    // ================================================================
    // Control flow
    // ================================================================

    fn compile_if(
        &mut self,
        test: &ASTNode,
        consequent: &ASTNode,
        alternate: Option<&ASTNode>,
    ) -> Result<(), CompileError> {
        self.compile_expr(test)?;
        if let Some(alt) = alternate {
            let else_label = self.emit(Opcode::JumpIfFalse(0));
            self.compile_statement(consequent)?;
            let done_label = self.emit(Opcode::Jump(0));
            self.patch_jump(else_label);
            self.compile_statement(alt)?;
            self.patch_jump(done_label);
        } else {
            let done_label = self.emit(Opcode::JumpIfFalse(0));
            self.compile_statement(consequent)?;
            self.patch_jump(done_label);
        }
        Ok(())
    }

    fn compile_while(
        &mut self,
        test: &ASTNode,
        body: &ASTNode,
    ) -> Result<(), CompileError> {
        let loop_start = self.function.bytecode.len() as u32;
        self.compile_expr(test)?;
        let exit_label = self.emit(Opcode::JumpIfFalse(0));
        self.loop_stack.push(LoopTargets {
            break_pc: Some(exit_label),
            continue_pc: Some(loop_start),
        });
        self.compile_statement(body)?;
        self.emit(Opcode::Jump(loop_start));
        self.patch_jump(exit_label);
        self.loop_stack.pop();
        Ok(())
    }

    fn compile_do_while(
        &mut self,
        body: &ASTNode,
        test: &ASTNode,
    ) -> Result<(), CompileError> {
        let loop_start = self.function.bytecode.len() as u32;
        self.loop_stack.push(LoopTargets {
            break_pc: None, // Will be patched
            continue_pc: Some(loop_start),
        });
        self.compile_statement(body)?;
        let continue_target = self.function.bytecode.len() as u32;
        self.compile_expr(test)?;
        let exit_label = self.emit(Opcode::JumpIfFalse(0));
        self.emit(Opcode::Jump(loop_start));
        self.patch_jump(exit_label);
        // Patch break targets
        if let Some(target) = self.loop_stack.last_mut().unwrap().break_pc.take() {
            // Break was emitted before we knew the exit PC
            // Actually, break_pc is set when we emit break. We need to set it before the loop.
            // Fix: set break_pc to exit_label before pushing
        }
        self.loop_stack.pop();
        Ok(())
    }

    fn compile_for(
        &mut self,
        init: Option<&ASTNode>,
        test: Option<&ASTNode>,
        update: Option<&ASTNode>,
        body: &ASTNode,
    ) -> Result<(), CompileError> {
        // Init
        if let Some(init_node) = init {
            match init_node {
                ASTNode::VariableDeclaration {
                    kind,
                    declarations,
                } => {
                    self.compile_var_decl(kind, declarations)?;
                }
                _ => {
                    self.compile_expr(init_node)?;
                    self.emit(Opcode::Pop);
                }
            }
        }

        let loop_start = self.function.bytecode.len() as u32;

        // Test
        if let Some(test_node) = test {
            self.compile_expr(test_node)?;
            let exit_label = self.emit(Opcode::JumpIfFalse(0));

            self.loop_stack.push(LoopTargets {
                break_pc: Some(exit_label),
                continue_pc: None, // Update will be the continue target
            });
            self.compile_statement(body)?;
            // Continue target: the update expression
            if let Some(update_node) = update {
                self.compile_expr(update_node)?;
                self.emit(Opcode::Pop);
            }
            self.emit(Opcode::Jump(loop_start));
            self.patch_jump(exit_label);
            self.loop_stack.pop();
        } else {
            // Infinite loop
            self.loop_stack.push(LoopTargets {
                break_pc: None,
                continue_pc: None,
            });
            self.compile_statement(body)?;
            if let Some(update_node) = update {
                self.compile_expr(update_node)?;
                self.emit(Opcode::Pop);
            }
            self.emit(Opcode::Jump(loop_start));
            // Break target will need to be patched
            self.loop_stack.pop();
        }

        Ok(())
    }

    fn compile_for_in(
        &mut self,
        left: &ASTNode,
        right: &ASTNode,
        body: &ASTNode,
    ) -> Result<(), CompileError> {
        // for-in: iterate over object keys using index-based loop
        // var __keys = Object.keys(obj);
        // for (var __i = 0; __i < __keys.length; __i++) { var key = __keys[__i]; body }

        // Get Object.keys(right)
        self.emit(Opcode::GetGlobal);
        let object_idx = self.add_constant(Constant::String("Object".to_string()));
        self.emit(Opcode::GetPropertyByName(object_idx));
        let keys_idx = self.add_constant(Constant::String("keys".to_string()));
        self.emit(Opcode::GetPropertyByName(keys_idx));
        // Stack: [keys_fn]
        self.compile_expr(right)?;
        // Stack: [keys_fn, obj]
        self.emit(Opcode::Call(1));
        // Stack: [keys_array]

        // Store keys in temp variable
        let keys_name = format!("__keys_{}", self.function.bytecode.len());
        let keys_var = self.declare_variable(&keys_name, VariableKind::Var);
        self.emit(Opcode::SetVar(keys_var));
        self.emit(Opcode::Pop);

        // Index variable
        let idx_name = format!("__fi_{}", self.function.bytecode.len());
        let idx_var = self.declare_variable(&idx_name, VariableKind::Var);
        self.emit(Opcode::PushInt(0));
        self.emit(Opcode::SetVar(idx_var));
        self.emit(Opcode::Pop);

        // Loop: __i < __keys.length
        let loop_start = self.function.bytecode.len() as u32;
        self.emit(Opcode::GetVar(idx_var));
        self.emit(Opcode::GetVar(keys_var));
        let len_idx = self.add_constant(Constant::String("length".to_string()));
        self.emit(Opcode::GetPropertyByName(len_idx));
        self.emit(Opcode::Lt);
        let exit_label = self.emit(Opcode::JumpIfFalse(0));
        self.emit(Opcode::Pop); // pop condition

        // Get keys[__i]
        self.emit(Opcode::GetVar(keys_var));
        self.emit(Opcode::GetVar(idx_var));
        self.emit(Opcode::GetProperty);

        // Assign to loop variable
        match left {
            ASTNode::Identifier(name) => {
                let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                self.emit(Opcode::SetVar(var_idx));
                self.emit(Opcode::Pop);
            }
            _ => {
                self.emit(Opcode::Pop);
            }
        }

        self.loop_stack.push(LoopTargets {
            break_pc: Some(exit_label),
            continue_pc: Some(loop_start),
        });
        self.compile_statement(body)?;
        self.loop_stack.pop();

        // __i++
        self.emit(Opcode::GetVar(idx_var));
        self.emit(Opcode::Inc);
        self.emit(Opcode::SetVar(idx_var));
        self.emit(Opcode::Pop);

        self.emit(Opcode::Jump(loop_start));
        self.patch_jump(exit_label);
        self.emit(Opcode::Pop); // pop condition from jump
        Ok(())
    }

    fn compile_for_of(
        &mut self,
        left: &ASTNode,
        right: &ASTNode,
        body: &ASTNode,
        _is_await: bool,
    ) -> Result<(), CompileError> {
        // for-of: use iterator protocol
        // 1. Get iterator from iterable
        // 2. Loop: call next(), check done, assign value
        // 3. Execute body

        // Compile the iterable expression
        self.compile_expr(right)?;

        // Get iterator: calls iterable[Symbol.iterator]() or uses GetIterator
        self.emit(Opcode::GetIterator);

        // Store iterator in a temp variable
        let iter_name = format!("__iter_{}", self.function.bytecode.len());
        let iter_var = self.declare_variable(&iter_name, VariableKind::Var);
        self.emit(Opcode::SetVar(iter_var));
        self.emit(Opcode::Pop);

        // Loop start
        let loop_start = self.function.bytecode.len() as u32;

        // Call iterator.next()
        self.emit(Opcode::GetVar(iter_var));
        self.emit(Opcode::IteratorNext);

        // Stack: [result_obj]
        // Check done: result.done
        let done_idx = self.add_constant(Constant::String("done".to_string()));
        self.emit(Opcode::Dup); // [result_obj, result_obj]
        self.emit(Opcode::GetPropertyByName(done_idx)); // [result_obj, done]
        let exit_label = self.emit(Opcode::JumpIfTrue(0)); // if done, exit
        self.emit(Opcode::Pop); // pop done value

        // Get value: result.value
        let value_idx = self.add_constant(Constant::String("value".to_string()));
        self.emit(Opcode::GetPropertyByName(value_idx)); // [value]

        // Assign to loop variable
        match left {
            ASTNode::Identifier(name) => {
                let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                self.emit(Opcode::SetVar(var_idx));
                self.emit(Opcode::Pop);
            }
            ASTNode::VariableDeclaration { kind, declarations } => {
                if let Some(decl) = declarations.first() {
                    let var_idx = self.resolve_or_declare_variable(&decl.name, VariableKind::Var);
                    self.emit(Opcode::SetVar(var_idx));
                    self.emit(Opcode::Pop);
                } else {
                    self.emit(Opcode::Pop);
                }
            }
            _ => { self.emit(Opcode::Pop); }
        }

        self.loop_stack.push(LoopTargets {
            break_pc: Some(exit_label),
            continue_pc: Some(loop_start),
        });
        self.compile_statement(body)?;
        self.loop_stack.pop();

        // Jump back to loop start
        self.emit(Opcode::Jump(loop_start));

        // Exit: pop the result_obj and done value
        self.patch_jump(exit_label);
        self.emit(Opcode::Pop); // pop result_obj
        self.emit(Opcode::Pop); // pop done value (true)

        Ok(())
    }

    fn compile_break(&mut self, _label: &Option<String>) -> Result<(), CompileError> {
        if let Some(target) = self.loop_stack.last() {
            if let Some(break_pc) = target.break_pc {
                self.emit(Opcode::Jump(break_pc));
            }
        }
        Ok(())
    }

    fn compile_continue(&mut self, _label: &Option<String>) -> Result<(), CompileError> {
        if let Some(target) = self.loop_stack.last() {
            if let Some(continue_pc) = target.continue_pc {
                self.emit(Opcode::Jump(continue_pc));
            }
        }
        Ok(())
    }

    fn compile_switch(
        &mut self,
        discriminant: &ASTNode,
        cases: &[SwitchCase],
    ) -> Result<(), CompileError> {
        self.compile_expr(discriminant)?;

        let mut case_labels = Vec::new();
        let mut has_default = false;

        // First pass: create jump targets for each case
        for case in cases {
            if case.test.is_some() {
                case_labels.push(self.emit(Opcode::JumpIfFalse(0))); // placeholder
            } else {
                has_default = true;
                case_labels.push(0); // default
            }
        }

        // Actually we need a different approach: compare and jump
        // Let me use a simpler approach: fall-through matching
        let mut end_patches = Vec::new();
        let mut next_case_pc = self.function.bytecode.len() as u32;

        // Re-emit the discriminant is already on stack, let's use a comparison approach
        // Actually, let's use a cleaner approach:
        // For each case, test equality with the discriminant
        for (i, case) in cases.iter().enumerate() {
            if let Some(test_expr) = &case.test {
                self.emit(Opcode::Dup); // Duplicate discriminant
                self.compile_expr(test_expr)?;
                self.emit(Opcode::StrictEq);
                let body_start = self.emit(Opcode::JumpIfFalse(0));

                // Case body
                for stmt in &case.consequent {
                    self.compile_statement(stmt)?;
                }
                end_patches.push(self.emit(Opcode::Jump(0)));
                self.patch_jump(body_start);
            } else {
                // Default case: just compile the body
                for stmt in &case.consequent {
                    self.compile_statement(stmt)?;
                }
            }
        }

        // Pop the discriminant
        self.emit(Opcode::Pop);

        // Patch all end jumps
        for patch in end_patches {
            self.patch_jump(patch);
        }

        Ok(())
    }

    fn compile_try(
        &mut self,
        block: &ASTNode,
        catch: Option<&CatchClause>,
        finally: Option<&ASTNode>,
    ) -> Result<(), CompileError> {
        // Push handler pointing to catch block
        let handler_label = self.emit(Opcode::PushHandler(0));

        // Compile try block
        self.compile_statement(block)?;

        // Pop handler (no exception occurred)
        self.emit(Opcode::PopHandler);

        // Jump over catch block
        let end_label = self.emit(Opcode::Jump(0));

        // Patch handler to point here (catch block start)
        let catch_pc = self.function.bytecode.len() as u32;
        self.patch_jump_to(handler_label, catch_pc);

        if let Some(catch_clause) = catch {
            // Declare catch parameter if present
            if let Some(param) = &catch_clause.param {
                if let ASTNode::Identifier(name) = param.as_ref() {
                    let var_idx = self.declare_variable(name, VariableKind::Var);
                    // Store the exception value (on stack) into the catch variable
                    self.emit(Opcode::SetVar(var_idx));
                    self.emit(Opcode::Pop);
                }
            } else {
                // No catch parameter - pop the exception value from the stack
                self.emit(Opcode::Pop);
            }
            self.compile_statement(&catch_clause.body)?;
        }

        // Patch jump to skip catch block
        self.patch_jump(end_label);

        // Compile finally block if present
        if let Some(finally_block) = finally {
            self.compile_statement(finally_block)?;
        }

        Ok(())
    }

    // ================================================================
    // Expressions
    // ================================================================

    fn compile_binary_op(
        &mut self,
        op: &BinaryOp,
        left: &ASTNode,
        right: &ASTNode,
    ) -> Result<(), CompileError> {
        match op {
            // Short-circuit operators
            BinaryOp::And => {
                self.compile_expr(left)?;
                let right_label = self.emit(Opcode::JumpIfFalse(0));
                self.emit(Opcode::Pop);
                self.compile_expr(right)?;
                self.patch_jump(right_label);
                Ok(())
            }
            BinaryOp::Or => {
                self.compile_expr(left)?;
                let right_label = self.emit(Opcode::JumpIfTrue(0));
                self.emit(Opcode::Pop);
                self.compile_expr(right)?;
                self.patch_jump(right_label);
                Ok(())
            }
            // Regular binary operators
            _ => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                let opcode = match op {
                    BinaryOp::Add => Opcode::Add,
                    BinaryOp::Sub => Opcode::Sub,
                    BinaryOp::Mul => Opcode::Mul,
                    BinaryOp::Div => Opcode::Div,
                    BinaryOp::Mod => Opcode::Mod,
                    BinaryOp::Pow => Opcode::Pow,
                    BinaryOp::BitAnd => Opcode::BitAnd,
                    BinaryOp::BitOr => Opcode::BitOr,
                    BinaryOp::BitXor => Opcode::BitXor,
                    BinaryOp::Shl => Opcode::Shl,
                    BinaryOp::Shr => Opcode::Shr,
                    BinaryOp::UShr => Opcode::UShr,
                    BinaryOp::Eq => Opcode::Eq,
                    BinaryOp::Ne => Opcode::Ne,
                    BinaryOp::StrictEq => Opcode::StrictEq,
                    BinaryOp::StrictNe => Opcode::StrictNe,
                    BinaryOp::Lt => Opcode::Lt,
                    BinaryOp::Le => Opcode::Le,
                    BinaryOp::Gt => Opcode::Gt,
                    BinaryOp::Ge => Opcode::Ge,
                    BinaryOp::In => Opcode::In,
                    BinaryOp::Instanceof => Opcode::Instanceof,
                    BinaryOp::NullishCoalescing => {
                        // a ?? b: if a is null/undefined, return b; else return a
                        // Simplified: compile both and handle
                        // Actually need to be smarter about this
                        // TODO: implement proper nullish coalescing
                        Opcode::Add // placeholder
                    }
                    BinaryOp::And | BinaryOp::Or => unreachable!(),
                };
                self.emit(opcode);
                Ok(())
            }
        }
    }

    fn compile_unary_op(
        &mut self,
        op: &UnaryOp,
        operand: &ASTNode,
        prefix: bool,
    ) -> Result<(), CompileError> {
        match op {
            UnaryOp::Minus => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Neg);
                Ok(())
            }
            UnaryOp::Plus => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Plus);
                Ok(())
            }
            UnaryOp::Not => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Not);
                Ok(())
            }
            UnaryOp::BitNot => {
                self.compile_expr(operand)?;
                self.emit(Opcode::BitNot);
                Ok(())
            }
            UnaryOp::Typeof => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Typeof);
                Ok(())
            }
            UnaryOp::Void => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Void);
                Ok(())
            }
            UnaryOp::Delete => {
                self.compile_expr(operand)?;
                self.emit(Opcode::Delete);
                Ok(())
            }
            UnaryOp::Inc => {
                if prefix {
                    // ++x: increment then return new value
                    self.compile_expr(operand)?;
                    self.emit(Opcode::Inc);
                    self.emit(Opcode::Dup);
                    // Store back
                    if let ASTNode::Identifier(name) = operand {
                        if let Some(&closure_idx) = self.closure_var_map.get(name) {
                            self.emit(Opcode::SetClosure(closure_idx));
                        } else {
                            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                            self.emit(Opcode::SetVar(var_idx));
                        }
                    }
                    Ok(())
                } else {
                    // x++: return old value then increment
                    self.compile_expr(operand)?;
                    self.emit(Opcode::Dup);
                    self.emit(Opcode::Inc);
                    // Store back
                    if let ASTNode::Identifier(name) = operand {
                        if let Some(&closure_idx) = self.closure_var_map.get(name) {
                            self.emit(Opcode::SetClosure(closure_idx));
                        } else {
                            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                            self.emit(Opcode::SetVar(var_idx));
                        }
                    }
                    self.emit(Opcode::Pop); // Pop the stored value, keep original
                    Ok(())
                }
            }
            UnaryOp::Dec => {
                if prefix {
                    self.compile_expr(operand)?;
                    self.emit(Opcode::Dec);
                    self.emit(Opcode::Dup);
                    if let ASTNode::Identifier(name) = operand {
                        if let Some(&closure_idx) = self.closure_var_map.get(name) {
                            self.emit(Opcode::SetClosure(closure_idx));
                        } else {
                            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                            self.emit(Opcode::SetVar(var_idx));
                        }
                    }
                    Ok(())
                } else {
                    self.compile_expr(operand)?;
                    self.emit(Opcode::Dup);
                    self.emit(Opcode::Dec);
                    if let ASTNode::Identifier(name) = operand {
                        if let Some(&closure_idx) = self.closure_var_map.get(name) {
                            self.emit(Opcode::SetClosure(closure_idx));
                        } else {
                            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                            self.emit(Opcode::SetVar(var_idx));
                        }
                    }
                    self.emit(Opcode::Pop);
                    Ok(())
                }
            }
        }
    }

    fn compile_assignment(
        &mut self,
        op: &AssignmentOp,
        left: &ASTNode,
        right: &ASTNode,
    ) -> Result<(), CompileError> {
        match op {
            AssignmentOp::Assign => {
                // Simple assignment
                match left {
                    ASTNode::Identifier(name) => {
                        self.compile_expr(right)?;
                        // Check local variables first (including parameters), then closures
                        if let Some(var_idx) = self.resolve_local_variable(name) {
                            self.emit(Opcode::SetVar(var_idx));
                        } else if let Some(&closure_idx) = self.closure_var_map.get(name) {
                            self.emit(Opcode::SetClosure(closure_idx));
                        } else {
                            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
                            self.emit(Opcode::SetVar(var_idx));
                        }
                        // SetVar already pushes the value
                    }
                    ASTNode::Member {
                        object,
                        property,
                        computed,
                    } => {
                        // For assignment `obj.prop = value`:
                        // Result should be the value being assigned
                        //
                        // SetPropertyByName expects stack: [..., obj, val] (val on top)
                        // SetProperty expects stack: [..., obj, key, val] (val on top)
                        //
                        // Both pop val first, then obj (and key), and push val back.
                        //
                        // For non-computed (obj.x = val):
                        //   Push val → Dup → [val, val]
                        //   Push obj → [val, val, obj]
                        //   Rotate(3) → [val, obj, val]  (bottom-to-top rotation)
                        //   SetPropertyByName → pops val, pops obj, sets prop, pushes val → [val, val]
                        //
                        // For computed (obj[key] = val):
                        //   Push val → Dup → [val, val]
                        //   Push obj → [val, val, obj]
                        //   Push key → [val, val, obj, key]
                        //   Rotate(4) → [val, obj, key, val]
                        //   SetProperty → pops val, pops key, pops obj, sets prop, pushes val → [val, val]
                        self.compile_expr(right)?;
                        self.emit(Opcode::Dup);
                        self.compile_expr(object)?;
                        if *computed {
                            self.compile_expr(property)?;
                            self.emit(Opcode::Rotate(4));
                            self.emit(Opcode::SetProperty);
                        } else {
                            self.emit(Opcode::Rotate(3));
                            if let ASTNode::Identifier(prop_name) = property.as_ref() {
                                let idx = self.add_constant(Constant::String(prop_name.clone()));
                                self.emit(Opcode::SetPropertyByName(idx));
                            } else {
                                self.compile_expr(property)?;
                                self.emit(Opcode::SetProperty);
                            }
                        }
                    }
                    _ => {
                        self.compile_expr(right)?;
                        self.emit(Opcode::Pop);
                    }
                }
                Ok(())
            }
            AssignmentOp::AddAssign => {
                self.compile_compound_assign(op, left, right)
            }
            AssignmentOp::SubAssign => {
                self.compile_compound_assign(op, left, right)
            }
            AssignmentOp::MulAssign => {
                self.compile_compound_assign(op, left, right)
            }
            AssignmentOp::DivAssign => {
                self.compile_compound_assign(op, left, right)
            }
            AssignmentOp::ModAssign => {
                self.compile_compound_assign(op, left, right)
            }
            _ => {
                // Other compound assignments: simplified
                self.compile_expr(right)?;
                self.emit(Opcode::Pop);
                Ok(())
            }
        }
    }

    fn compile_compound_assign(
        &mut self,
        op: &AssignmentOp,
        left: &ASTNode,
        right: &ASTNode,
    ) -> Result<(), CompileError> {
        if let ASTNode::Identifier(name) = left {
            let var_idx = self.resolve_or_declare_variable(name, VariableKind::Var);
            self.emit(Opcode::GetVar(var_idx));
            self.compile_expr(right)?;
            let arith_op = match op {
                AssignmentOp::AddAssign => Opcode::Add,
                AssignmentOp::SubAssign => Opcode::Sub,
                AssignmentOp::MulAssign => Opcode::Mul,
                AssignmentOp::DivAssign => Opcode::Div,
                AssignmentOp::ModAssign => Opcode::Mod,
                _ => Opcode::Add,
            };
            self.emit(arith_op);
            self.emit(Opcode::SetVar(var_idx));
            // SetVar pushes the value already
        } else {
            self.compile_expr(right)?;
            self.emit(Opcode::Pop);
        }
        Ok(())
    }

    fn compile_call(
        &mut self,
        callee: &ASTNode,
        args: &[ASTNode],
    ) -> Result<(), CompileError> {
        // Check if this is a method call (obj.method())
        if let ASTNode::Member { object, property, computed } = callee {
            if !computed {
                if let ASTNode::Identifier(name) = property.as_ref() {
                    // Method call: compile object, then get field (keeping object on stack)
                    self.compile_expr(object)?;
                    let idx = self.add_constant(Constant::String(name.clone()));
                    self.emit(Opcode::GetField2(idx)); // pushes both object and method
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    self.emit(Opcode::CallMethod(args.len() as u32));
                    return Ok(());
                }
            }
        }
        // General function call
        self.compile_expr(callee)?;
        for arg in args {
            self.compile_expr(arg)?;
        }
        self.emit(Opcode::Call(args.len() as u32));
        Ok(())
    }

    fn compile_new(
        &mut self,
        callee: &ASTNode,
        args: &[ASTNode],
    ) -> Result<(), CompileError> {
        self.compile_expr(callee)?;
        for arg in args {
            self.compile_expr(arg)?;
        }
        self.emit(Opcode::New(args.len() as u32));
        Ok(())
    }

    fn compile_super_call(
        &mut self,
        args: &[ASTNode],
    ) -> Result<(), CompileError> {
        // Get the parent constructor from closure variable
        if let Some(&super_idx) = self.closure_var_map.get("__super_ctor__") {
            self.emit(Opcode::GetClosure(super_idx));
        } else {
            self.emit(Opcode::PushUndefined);
            return Ok(());
        }
        // Push this and swap: [this, parent_ctor]
        self.emit(Opcode::This);
        self.emit(Opcode::Swap);
        // Push arguments: [this, parent_ctor, arg0, arg1, ...]
        for arg in args {
            self.compile_expr(arg)?;
        }
        // CallMethod pops args, then func (parent_ctor), then this
        self.emit(Opcode::CallMethod(args.len() as u32));
        Ok(())
    }

    fn compile_member(
        &mut self,
        object: &ASTNode,
        property: &ASTNode,
        computed: bool,
    ) -> Result<(), CompileError> {
        self.compile_expr(object)?;
        if computed {
            self.compile_expr(property)?;
            self.emit(Opcode::GetProperty);
        } else if let ASTNode::Identifier(name) = property {
            let idx = self.add_constant(Constant::String(name.clone()));
            self.emit(Opcode::GetPropertyByName(idx));
        } else {
            self.compile_expr(property)?;
            self.emit(Opcode::GetProperty);
        }
        Ok(())
    }

    fn compile_property_def(&mut self, prop: &PropertyDefinition) -> Result<(), CompileError> {
        // Spread in object: copy all properties from source
        if let ASTNode::SpreadElement(ref inner) = *prop.key {
            // Stack: [target_obj]
            // Store target in temp variable
            let tmp_name = format!("__spread_tgt_{}", self.function.bytecode.len());
            let tmp_var = self.declare_variable(&tmp_name, VariableKind::Var);
            self.emit(Opcode::SetVar(tmp_var));
            self.emit(Opcode::Pop);

            // Store source in temp variable
            self.compile_expr(inner)?;
            let src_name = format!("__spread_src_{}", self.function.bytecode.len());
            let src_var = self.declare_variable(&src_name, VariableKind::Var);
            self.emit(Opcode::SetVar(src_var));
            self.emit(Opcode::Pop);

            // Call Object.assign(tmp_target, tmp_source)
            self.emit(Opcode::GetGlobal);
            let obj_idx = self.add_constant(Constant::String("Object".to_string()));
            self.emit(Opcode::GetPropertyByName(obj_idx));
            let assign_idx = self.add_constant(Constant::String("assign".to_string()));
            self.emit(Opcode::GetPropertyByName(assign_idx));
            // Stack: [assign_fn]
            self.emit(Opcode::GetVar(tmp_var));
            // Stack: [assign_fn, target]
            self.emit(Opcode::GetVar(src_var));
            // Stack: [assign_fn, target, source]
            self.emit(Opcode::Call(2));
            // Stack: [result]
            self.emit(Opcode::Pop);

            // Push target back on stack for further property additions
            self.emit(Opcode::GetVar(tmp_var));
            return Ok(());
        }

        // Get the object on the stack
        self.emit(Opcode::Dup);

        match *prop.key {
            ASTNode::Identifier(ref name) => {
                let idx = self.add_constant(Constant::String(name.clone()));
                if let Some(value) = &prop.value {
                    self.compile_expr(value)?;
                    self.emit(Opcode::SetPropertyByName(idx));
                    self.emit(Opcode::Pop); // Remove value, keep object
                } else {
                    // Shorthand: value is the identifier
                    let var_idx = self.resolve_variable(name);
                    self.emit(Opcode::GetVar(var_idx));
                    self.emit(Opcode::SetPropertyByName(idx));
                    self.emit(Opcode::Pop); // Remove value, keep object
                }
            }
            ASTNode::StringLiteral(ref s) => {
                let idx = self.add_constant(Constant::String(s.clone()));
                if let Some(value) = &prop.value {
                    self.compile_expr(value)?;
                    self.emit(Opcode::SetPropertyByName(idx));
                    self.emit(Opcode::Pop); // Remove value, keep object
                }
            }
            _ => {
                self.compile_expr(&prop.key)?;
                if let Some(value) = &prop.value {
                    self.compile_expr(value)?;
                    self.emit(Opcode::SetProperty);
                    self.emit(Opcode::Pop); // Remove value, keep object
                }
            }
        }
        Ok(())
    }

    fn compile_class_decl(
        &mut self,
        name: &str,
        super_class: Option<&ASTNode>,
        body: &[ClassElement],
    ) -> Result<(), CompileError> {
        // If extends, compile the parent class expression first
        let has_super = super_class.is_some();
        let mut super_var_idx: Option<u32> = None;
        if let Some(parent) = super_class {
            self.compile_expr(parent)?;
        }

        // Create the constructor function
        let ctor_idx = self.function.functions.len() as u32;
        let mut ctor_compiler = Compiler::new();
        ctor_compiler.function.name = Some(name.to_string());
        ctor_compiler.function.filename = self.filename.clone();
        ctor_compiler.strict_mode = self.strict_mode;

        // Set up closure variable mapping for the constructor
        // Map all variables from the class scope that will be captured
        if has_super {
            // __super_ctor__ will be at index 0 in the closure
            ctor_compiler.closure_var_map.insert("__super_ctor__".to_string(), 0);
        }

        // If extending, set up closure for super() calls
        if has_super {
            // Stack has parent constructor from compile_expr(super_class)
            // Store it in a local variable
            let super_var_name = "__super_ctor__";
            let idx = self.declare_variable(super_var_name, VariableKind::Var);
            super_var_idx = Some(idx);
            self.emit(Opcode::SetVar(idx));
            self.emit(Opcode::Pop);

            // Mark variables that will be captured by the constructor
            // Only variables that the constructor actually uses need to be captured
            // For now, mark __super_ctor__ as captured
            for var in &mut self.function.variables {
                if var.name == super_var_name {
                    var.is_captured = true;
                }
            }
        }

        // Find and compile the constructor body
        let mut has_constructor = false;
        for element in body {
            if let ASTNode::Identifier(key_name) = element.key.as_ref() {
                if key_name == "constructor" {
                    if let Some(value) = &element.value {
                        if let ASTNode::FunctionDeclaration { params, body: func_body, .. } = value.as_ref() {
                            has_constructor = true;
                            let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                            ctor_compiler.function.params = param_names;
                            for param in params {
                                let _ = ctor_compiler.declare_variable(&param.name, VariableKind::Var);
                            }
                            ctor_compiler.compile(func_body)?;
                        }
                    }
                }
            }
        }

        if !has_constructor {
            // Default constructor: call super() if extending, then return this
            if has_super {
                // TODO: auto-generate super() call
            }
            ctor_compiler.emit(Opcode::This);
            ctor_compiler.emit(Opcode::Return);
        }

        if !matches!(ctor_compiler.function.bytecode.last(), Some(Opcode::Return)) {
            ctor_compiler.emit(Opcode::This);
            ctor_compiler.emit(Opcode::Return);
        }

        ctor_compiler.finalize_closure();
        self.function.functions.push(ctor_compiler.function);

        // Declare the class variable and create the constructor closure
        let var_idx = self.declare_variable(name, VariableKind::Var);
        self.emit(Opcode::CreateClosure(ctor_idx));
        self.emit(Opcode::Dup);
        self.emit(Opcode::SetVar(var_idx));
        self.emit(Opcode::Pop); // pop SetVar result

        // Create prototype and add methods
        self.emit(Opcode::CreateObject); // prototype object

        for element in body {
            if let ASTNode::Identifier(key_name) = element.key.as_ref() {
                if key_name == "constructor" || element.is_static {
                    continue;
                }
                if let Some(value) = &element.value {
                    if let ASTNode::FunctionDeclaration { params, body: func_body, .. } = value.as_ref() {
                        let key_idx = self.add_constant(Constant::String(key_name.clone()));

                        let method_idx = self.function.functions.len() as u32;
                        let mut method_compiler = Compiler::new();
                        method_compiler.function.name = Some(key_name.clone());
                        method_compiler.function.filename = self.filename.clone();
                        method_compiler.strict_mode = self.strict_mode;

                        let mut outer_vars: Vec<String> = self.var_map.iter()
                            .flat_map(|scope| scope.keys().cloned())
                            .collect();
                        outer_vars.sort();
                        outer_vars.dedup();
                        for (i, vname) in outer_vars.iter().enumerate() {
                            method_compiler.closure_var_map.insert(vname.clone(), i as u32);
                        }
                        for var_name in &outer_vars {
                            for var in &mut self.function.variables {
                                if &var.name == var_name {
                                    var.is_captured = true;
                                }
                            }
                        }

                        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                        method_compiler.function.params = param_names;
                        for param in params {
                            let _ = method_compiler.declare_variable(&param.name, VariableKind::Var);
                        }
                        method_compiler.compile(func_body)?;
                        if !matches!(method_compiler.function.bytecode.last(), Some(Opcode::Return)) {
                            method_compiler.emit(Opcode::PushUndefined);
                            method_compiler.emit(Opcode::Return);
                        }
                        method_compiler.finalize_closure();
                        self.function.functions.push(method_compiler.function);

                        // Stack: [... ctor, prototype]
                        self.emit(Opcode::Dup); // [... ctor, prototype, prototype]
                        self.emit(Opcode::CreateClosure(method_idx));
                        self.emit(Opcode::SetPropertyByName(key_idx));
                        self.emit(Opcode::Pop);
                    }
                }
            }
        }

        // Set prototype on constructor: ctor.prototype = prototype
        let proto_key = self.add_constant(Constant::String("prototype".to_string()));
        self.emit(Opcode::SetPropertyByName(proto_key));
        self.emit(Opcode::Pop); // pop result

        // If extends, set up prototype chain using structural prototype
        if has_super {
            // Get Child constructor from variable
            self.emit(Opcode::GetVar(var_idx));
            let child_proto_key = self.add_constant(Constant::String("prototype".to_string()));
            self.emit(Opcode::GetPropertyByName(child_proto_key)); // get Child.prototype

            // Load parent class from local variable
            self.emit(Opcode::GetVar(super_var_idx.unwrap()));
            let parent_proto_key = self.add_constant(Constant::String("prototype".to_string()));
            self.emit(Opcode::GetPropertyByName(parent_proto_key)); // get Parent.prototype

            // SetProto: pops [proto, obj], sets obj.prototype = proto
            // Stack: [Child.prototype, Parent.prototype]
            // Pops Parent.prototype (proto), Child.prototype (obj)
            // Sets Child.prototype.prototype = Parent.prototype
            self.emit(Opcode::SetProto);
            self.emit(Opcode::Pop); // pop result

            // Copy static methods from parent to child constructor
            // We emit code to copy all properties from parent to child at runtime
            self.emit(Opcode::GetVar(var_idx)); // [Child]
            self.emit(Opcode::GetVar(super_var_idx.unwrap())); // [Child, Parent]
            self.emit(Opcode::CopyProperties); // copies Parent's properties to Child
            self.emit(Opcode::Pop); // pop result
        }

        // Add static methods and properties to the constructor
        // Stack at this point is empty - load constructor from variable
        for element in body {
            if !element.is_static {
                continue;
            }
            if let ASTNode::Identifier(key_name) = element.key.as_ref() {
                if let Some(value) = &element.value {
                    if let ASTNode::FunctionDeclaration { params, body: func_body, .. } = value.as_ref() {
                        // Compile static method as a closure
                        let key_idx = self.add_constant(Constant::String(key_name.clone()));
                        let method_idx = self.function.functions.len() as u32;
                        let mut method_compiler = Compiler::new();
                        method_compiler.function.name = Some(key_name.clone());
                        method_compiler.function.filename = self.filename.clone();
                        method_compiler.strict_mode = self.strict_mode;

                        let mut outer_vars: Vec<String> = self.var_map.iter()
                            .flat_map(|scope| scope.keys().cloned())
                            .collect();
                        outer_vars.sort();
                        outer_vars.dedup();
                        for (i, vname) in outer_vars.iter().enumerate() {
                            method_compiler.closure_var_map.insert(vname.clone(), i as u32);
                        }

                        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                        method_compiler.function.params = param_names;
                        for param in params {
                            let _ = method_compiler.declare_variable(&param.name, VariableKind::Var);
                        }
                        method_compiler.compile(func_body)?;
                        if !matches!(method_compiler.function.bytecode.last(), Some(Opcode::Return)) {
                            method_compiler.emit(Opcode::PushUndefined);
                            method_compiler.emit(Opcode::Return);
                        }
                        method_compiler.finalize_closure();
                        self.function.functions.push(method_compiler.function);

                        // Load constructor from variable, add static method
                        self.emit(Opcode::GetVar(var_idx)); // [ctor]
                        self.emit(Opcode::CreateClosure(method_idx)); // [ctor, method]
                        self.emit(Opcode::SetPropertyByName(key_idx)); // [method]
                        self.emit(Opcode::Pop); // []
                    } else {
                        // Static property (non-function)
                        let key_idx = self.add_constant(Constant::String(key_name.clone()));
                        self.emit(Opcode::GetVar(var_idx)); // [ctor]
                        self.compile_expr(value)?; // [ctor, value]
                        self.emit(Opcode::SetPropertyByName(key_idx)); // [value]
                        self.emit(Opcode::Pop); // []
                    }
                }
            }
        }

        Ok(())
    }

    // ================================================================
    // Import / Export
    // ================================================================

    fn compile_import(
        &mut self,
        specifiers: &[ImportSpecifier],
        source: &str,
    ) -> Result<(), CompileError> {
        // Emit Import opcode to load the module
        let idx = self.add_constant(Constant::String(source.to_string()));
        self.emit(Opcode::Import(idx));

        // Bind imported names to local variables
        // Stack starts with: [module_obj]
        // For each specifier: Dup module, GetProperty, SetVar, Pop (remove SetVar result)
        let has_namespace = specifiers.iter().any(|s| matches!(s, ImportSpecifier::Namespace { .. }));

        for spec in specifiers {
            // Duplicate the module object so it stays on the stack
            self.emit(Opcode::Dup);
            // Stack: [module_obj, module_obj]

            match spec {
                ImportSpecifier::Default { local } => {
                    let prop_idx = self.add_constant(Constant::String("default".to_string()));
                    self.emit(Opcode::GetPropertyByName(prop_idx));
                    // Stack: [module_obj, default_value]
                    let var_idx = self.declare_variable(local, VariableKind::Var);
                    self.emit(Opcode::SetVar(var_idx));
                    // Stack: [module_obj, default_value] (SetVar pushes back)
                    self.emit(Opcode::Pop);
                    // Stack: [module_obj]
                }
                ImportSpecifier::Named { imported, local } => {
                    let prop_idx = self.add_constant(Constant::String(imported.clone()));
                    self.emit(Opcode::GetPropertyByName(prop_idx));
                    // Stack: [module_obj, prop_value]
                    let var_idx = self.declare_variable(local, VariableKind::Var);
                    self.emit(Opcode::SetVar(var_idx));
                    // Stack: [module_obj, prop_value] (SetVar pushes back)
                    self.emit(Opcode::Pop);
                    // Stack: [module_obj]
                }
                ImportSpecifier::Namespace { local } => {
                    let var_idx = self.declare_variable(local, VariableKind::Var);
                    self.emit(Opcode::SetVar(var_idx));
                    self.emit(Opcode::Pop);
                }
            }
        }

        // Pop the module object
        self.emit(Opcode::Pop);

        Ok(())
    }

    fn compile_export(
        &mut self,
        declaration: Option<&ASTNode>,
        specifiers: &[ExportSpecifier],
        _source: Option<&str>,
    ) -> Result<(), CompileError> {
        if let Some(decl) = declaration {
            // Handle FunctionDeclaration directly (export default function ... or export function ...)
            if let ASTNode::FunctionDeclaration { name, params, body, is_async, is_generator } = decl {
                let effective_name = if name.is_empty() { "default" } else { name.as_str() };
                self.compile_func_decl(effective_name, params, body, *is_async, *is_generator)?;
                return Ok(());
            }
            // Handle ExpressionStatement wrapping a FunctionDeclaration
            if let ASTNode::ExpressionStatement(inner) = decl {
                if let ASTNode::FunctionDeclaration { name, params, body, is_async, is_generator } = inner.as_ref() {
                    let effective_name = if name.is_empty() { "default" } else { name.as_str() };
                    self.compile_func_decl(effective_name, params, body, *is_async, *is_generator)?;
                    return Ok(());
                }
                // For export default <expression>, compile and assign to "default"
                self.compile_expr(inner)?;
                let var_idx = self.declare_variable("default", VariableKind::Var);
                self.emit(Opcode::SetVar(var_idx));
                self.emit(Opcode::Pop);
                return Ok(());
            }
            // Handle VariableDeclaration (export const/let/var ...)
            if let ASTNode::VariableDeclaration { .. } = decl {
                self.compile_statement(decl)?;
                return Ok(());
            }
            // Handle ClassDeclaration (export class ...)
            if let ASTNode::ClassDeclaration { .. } = decl {
                self.compile_statement(decl)?;
                return Ok(());
            }
            self.compile_statement(decl)?;
        }
        Ok(())
    }

    // ================================================================
    // Line number tracking
    // ================================================================

    fn add_line_number(&mut self, node: &ASTNode) {
        // We don't have line info on ASTNode, but we track it for error reporting
    }

    // ================================================================
    // Helpers
    // ================================================================

    /// Emit an opcode and return its index.
    fn emit(&mut self, op: Opcode) -> u32 {
        let idx = self.function.bytecode.len() as u32;
        self.function.bytecode.push(op);
        idx
    }

    /// Patch a jump instruction to the current position.
    fn patch_jump(&mut self, idx: u32) {
        let current_pc = self.function.bytecode.len() as u32;
        self.patch_jump_to(idx, current_pc);
    }

    /// Patch a jump/handler instruction to a specific PC.
    fn patch_jump_to(&mut self, idx: u32, target_pc: u32) {
        if let Some(op) = self.function.bytecode.get_mut(idx as usize) {
            match op {
                Opcode::Jump(target)
                | Opcode::JumpIfTrue(target)
                | Opcode::JumpIfFalse(target)
                | Opcode::PushHandler(target) => {
                    *target = target_pc;
                }
                _ => {}
            }
        }
    }

    /// Add a constant to the constant pool.
    fn add_constant(&mut self, constant: Constant) -> u32 {
        // Check for duplicates
        for (i, c) in self.function.constants.iter().enumerate() {
            match (c, &constant) {
                (Constant::Float(a), Constant::Float(b)) if a == b => return i as u32,
                (Constant::String(a), Constant::String(b)) if a == b => return i as u32,
                _ => {}
            }
        }
        let idx = self.function.constants.len() as u32;
        self.function.constants.push(constant);
        idx
    }

    /// Finalize the closure variable mapping: store the expected closure var names
    /// (sorted by index) in the function's closure_vars field.
    fn finalize_closure(&mut self) {
        let mut pairs: Vec<(u32, String)> = self.closure_var_map.iter()
            .map(|(name, &idx)| (idx, name.clone()))
            .collect();
        pairs.sort_by_key(|(idx, _)| *idx);
        self.function.closure_vars = pairs.into_iter().map(|(_, name)| name).collect();
    }

    /// Declare a variable in the current scope.
    fn declare_variable(&mut self, name: &str, kind: VariableKind) -> u32 {
        let idx = self.function.variables.len() as u32;
        self.function.variables.push(Variable {
            name: name.to_string(),
            kind,
            scope_level: (self.var_map.len() - 1) as u32,
            is_captured: false,
            is_parameter: false,
        });
        if let Some(scope) = self.var_map.last_mut() {
            scope.insert(name.to_string(), idx);
        }
        idx
    }

    /// Declare a function parameter variable (is_parameter = true).
    fn declare_parameter(&mut self, name: &str) -> u32 {
        let idx = self.function.variables.len() as u32;
        self.function.variables.push(Variable {
            name: name.to_string(),
            kind: VariableKind::Var,
            scope_level: (self.var_map.len() - 1) as u32,
            is_captured: false,
            is_parameter: true,
        });
        if let Some(scope) = self.var_map.last_mut() {
            scope.insert(name.to_string(), idx);
        }
        idx
    }

    /// Resolve a variable by name, returning its index.
    fn resolve_variable(&self, name: &str) -> u32 {
        // Search from innermost to outermost scope
        for scope in self.var_map.iter().rev() {
            if let Some(&idx) = scope.get(name) {
                return idx;
            }
        }
        // Check if it's a closure variable from outer scope
        if let Some(&idx) = self.closure_var_map.get(name) {
            return idx;
        }
        // If not found, declare in the outermost scope (global)
        if let Some(scope) = self.var_map.first() {
            if let Some(&idx) = scope.get(name) {
                return idx;
            }
        }
        0 // fallback
    }

    /// Try to resolve a local variable (in var_map only, not closure); returns None if not found.
    fn resolve_local_variable(&self, name: &str) -> Option<u32> {
        for scope in self.var_map.iter().rev() {
            if let Some(&idx) = scope.get(name) {
                return Some(idx);
            }
        }
        None
    }

    /// Try to resolve a variable; returns None if not found.
    fn resolve_variable_opt(&self, name: &str) -> Option<u32> {
        for scope in self.var_map.iter().rev() {
            if let Some(&idx) = scope.get(name) {
                return Some(idx);
            }
        }
        // Check if it's a closure variable from outer scope
        if let Some(&idx) = self.closure_var_map.get(name) {
            return Some(idx);
        }
        None
    }

    /// Resolve or declare a variable.
    fn resolve_or_declare_variable(&mut self, name: &str, kind: VariableKind) -> u32 {
        if let Some(idx) = self.resolve_variable_opt(name) {
            idx
        } else {
            self.declare_variable(name, kind)
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level compile function: parse source code and compile to bytecode.
pub fn compile_source(source: &str, filename: Option<&str>) -> Result<BytecodeFunction, CompileError> {
    let mut parser = crate::parser::Parser::new(source);
    let ast = parser.parse().map_err(|e| CompileError::new(&e.to_string()))?;

    let mut compiler = Compiler::new();
    if let Some(f) = filename {
        compiler.set_filename(f.to_string());
    }
    compiler.compile(&ast).map_err(|e| CompileError::new(&e.to_string()))?;
    Ok(compiler.into_function())
}

/// Compile and execute JavaScript source code.
pub fn eval_source(
    source: &str,
    filename: Option<&str>,
) -> Result<crate::value::JSValue, String> {
    use crate::context::JSContext;
    use crate::interpreter::Interpreter;
    use crate::runtime::JSRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    let func = compile_source(source, filename).map_err(|e| e.to_string())?;

    let rt = Rc::new(RefCell::new(JSRuntime::new()));
    let ctx = JSContext::new(rt);
    let mut interp = Interpreter::new(ctx);
    interp.execute(&func).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_and_get_function(source: &str) -> BytecodeFunction {
        compile_source(source, None).unwrap()
    }

    #[test]
    fn test_compiler_creation() {
        let compiler = Compiler::new();
        assert_eq!(compiler.function.bytecode.len(), 0);
    }

    #[test]
    fn test_compile_number_literal() {
        let func = compile_and_get_function("42");
        assert!(!func.bytecode.is_empty());
    }

    #[test]
    fn test_compile_string_literal() {
        let func = compile_and_get_function("\"hello\"");
        assert!(!func.bytecode.is_empty());
        assert_eq!(func.constants.len(), 1);
    }

    #[test]
    fn test_compile_arithmetic() {
        let func = compile_and_get_function("1 + 2");
        // Should have: PushInt(1), PushInt(2), Add
        assert!(func.bytecode.len() >= 3);
    }

    #[test]
    fn test_compile_var_declaration() {
        let func = compile_and_get_function("var x = 10;");
        assert!(!func.variables.is_empty());
        assert_eq!(func.variables[0].name, "x");
    }

    #[test]
    fn test_compile_if_statement() {
        let func = compile_and_get_function("if (true) { 1; } else { 2; }");
        // Should have jump instructions
        let has_jump = func.bytecode.iter().any(|op| matches!(op, Opcode::JumpIfFalse(_)));
        assert!(has_jump);
    }

    #[test]
    fn test_compile_while_loop() {
        let func = compile_and_get_function("while (true) { break; }");
        let has_jump = func.bytecode.iter().any(|op| matches!(op, Opcode::Jump(_)));
        assert!(has_jump);
    }

    #[test]
    fn test_compile_function_declaration() {
        let func = compile_and_get_function("function add(a, b) { return a + b; }");
        assert!(!func.functions.is_empty());
        assert_eq!(func.functions[0].name, Some("add".to_string()));
    }

    #[test]
    fn test_compile_array_literal() {
        let func = compile_and_get_function("[1, 2, 3]");
        let has_create_array = func.bytecode.iter().any(|op| matches!(op, Opcode::CreateArray));
        assert!(has_create_array);
    }

    #[test]
    fn test_compile_object_literal() {
        let func = compile_and_get_function("var x = { a: 1, b: 2 };");
        let has_create_obj = func.bytecode.iter().any(|op| matches!(op, Opcode::CreateObject));
        assert!(has_create_obj);
    }

    #[test]
    fn test_compile_comparison() {
        let func = compile_and_get_function("1 < 2");
        let has_lt = func.bytecode.iter().any(|op| matches!(op, Opcode::Lt));
        assert!(has_lt);
    }

    #[test]
    fn test_compile_unary_not() {
        let func = compile_and_get_function("!true");
        let has_not = func.bytecode.iter().any(|op| matches!(op, Opcode::Not));
        assert!(has_not);
    }

    #[test]
    fn test_compile_ternary() {
        let func = compile_and_get_function("true ? 1 : 2");
        let has_jump = func.bytecode.iter().any(|op| matches!(op, Opcode::JumpIfFalse(_)));
        assert!(has_jump);
    }

    #[test]
    fn test_compile_for_loop() {
        let func = compile_and_get_function("for (var i = 0; i < 10; i++) { 1; }");
        let has_jump = func.bytecode.iter().any(|op| matches!(op, Opcode::Jump(_)));
        assert!(has_jump);
    }

    #[test]
    fn test_compile_arrow_function() {
        let func = compile_and_get_function("var add = (a, b) => a + b;");
        assert!(!func.functions.is_empty());
    }

    #[test]
    fn test_compile_null_literal() {
        let func = compile_and_get_function("null");
        let has_null = func.bytecode.iter().any(|op| matches!(op, Opcode::PushNull));
        assert!(has_null);
    }

    #[test]
    fn test_compile_undefined_literal() {
        let func = compile_and_get_function("undefined");
        let has_undef = func.bytecode.iter().any(|op| matches!(op, Opcode::PushUndefined));
        assert!(has_undef);
    }

    #[test]
    fn test_compile_typeof() {
        let func = compile_and_get_function("typeof x");
        let has_typeof = func.bytecode.iter().any(|op| matches!(op, Opcode::Typeof));
        assert!(has_typeof);
    }

    #[test]
    fn test_compile_property_access() {
        let func = compile_and_get_function("obj.prop");
        let has_get = func.bytecode.iter().any(|op| matches!(op, Opcode::GetPropertyByName(_)));
        assert!(has_get);
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_debug_compile() {
        let code = "1 + 2";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
        println!("Variables: {:?}", bytecode.variables);
        
        // Check that we have some bytecode
        assert!(!bytecode.bytecode.is_empty());
    }
}

#[cfg(test)]
mod typeof_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_typeof_array() {
        let code = "typeof Array";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
        
        // Should have GetPropertyByName for "Array"
        assert!(bytecode.bytecode.iter().any(|op| matches!(op, Opcode::GetPropertyByName(_))));
    }
}

#[cfg(test)]
mod array_isarray_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_array_isarray() {
        let code = "Array.isArray([1,2,3])";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
    }
}

#[cfg(test)]
mod array_isarray_var_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_array_isarray_var() {
        let code = "var arr = [1,2,3]; Array.isArray(arr)";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
    }
}

#[cfg(test)]
mod object_keys_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_object_keys() {
        let code = "Object.keys({a:1, b:2})";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
    }
}

#[cfg(test)]
mod object_keys_inline_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_object_keys_inline() {
        let code = "Object.keys({a:1})";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
    }
}

#[cfg(test)]
mod string_upper_tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_string_upper() {
        let code = "'hello'.toUpperCase()";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        
        println!("Bytecode: {:?}", bytecode.bytecode);
        println!("Constants: {:?}", bytecode.constants);
    }
}
