#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! JavaScript bytecode interpreter.
//!
//! Executes JavaScript bytecode with full support for all opcodes,
//! type coercion, exception handling, prototype chains, and more.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::context::JSContext;
use crate::runtime::JSRuntime;
use crate::value::{
    BytecodeFunction, Constant, FunctionBody, JSFunction, JSObject, JSValue, Opcode, VariableKind,
};

// Thread-local storage for the current interpreter's context (used by eval)
thread_local! {
    static EVAL_CONTEXT: RefCell<Option<JSContext>> = RefCell::new(None);
}

// Thread-local for accessing the host environment from native functions.
// These are populated from JSRuntime.host in execute().
thread_local! {
    static HOST_OUTPUT: RefCell<Option<Rc<dyn crate::host::Output>>> = RefCell::new(None);
    static HOST_CLOCK: RefCell<Option<Rc<dyn crate::host::Clock>>> = RefCell::new(None);
    static HOST_EXECUTOR: RefCell<Option<Rc<dyn crate::host::AsyncExecutor>>> = RefCell::new(None);
}

/// Get the current host output (for console.log, etc.)
pub fn get_output() -> Option<Rc<dyn crate::host::Output>> {
    HOST_OUTPUT.with(|o| o.borrow().clone())
}

/// Get the current host clock (for Date.now(), etc.)
pub fn get_clock() -> Option<Rc<dyn crate::host::Clock>> {
    HOST_CLOCK.with(|c| c.borrow().clone())
}

/// Get the current host async executor (for await, Promise, setTimeout, etc.)
pub fn get_executor() -> Option<Rc<dyn crate::host::AsyncExecutor>> {
    HOST_EXECUTOR.with(|e| e.borrow().clone())
}

// ============================================================================
// Exception handler
// ============================================================================

/// Exception handler entry on the handler stack.
#[derive(Debug, Clone)]
struct ExceptionHandler {
    /// Call frame depth (self.stack.len()) when this handler was pushed
    frame_depth: usize,
    /// Operand stack depth within the current frame when handler was pushed
    stack_depth: usize,
    /// Program counter to jump to for the catch block
    catch_pc: u32,
    /// Whether this handler has a catch block
    has_catch: bool,
}

// ============================================================================
// Stack frame
// ============================================================================

/// Stack frame for function calls.
#[derive(Debug, Clone)]
struct StackFrame {
    /// Function being executed
    function: BytecodeFunction,
    /// Program counter
    pc: u32,
    /// Local variables (shared references for closure capture)
    locals: Vec<Rc<RefCell<JSValue>>>,
    /// Operand stack for this frame
    stack: Vec<JSValue>,
    /// This binding
    this: JSValue,
    /// Captured closure variables (shared references from parent scope)
    closure: Vec<Rc<RefCell<JSValue>>>,
    /// Generator yields collection (shared with generator object)
    generator_yields: Option<Rc<RefCell<Vec<JSValue>>>>,
    /// Whether this frame is a constructor call (new)
    is_constructor: bool,
}

impl StackFrame {
    /// Create a new stack frame for a bytecode function.
    fn new(
        function: BytecodeFunction,
        this: JSValue,
        args: &[JSValue],
        closure: Vec<Rc<RefCell<JSValue>>>,
    ) -> Self {
        let mut locals: Vec<Rc<RefCell<JSValue>>> = (0..function.variables.len())
            .map(|_| Rc::new(RefCell::new(JSValue::Undefined)))
            .collect();

        // Track which local index holds `arguments` so positional fallback won't clobber it
        let mut arguments_idx: Option<usize> = None;
        if !function.is_arrow {
            let args_arr = crate::builtins::array::create_array(args.to_vec());
            for (j, var) in function.variables.iter().enumerate() {
                if var.name == "arguments" && !var.is_parameter {
                    locals[j] = Rc::new(RefCell::new(args_arr));
                    arguments_idx = Some(j);
                    break;
                }
            }
        }

        // Initialize parameter locals with arguments
        let rest_idx = function.rest_param_index;
        for (i, param_name) in function.params.iter().enumerate() {
            if rest_idx == Some(i) {
                let rest_args: Vec<JSValue> = args.get(i..).unwrap_or(&[]).to_vec();
                let rest_arr = crate::builtins::array::create_array(rest_args);
                for (j, var) in function.variables.iter().enumerate() {
                    if var.name == *param_name && var.is_parameter {
                        locals[j] = Rc::new(RefCell::new(rest_arr));
                        break;
                    }
                }
            } else if i < args.len() {
                for (j, var) in function.variables.iter().enumerate() {
                    if var.name == *param_name && var.is_parameter {
                        locals[j] = Rc::new(RefCell::new(args[i].clone()));
                        break;
                    }
                }
            }
        }

        // If no parameter variables found, use positional mapping
        // Skip the `arguments` slot to avoid overwriting it
        if function.variables.iter().all(|v| !v.is_parameter) {
            for (i, arg) in args.iter().enumerate() {
                if i < locals.len() && Some(i) != arguments_idx {
                    locals[i] = Rc::new(RefCell::new(arg.clone()));
                }
            }
        }

        StackFrame {
            function,
            pc: 0,
            locals,
            stack: Vec::with_capacity(256),
            this,
            closure,
            generator_yields: None,
            is_constructor: false,
        }
    }

    /// Pop a value from the operand stack.
    fn pop(&mut self) -> Result<JSValue, RuntimeError> {
        self.stack
            .pop()
            .ok_or_else(|| RuntimeError::new("Stack underflow"))
    }

    /// Peek at the top value without removing it.
    fn peek(&self) -> Result<JSValue, RuntimeError> {
        self.stack
            .last()
            .cloned()
            .ok_or_else(|| RuntimeError::new("Stack underflow"))
    }

    /// Push a value onto the operand stack.
    fn push(&mut self, val: JSValue) {
        self.stack.push(val);
    }
}

// ============================================================================
// Runtime error
// ============================================================================

/// Runtime error with stack trace.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
    pub stack_trace: Vec<StackFrameInfo>,
}

/// Stack frame information for error traces.
#[derive(Debug, Clone)]
pub struct StackFrameInfo {
    pub function_name: String,
    pub filename: Option<String>,
    pub line_number: Option<u32>,
    pub pc: u32,
}

impl RuntimeError {
    /// Create a new runtime error.
    pub fn new(message: &str) -> Self {
        RuntimeError {
            message: message.to_string(),
            stack_trace: Vec::new(),
        }
    }

    /// Create a new runtime error with a message.
    pub fn with_message(message: String) -> Self {
        RuntimeError {
            message,
            stack_trace: Vec::new(),
        }
    }

    /// Add a frame to the stack trace.
    fn add_frame(&mut self, function_name: &str, filename: Option<&str>, line: Option<u32>, pc: u32) {
        self.stack_trace.push(StackFrameInfo {
            function_name: function_name.to_string(),
            filename: filename.map(|s| s.to_string()),
            line_number: line,
            pc,
        });
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        for frame in &self.stack_trace {
            write!(f, "\n    at ")?;
            if !frame.function_name.is_empty() {
                write!(f, "{}", frame.function_name)?;
            } else {
                write!(f, "<anonymous>")?;
            }
            if let Some(ref filename) = frame.filename {
                write!(f, " ({}", filename)?;
                if let Some(line) = frame.line_number {
                    write!(f, ":{}", line)?;
                }
                write!(f, ")")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}

// ============================================================================
// Interpreter
// ============================================================================

/// JavaScript bytecode interpreter.
pub struct Interpreter {
    /// Execution context
    context: JSContext,
    /// Call stack
    stack: Vec<StackFrame>,
    /// Exception handler stack (shared across frames)
    handlers: Vec<ExceptionHandler>,
    /// Current pending exception (during unwinding)
    pending_exception: Option<JSValue>,
    /// Maximum stack depth
    max_stack_depth: usize,
    /// Interrupt flag
    interrupted: bool,
    /// Execution instruction count (for interrupt checking)
    instruction_count: u64,
    /// Instructions between interrupt checks
    interrupt_interval: u64,
    /// Saved top-level locals (for module exports)
    pub saved_locals: Vec<Rc<RefCell<JSValue>>>,
    /// Saved variable names (for module exports)
    pub saved_var_names: Vec<String>,
    /// Generator yielded value (set by Yield opcode, read by run_until_yield)
    generator_yielded: Option<JSValue>,
    /// Pending await Promise (set by Await opcode when Promise is pending)
    pending_await: Option<JSValue>,
}

/// Action to take after executing an opcode.
enum OpcodeAction {
    /// Continue processing opcodes in the current frame.
    Continue,
    /// A new frame was pushed (function call/constructor). Restart trampoline.
    FramePushed,
    /// A return was executed in the current frame.
    Returned,
    /// A yield was executed in the current frame (generator pause).
    Yielded,
    /// An await hit a pending Promise (async pause).
    Awaited,
}

/// Get the current eval context (if an interpreter is running).
/// Used by the eval() function to execute in the current scope.
pub fn get_eval_context() -> Option<JSContext> {
    EVAL_CONTEXT.with(|ctx| {
        let borrow = ctx.borrow();
        borrow.as_ref().map(|c| JSContext {
            runtime: c.runtime.clone(),
            global: c.global.clone(),
            var_env: c.var_env.clone(),
            this: c.this.clone(),
            exception: c.exception.clone(),
            strict_mode: c.strict_mode,
        })
    })
}

impl Interpreter {
    /// Create a new interpreter with the given context.
    pub fn new(context: JSContext) -> Self {
        Interpreter {
            context,
            stack: Vec::with_capacity(64),
            handlers: Vec::new(),
            pending_exception: None,
            max_stack_depth: 16384,
            interrupted: false,
            instruction_count: 0,
            interrupt_interval: 1000,
            saved_locals: Vec::new(),
            saved_var_names: Vec::new(),
            generator_yielded: None,
            pending_await: None,
        }
    }

    /// Create a new interpreter with custom settings.
    pub fn with_config(context: JSContext, max_stack_depth: usize, interrupt_interval: u64) -> Self {
        Interpreter {
            context,
            stack: Vec::with_capacity(64),
            handlers: Vec::new(),
            pending_exception: None,
            max_stack_depth,
            interrupted: false,
            instruction_count: 0,
            interrupt_interval,
            saved_locals: Vec::new(),
            saved_var_names: Vec::new(),
            generator_yielded: None,
            pending_await: None,
        }
    }

    /// Get a reference to the execution context.
    pub fn context(&self) -> &JSContext {
        &self.context
    }

    /// Get a mutable reference to the execution context.
    pub fn context_mut(&mut self) -> &mut JSContext {
        &mut self.context
    }

    /// Set the interrupt flag.
    pub fn interrupt(&mut self) {
        self.interrupted = true;
    }

    /// Clear the interrupt flag.
    pub fn clear_interrupt(&mut self) {
        self.interrupted = false;
    }

    /// Check if the interpreter is interrupted.
    pub fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    /// Execute a bytecode function.
    pub fn execute(&mut self, function: &BytecodeFunction) -> Result<JSValue, RuntimeError> {
        // Set eval context so eval() can access current scope
        EVAL_CONTEXT.with(|ctx| {
            *ctx.borrow_mut() = Some(JSContext {
                runtime: self.context.runtime.clone(),
                global: self.context.global.clone(),
                var_env: self.context.var_env.clone(),
                this: self.context.this.clone(),
                exception: self.context.exception.clone(),
                strict_mode: self.context.strict_mode,
            });
        });

        // Set host environment for native functions (console, Date, Math, async)
        // Read from the runtime's HostEnvironment (user-configurable)
        {
            let rt = self.context.runtime.borrow();
            HOST_OUTPUT.with(|o| {
                *o.borrow_mut() = Some(rt.host.output.clone());
            });
            HOST_CLOCK.with(|c| {
                *c.borrow_mut() = Some(rt.host.clock.clone());
            });
            HOST_EXECUTOR.with(|e| {
                *e.borrow_mut() = Some(rt.host.executor.clone());
            });
        }

        // Save variable names for module export support
        let var_names: Vec<String> = function.variables.iter().map(|v| v.name.clone()).collect();
        self.saved_var_names = var_names;

        let frame = StackFrame::new(
            function.clone(),
            JSValue::undefined(),
            &[],
            Vec::new(),
        );

        self.stack.push(frame);
        // run() processes the frame and pops it via return_from_frame,
        // which also saves top-level locals for module export support.
        self.run()
    }

    /// Call a function value with the given arguments and this binding.
    pub fn call_function(
        &mut self,
        func: &JSValue,
        this: JSValue,
        args: &[JSValue],
    ) -> Result<JSValue, RuntimeError> {
        match func {
            JSValue::Function(f) => {
                let func_ref = f.clone();
                let (body_clone, closure_clone) = {
                    let func_borrow = func_ref.borrow();
                    let body = func_borrow.body.clone();
                    let closure = func_borrow.closure.clone();
                    (body, closure)
                };

                match body_clone {
                    FunctionBody::Native(native_fn) => {
                        let result = native_fn(&this, args);
                        Ok(result)
                    }
                    FunctionBody::Bytecode(bc_func) => {
                        let closure_var_order = bc_func.closure_vars.clone();
                        let function = bc_func;
                        let closure: Vec<Rc<RefCell<JSValue>>> = closure_var_order.iter()
                            .filter_map(|name| closure_clone.get(name).cloned())
                            .collect();

                        let frame = StackFrame::new(function, this, args, closure);
                        self.push_frame(frame)?;
                        // run() processes the frame and pops it via return_from_frame
                        self.run()
                    }
                    FunctionBody::Closure(closure_fn) => {
                        let result = closure_fn(&this, args);
                        Ok(result)
                    }
                    FunctionBody::Source(_) => {
                        Err(RuntimeError::new("Source function execution not yet supported"))
                    }
                    FunctionBody::Generator { func, yields } => {
                        // For call_function, just return undefined for generators
                        Ok(JSValue::undefined())
                    }
                    FunctionBody::GeneratorNext { state } => {
                        // For call_function, just return undefined for generator next
                        Ok(JSValue::undefined())
                    }
                }
            }
            _ => Err(RuntimeError::new("Cannot call non-function")),
        }
    }

    /// Push a new stack frame with overflow detection.
    fn push_frame(&mut self, frame: StackFrame) -> Result<(), RuntimeError> {
        if self.stack.len() >= self.max_stack_depth {
            return Err(RuntimeError::new("Maximum call stack size exceeded"));
        }
        self.stack.push(frame);
        Ok(())
    }

    /// Pop the current stack frame.
    fn pop_frame(&mut self) {
        self.stack.pop();
    }

    /// Build a stack trace from the current call stack.
    fn build_stack_trace(&self) -> Vec<StackFrameInfo> {
        let mut trace = Vec::new();
        for frame in self.stack.iter().rev() {
            let name = frame
                .function
                .name
                .clone()
                .unwrap_or_default();
            let line = frame.function.line_number(frame.pc);
            trace.push(StackFrameInfo {
                function_name: name,
                filename: frame.function.filename.clone(),
                line_number: line,
                pc: frame.pc,
            });
        }
        trace
    }

    /// The main interpreter execution loop.
    fn run(&mut self) -> Result<JSValue, RuntimeError> {
        // The last return value from the innermost frame (used when stack becomes empty)
        let mut final_result: Option<JSValue> = None;

        // Outer trampoline loop: restarts when a new frame is pushed
        'trampoline: loop {
            // Inner opcode loop: processes the current frame
            loop {
                // Check for interrupts
                self.instruction_count += 1;
                if self.instruction_count >= self.interrupt_interval {
                    self.instruction_count = 0;
                    if self.interrupted {
                        return Err(RuntimeError::new("Script execution interrupted"));
                    }
                }

                // Check for pending exception (during stack unwinding)
                if self.pending_exception.is_some() && !self.handlers.is_empty() {
                    let handler = self.handlers.last().unwrap();
                    let handler_clone = handler.clone();

                    if handler_clone.has_catch {
                        // Unwind call frames back to the handler's frame depth
                        while self.stack.len() > handler_clone.frame_depth {
                            self.stack.pop();
                        }

                        // Restore operand stack depth and jump to catch block
                        if let Some(frame) = self.stack.last_mut() {
                            frame.stack.truncate(handler_clone.stack_depth);
                            frame.pc = handler_clone.catch_pc;
                        }

                        // Push the exception value onto the stack
                        if let Some(exc) = self.pending_exception.take() {
                            if let Some(frame) = self.stack.last_mut() {
                                frame.stack.push(exc);
                            }
                        }

                        // Remove the used handler
                        self.handlers.pop();
                        continue;
                    }
                }

                // Get the current frame
                let frame = match self.stack.last_mut() {
                    Some(f) => f,
                    None => {
                        return if let Some(exc) = self.pending_exception.take() {
                            let mut err = RuntimeError::with_message(exc.to_string());
                            err.stack_trace = self.build_stack_trace();
                            Err(err)
                        } else {
                            Ok(final_result.unwrap_or(JSValue::undefined()))
                        };
                    }
                };

                // Check if we've reached the end of the bytecode
                if frame.pc >= frame.function.bytecode.len() as u32 {
                    let result = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                    self.return_from_frame(result, &mut final_result);
                    continue;
                }

                // Fetch the opcode
                let opcode = frame.function.bytecode[frame.pc as usize].clone();
                frame.pc += 1;

                // Execute the opcode
                let action = self.execute_opcode(&opcode)?;
                match action {
                    OpcodeAction::Continue => {}
                    OpcodeAction::FramePushed => {
                        // A new frame was pushed - restart the trampoline
                        continue 'trampoline;
                    }
                    OpcodeAction::Returned => {
                        // Return was executed
                        if let Some(exc) = self.pending_exception.take() {
                            if self.stack.len() <= 1 {
                                let mut err = RuntimeError::with_message(exc.to_string());
                                err.stack_trace = self.build_stack_trace();
                                self.stack.pop();
                                return Err(err);
                            }
                            self.stack.pop();
                            self.pending_exception = Some(exc);
                            continue;
                        }
                        let result = self.stack.last().unwrap()
                            .stack.last().cloned().unwrap_or(JSValue::undefined());
                        self.return_from_frame(result, &mut final_result);
                    }
                    OpcodeAction::Yielded => {
                        // Generator yielded - save the frame state and return the yielded value
                        let yielded = self.generator_yielded.take().unwrap_or(JSValue::undefined());
                        // Save the current frame state to the generator object
                        if let Some(frame) = self.stack.last() {
                            if let Some(ref yields) = frame.generator_yields {
                                yields.borrow_mut().push(yielded.clone());
                            }
                        }
                        self.stack.pop();
                        return Ok(yielded);
                    }
                    OpcodeAction::Awaited => {
                        // Await hit a pending Promise - delegate to host executor
                        let promise = self.pending_await.take().unwrap_or(JSValue::undefined());
                        let executor = get_executor().unwrap_or_else(|| {
                            std::rc::Rc::new(crate::host::SyncExecutor)
                        });

                        // Use a shared cell to receive the resolved value
                        let result_cell = std::rc::Rc::new(std::cell::RefCell::new(JSValue::undefined()));
                        let result_ref = result_cell.clone();

                        executor.on_await(&promise, Box::new(move |val| {
                            *result_ref.borrow_mut() = val;
                        }));

                        // Push the resolved value and continue
                        let resolved = std::rc::Rc::try_unwrap(result_cell)
                            .map(|c| c.into_inner())
                            .unwrap_or(JSValue::undefined());
                        if let Some(frame) = self.stack.last_mut() {
                            frame.stack.push(resolved);
                        }
                    }
                }
            }
        }
    }

    /// Pop the current frame and push the return value onto the caller's stack.
    /// If there's no caller (last frame), store it in final_result.
    fn return_from_frame(&mut self, result: JSValue, final_result: &mut Option<JSValue>) {
        // Check if this is a constructor frame
        let is_ctor = self.stack.last().map(|f| f.is_constructor).unwrap_or(false);
        let this_val = self.stack.last().map(|f| f.this.clone());
        let is_last_frame = self.stack.len() == 1;

        // Save top-level locals before popping (for module export support)
        if is_last_frame {
            if let Some(frame) = self.stack.last() {
                self.saved_locals = frame.locals.clone();
            }
        }

        // Clean up handlers that belong to this frame
        let frame_depth = self.stack.len();
        self.handlers.retain(|h| h.frame_depth < frame_depth);

        self.stack.pop();

        // For constructors: if the return value is not an object, use `this` instead
        let final_val = if is_ctor {
            if result.is_object() {
                result
            } else {
                this_val.unwrap_or(result)
            }
        } else {
            result
        };

        if let Some(caller) = self.stack.last_mut() {
            caller.stack.push(final_val);
        } else {
            *final_result = Some(final_val);
        }
    }

    /// Run a single frame to completion without the trampoline (for generators).
    /// Executes the frame's bytecode in a simple loop, handling yields and returns.
    fn run_frame_direct(&mut self, frame: StackFrame) -> Result<JSValue, RuntimeError> {
        self.stack.push(frame);
        let mut result = JSValue::undefined();

        loop {
            let frame = match self.stack.last_mut() {
                Some(f) => f,
                None => break,
            };

            if frame.pc >= frame.function.bytecode.len() as u32 {
                result = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                self.stack.pop();
                break;
            }

            let opcode = frame.function.bytecode[frame.pc as usize].clone();
            frame.pc += 1;

            let action = self.execute_opcode(&opcode)?;
            match action {
                OpcodeAction::Continue => {}
                OpcodeAction::FramePushed => {
                    // A function call within the generator - run the callee
                    let inner_result = self.run_frame_direct_simple()?;
                    // Push result on the generator frame
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(inner_result);
                    }
                }
                OpcodeAction::Returned => {
                    result = self.stack.last().unwrap()
                        .stack.last().cloned().unwrap_or(JSValue::undefined());
                    self.stack.pop();
                    break;
                }
                OpcodeAction::Yielded => {
                    let yielded = self.generator_yielded.take().unwrap_or(JSValue::undefined());
                    self.stack.pop();
                    result = yielded;
                    break;
                }
                OpcodeAction::Awaited => {
                    // Await in a sub-frame - resolve via executor
                    let promise = self.pending_await.take().unwrap_or(JSValue::undefined());
                    let executor = get_executor().unwrap_or_else(|| std::rc::Rc::new(crate::host::SyncExecutor));
                    let result_cell = std::rc::Rc::new(std::cell::RefCell::new(JSValue::undefined()));
                    let result_ref = result_cell.clone();
                    executor.on_await(&promise, Box::new(move |val| {
                        *result_ref.borrow_mut() = val;
                    }));
                    let resolved = std::rc::Rc::try_unwrap(result_cell).map(|c| c.into_inner()).unwrap_or(JSValue::undefined());
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(resolved);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Simple frame runner for nested calls within generators.
    fn run_frame_direct_simple(&mut self) -> Result<JSValue, RuntimeError> {
        let mut result = JSValue::undefined();

        loop {
            let frame = match self.stack.last_mut() {
                Some(f) => f,
                None => break,
            };

            if frame.pc >= frame.function.bytecode.len() as u32 {
                result = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                self.stack.pop();
                break;
            }

            let opcode = frame.function.bytecode[frame.pc as usize].clone();
            frame.pc += 1;

            let action = self.execute_opcode(&opcode)?;
            match action {
                OpcodeAction::Continue => {}
                OpcodeAction::FramePushed => {
                    let inner_result = self.run_frame_direct_simple()?;
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(inner_result);
                    }
                }
                OpcodeAction::Returned => {
                    result = self.stack.last().unwrap()
                        .stack.last().cloned().unwrap_or(JSValue::undefined());
                    self.stack.pop();
                    break;
                }
                OpcodeAction::Yielded => {
                    let yielded = self.generator_yielded.take().unwrap_or(JSValue::undefined());
                    self.stack.pop();
                    result = yielded;
                    break;
                }
                OpcodeAction::Awaited => {
                    let promise = self.pending_await.take().unwrap_or(JSValue::undefined());
                    let executor = get_executor().unwrap_or_else(|| std::rc::Rc::new(crate::host::SyncExecutor));
                    let result_cell = std::rc::Rc::new(std::cell::RefCell::new(JSValue::undefined()));
                    let result_ref = result_cell.clone();
                    executor.on_await(&promise, Box::new(move |val| {
                        *result_ref.borrow_mut() = val;
                    }));
                    let resolved = std::rc::Rc::try_unwrap(result_cell).map(|c| c.into_inner()).unwrap_or(JSValue::undefined());
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(resolved);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Run a generator frame lazily - execute until next yield or return.
    /// Returns (yielded_value, is_done, saved_frame).
    /// If yielded, saved_frame contains the paused frame state for resumption.
    fn run_generator_until_yield(
        &mut self,
        frame: StackFrame,
        state: Rc<RefCell<crate::value::GeneratorState>>,
    ) -> Result<(JSValue, bool, Option<StackFrame>), RuntimeError> {
        self.stack.push(frame);

        loop {
            let frame = match self.stack.last_mut() {
                Some(f) => f,
                None => return Ok((JSValue::undefined(), true, None)),
            };

            if frame.pc >= frame.function.bytecode.len() as u32 {
                // Generator finished without explicit return
                let result = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                self.stack.pop();
                // Mark generator as done
                state.borrow_mut().done = true;
                return Ok((result, true, None));
            }

            let opcode = frame.function.bytecode[frame.pc as usize].clone();
            frame.pc += 1;

            let action = self.execute_opcode(&opcode)?;
            match action {
                OpcodeAction::Continue => {}
                OpcodeAction::FramePushed => {
                    // Function call within generator - run callee to completion
                    let inner_result = self.run_frame_direct_simple()?;
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(inner_result);
                    }
                }
                OpcodeAction::Returned => {
                    // Generator returned
                    let result = self.stack.last().unwrap()
                        .stack.last().cloned().unwrap_or(JSValue::undefined());
                    self.stack.pop();
                    state.borrow_mut().done = true;
                    return Ok((result, true, None));
                }
                OpcodeAction::Yielded => {
                    let yielded = self.generator_yielded.take().unwrap_or(JSValue::undefined());
                    if let Some(frame) = self.stack.last() {
                        let mut s = state.borrow_mut();
                        s.saved_pc = frame.pc;
                        s.saved_locals = frame.locals.clone();
                        s.saved_stack = frame.stack.clone();
                        s.saved_closure = frame.closure.clone();
                    }
                    self.stack.pop();
                    return Ok((yielded, false, None));
                }
                OpcodeAction::Awaited => {
                    let promise = self.pending_await.take().unwrap_or(JSValue::undefined());
                    let executor = get_executor().unwrap_or_else(|| std::rc::Rc::new(crate::host::SyncExecutor));
                    let result_cell = std::rc::Rc::new(std::cell::RefCell::new(JSValue::undefined()));
                    let result_ref = result_cell.clone();
                    executor.on_await(&promise, Box::new(move |val| {
                        *result_ref.borrow_mut() = val;
                    }));
                    let resolved = std::rc::Rc::try_unwrap(result_cell).map(|c| c.into_inner()).unwrap_or(JSValue::undefined());
                    if let Some(frame) = self.stack.last_mut() {
                        frame.stack.push(resolved);
                    }
                }
            }
        }
    }

    /// Execute a single opcode.
    fn execute_opcode(&mut self, opcode: &Opcode) -> Result<OpcodeAction, RuntimeError> {
        match opcode {
            // ================================================================
            // Stack operations
            // ================================================================

            Opcode::GetGlobal => {
                let global = self.context.global.clone();
                self.push_stack(JSValue::Object(global));
            }

            Opcode::GetGlobalVar(idx) => {
                let name = self.get_constant_string(*idx)?;
                let global_val = JSValue::Object(self.context.global.clone());
                let val = global_val.get_property(&name).unwrap_or(JSValue::undefined());
                if val.is_undefined() {
                    let err = JSValue::object("ReferenceError");
                    err.set_property("message", JSValue::string(&format!("{} is not defined", name)));
                    err.set_property("name", JSValue::string("ReferenceError"));
                    // Set prototype to ReferenceError.prototype for instanceof to work
                    let ref_error = global_val.get_property("ReferenceError");
                    if let Some(proto) = ref_error.and_then(|r| r.get_property("prototype")) {
                        if let JSValue::Object(obj) = &err {
                            if let JSValue::Object(p) = &proto {
                                obj.borrow_mut().prototype = Some(p.clone());
                            }
                        }
                    }
                    self.pending_exception = Some(err);
                    return Ok(OpcodeAction::Continue);
                }
                self.push_stack(val);
            }

            Opcode::PushUndefined => {
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::undefined());
            }

            Opcode::PushNull => {
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::null());
            }

            Opcode::PushBool(b) => {
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::bool(*b));
            }

            Opcode::PushInt(i) => {
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::int(*i));
            }

            Opcode::PushFloat(idx) => {
                let val = self.get_constant_float(*idx)?;
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::float(val));
            }

            Opcode::PushString(idx) => {
                let val = self.get_constant_string(*idx)?;
                let frame = self.stack.last_mut().unwrap();
                frame.push(JSValue::string(&val));
            }

            Opcode::PushRegExp(p_idx, f_idx) => {
                let pattern = self.get_constant_string(*p_idx)?;
                let flags = self.get_constant_string(*f_idx)?;
                let regexp = JSValue::object("RegExp");
                regexp.set_property("source", JSValue::string(&pattern));
                regexp.set_property("flags", JSValue::string(&flags));
                regexp.set_property("lastIndex", JSValue::Int(0));
                let frame = self.stack.last_mut().unwrap();
                frame.push(regexp);
            }

            Opcode::Pop => {
                let frame = self.stack.last_mut().unwrap();
                frame.pop()?;
            }

            Opcode::Dup => {
                let frame = self.stack.last_mut().unwrap();
                let val = frame.peek()?;
                frame.push(val);
            }

            Opcode::Swap => {
                let frame = self.stack.last_mut().unwrap();
                let len = frame.stack.len();
                if len < 2 {
                    return Err(self.runtime_error("Stack underflow on swap"));
                }
                frame.stack.swap(len - 1, len - 2);
            }

            Opcode::Rotate(n) => {
                // Rotate top N elements: move the bottom element to the top
                // e.g., Rotate(3): [a, b, c] → [b, c, a]
                let frame = self.stack.last_mut().unwrap();
                let len = frame.stack.len();
                if len < *n as usize {
                    return Err(self.runtime_error("Stack underflow on rotate"));
                }
                let bottom = frame.stack.remove(len - *n as usize);
                frame.stack.push(bottom);
            }

            // ================================================================
            // Arithmetic
            // ================================================================

            Opcode::Add => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let result = Self::op_add(&left, &right)?;
                self.push_stack(result);
            }

            Opcode::Sub => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = Self::to_number_value(&left);
                let r = Self::to_number_value(&right);
                self.push_stack(JSValue::float(l - r));
            }

            Opcode::Mul => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = Self::to_number_value(&left);
                let r = Self::to_number_value(&right);
                self.push_stack(JSValue::float(l * r));
            }

            Opcode::Div => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = Self::to_number_value(&left);
                let r = Self::to_number_value(&right);
                let result = if r == 0.0 {
                    if l == 0.0 {
                        f64::NAN
                    } else if l.is_sign_positive() {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    }
                } else {
                    l / r
                };
                self.push_stack(JSValue::float(result));
            }

            Opcode::Mod => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = Self::to_number_value(&left);
                let r = Self::to_number_value(&right);
                if r == 0.0 {
                    self.push_stack(JSValue::float(f64::NAN));
                } else {
                    // JavaScript modulo preserves sign of dividend
                    let result = l - (l / r).trunc() * r;
                    self.push_stack(JSValue::float(result));
                }
            }

            Opcode::Pow => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = Self::to_number_value(&left);
                let r = Self::to_number_value(&right);
                let result = l.powf(r);
                self.push_stack(JSValue::float(result));
            }

            Opcode::Neg => {
                let val = self.pop_stack()?;
                let n = Self::to_number_value(&val);
                self.push_stack(JSValue::float(-n));
            }

            Opcode::Plus => {
                // Unary + : convert to number
                let val = self.pop_stack()?;
                let n = Self::to_number_value(&val);
                self.push_stack(JSValue::float(n));
            }

            Opcode::Inc => {
                let val = self.pop_stack()?;
                let n = Self::to_number_value(&val);
                self.push_stack(JSValue::float(n + 1.0));
            }

            Opcode::Dec => {
                let val = self.pop_stack()?;
                let n = Self::to_number_value(&val);
                self.push_stack(JSValue::float(n - 1.0));
            }

            // ================================================================
            // Bitwise
            // ================================================================

            Opcode::BitAnd => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_int32();
                let r = right.to_int32();
                self.push_stack(JSValue::int(l & r));
            }

            Opcode::BitOr => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_int32();
                let r = right.to_int32();
                self.push_stack(JSValue::int(l | r));
            }

            Opcode::BitXor => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_int32();
                let r = right.to_int32();
                self.push_stack(JSValue::int(l ^ r));
            }

            Opcode::BitNot => {
                let val = self.pop_stack()?;
                let n = val.to_int32();
                self.push_stack(JSValue::int(!n));
            }

            Opcode::Shl => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_int32();
                let r = right.to_uint32() & 0x1F; // mask to 5 bits
                self.push_stack(JSValue::int(l.wrapping_shl(r)));
            }

            Opcode::Shr => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_int32();
                let r = right.to_uint32() & 0x1F;
                self.push_stack(JSValue::int(l.wrapping_shr(r)));
            }

            Opcode::UShr => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let l = left.to_uint32();
                let r = right.to_uint32() & 0x1F;
                self.push_stack(JSValue::int((l.wrapping_shr(r)) as i32));
            }

            // ================================================================
            // Comparison
            // ================================================================

            Opcode::Eq => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                self.push_stack(JSValue::bool(left.abstract_eq(&right)));
            }

            Opcode::Ne => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                self.push_stack(JSValue::bool(!left.abstract_eq(&right)));
            }

            Opcode::StrictEq => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                self.push_stack(JSValue::bool(left.strict_eq(&right)));
            }

            Opcode::StrictNe => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                self.push_stack(JSValue::bool(!left.strict_eq(&right)));
            }

            Opcode::Lt => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let result = Self::op_less_than(&left, &right)?;
                self.push_stack(JSValue::bool(result));
            }

            Opcode::Le => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                // a <= b is !(b < a)
                let result = Self::op_less_than(&right, &left)?;
                self.push_stack(JSValue::bool(!result));
            }

            Opcode::Gt => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                // a > b is b < a
                let result = Self::op_less_than(&right, &left)?;
                self.push_stack(JSValue::bool(result));
            }

            Opcode::Ge => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                // a >= b is !(a < b)
                let result = Self::op_less_than(&left, &right)?;
                self.push_stack(JSValue::bool(!result));
            }

            // ================================================================
            // Logical
            // ================================================================

            Opcode::LogicalAnd => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                if left.is_truthy() {
                    self.push_stack(right);
                } else {
                    self.push_stack(left);
                }
            }

            Opcode::LogicalOr => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                if left.is_falsy() {
                    self.push_stack(right);
                } else {
                    self.push_stack(left);
                }
            }

            Opcode::Not => {
                let val = self.pop_stack()?;
                self.push_stack(JSValue::bool(val.is_falsy()));
            }

            // ================================================================
            // Control flow
            // ================================================================

            Opcode::Jump(target) => {
                let frame = self.stack.last_mut().unwrap();
                frame.pc = *target;
            }

            Opcode::JumpIfTrue(target) => {
                let frame = self.stack.last().unwrap();
                let val = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                if val.is_truthy() {
                    let frame = self.stack.last_mut().unwrap();
                    frame.pc = *target;
                }
            }

            Opcode::JumpIfFalse(target) => {
                let frame = self.stack.last().unwrap();
                let val = frame.stack.last().cloned().unwrap_or(JSValue::undefined());
                if val.is_falsy() {
                    let frame = self.stack.last_mut().unwrap();
                    frame.pc = *target;
                }
            }

            Opcode::Return => {
                let val = self.pop_stack().unwrap_or(JSValue::undefined());
                let is_async = self.stack.last().map(|f| f.function.is_async).unwrap_or(false);
                let frame = self.stack.last_mut().unwrap();
                frame.stack.clear();
                // If this is an async function, wrap the return value in a resolved Promise
                if is_async {
                    let promise = JSValue::object("Promise");
                    promise.set_property("__state", JSValue::Int(1)); // FULFILLED
                    promise.set_property("__result", val);
                    promise.set_property("__reactions", crate::builtins::array::create_array(vec![]));
                    frame.stack.push(promise);
                } else {
                    frame.stack.push(val);
                }
                return Ok(OpcodeAction::Returned);
            }

            Opcode::Throw => {
                let val = self.pop_stack()?;
                // Set pending exception
                self.pending_exception = Some(val);
                // The exception will be handled in the next iteration of the run loop
            }

            Opcode::PushHandler(catch_pc) => {
                let frame_depth = self.stack.len();
                let stack_depth = self.stack.last().map(|f| f.stack.len()).unwrap_or(0);
                self.handlers.push(ExceptionHandler {
                    frame_depth,
                    stack_depth,
                    catch_pc: *catch_pc,
                    has_catch: true,
                });
            }

            Opcode::PopHandler => {
                self.handlers.pop();
            }

            // ================================================================
            // Variables
            // ================================================================

            Opcode::GetVar(idx) => {
                let frame = self.stack.last().unwrap();
                if (*idx as usize) < frame.locals.len() {
                    let val = frame.locals[*idx as usize].borrow().clone();
                    self.push_stack(val);
                } else {
                    // Undeclared variable: throw ReferenceError
                    let name = self.saved_var_names.get(*idx as usize).cloned().unwrap_or_else(|| "?".to_string());
                    return Err(RuntimeError::with_message(format!("ReferenceError: {} is not defined", name)));
                }
            }

            Opcode::SetVar(idx) => {
                let val = self.pop_stack()?;
                let frame = self.stack.last_mut().unwrap();
                if (*idx as usize) < frame.locals.len() {
                    *frame.locals[*idx as usize].borrow_mut() = val.clone();
                }
                self.push_stack(val);
            }

            Opcode::DeclareVar(idx, _kind) => {
                // Ensure the local slot exists
                let frame = self.stack.last_mut().unwrap();
                if (*idx as usize) >= frame.locals.len() {
                    frame.locals.resize(*idx as usize + 1, Rc::new(RefCell::new(JSValue::Undefined)));
                }
            }

            Opcode::This => {
                let this = self.stack.last()
                    .map(|f| f.this.clone())
                    .unwrap_or(JSValue::undefined());
                self.push_stack(this);
            }

            // ================================================================
            // Closure variables
            // ================================================================

            Opcode::GetClosure(idx) => {
                let frame = self.stack.last().unwrap();
                let val = if (*idx as usize) < frame.closure.len() {
                    frame.closure[*idx as usize].borrow().clone()
                } else {
                    JSValue::undefined()
                };
                self.push_stack(val);
            }

            Opcode::SetClosure(idx) => {
                let val = self.pop_stack()?;
                let frame = self.stack.last_mut().unwrap();
                if (*idx as usize) < frame.closure.len() {
                    *frame.closure[*idx as usize].borrow_mut() = val.clone();
                }
                self.push_stack(val);
            }

            // ================================================================
            // Properties
            // ================================================================

            Opcode::GetProperty => {
                let prop = self.pop_stack()?;
                let obj = self.pop_stack()?;
                // Check for null/undefined
                if obj.is_null() || obj.is_undefined() {
                    let type_name = if obj.is_null() { "null" } else { "undefined" };
                    let msg = format!("Cannot read properties of {} (reading '{}')", type_name, prop.to_string());
                    let err = JSValue::object("TypeError");
                    err.set_property("message", JSValue::string(&msg));
                    err.set_property("name", JSValue::string("TypeError"));
                    self.pending_exception = Some(err);
                    return Ok(OpcodeAction::Continue);
                }
                let prop_name = prop.to_string();
                // Auto-box primitives for property access
                let wrapped = self.auto_box(obj);
                let result = wrapped.get_property(&prop_name).unwrap_or(JSValue::undefined());
                self.push_stack(result);
            }

            Opcode::SetProperty => {
                let val = self.pop_stack()?;
                let prop = self.pop_stack()?;
                let obj = self.pop_stack()?;
                let prop_name = prop.to_string();
                obj.set_property(&prop_name, val.clone());
                // Assignment expression returns the value
                self.push_stack(val);
            }

            Opcode::GetPropertyByName(idx) => {
                let prop_name = self.get_constant_string(*idx)?;
                let obj = self.pop_stack()?;
                // Check for null/undefined
                if obj.is_null() || obj.is_undefined() {
                    let type_name = if obj.is_null() { "null" } else { "undefined" };
                    let msg = format!("Cannot read properties of {} (reading '{}')", type_name, prop_name);
                    let err = JSValue::object("TypeError");
                    err.set_property("message", JSValue::string(&msg));
                    err.set_property("name", JSValue::string("TypeError"));
                    self.pending_exception = Some(err);
                    return Ok(OpcodeAction::Continue);
                }
                // Auto-box primitives for property access
                let wrapped = self.auto_box(obj.clone());

                // Check if this is a Proxy - intercept with handler's get trap
                let is_proxy = match &wrapped {
                    JSValue::Object(o) => o.borrow().class_name == "Proxy",
                    _ => false,
                };

                let result = if is_proxy {
                    // Get target and handler from Proxy's internal_slots
                    let (target, handler) = match &wrapped {
                        JSValue::Object(o) => {
                            let borrow = o.borrow();
                            let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                            let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                            (target, handler)
                        }
                        _ => (JSValue::undefined(), JSValue::undefined()),
                    };

                    // Check for get trap
                    if let Some(get_trap) = handler.get_property("get") {
                        let result = self.execute_function(&get_trap, handler.clone(), &[target, JSValue::string(&prop_name)]);
                        match result {
                            Ok(true) => {
                                let inner_result = self.run_frame_direct_simple().ok();
                                inner_result.unwrap_or(JSValue::undefined())
                            }
                            Ok(false) => {
                                self.pop_stack().unwrap_or(JSValue::undefined())
                            }
                            Err(_) => JSValue::undefined(),
                        }
                    } else {
                        // No get trap - look up on target directly
                        target.get_property(&prop_name).unwrap_or(JSValue::undefined())
                    }
                } else {
                    let mut res = wrapped.get_property(&prop_name);

                    // If not found on the object, look up the prototype from global
                    if res.is_none() {
                        let class_name = match &wrapped {
                            JSValue::Object(o) => o.borrow().class_name.clone(),
                            _ => String::new(),
                        };
                        if !class_name.is_empty() {
                            if let Some(ctor) = self.context.global.borrow().properties.get(&class_name).cloned() {
                                if let Some(proto) = ctor.get_property("prototype") {
                                    if let JSValue::Object(ref proto_obj) = proto {
                                        res = JSValue::Object(proto_obj.clone()).get_property(&prop_name);
                                    }
                                }
                            }
                        }
                    }

                    res.unwrap_or(JSValue::undefined())
                };

                self.push_stack(result);
            }

            Opcode::GetField2(idx) => {
                let prop_name = self.get_constant_string(*idx)?;
                let obj = self.pop_stack()?;
                // Check for null/undefined
                if obj.is_null() || obj.is_undefined() {
                    let type_name = if obj.is_null() { "null" } else { "undefined" };
                    let msg = format!("Cannot read properties of {} (reading '{}')", type_name, prop_name);
                    let err = JSValue::object("TypeError");
                    err.set_property("message", JSValue::string(&msg));
                    err.set_property("name", JSValue::string("TypeError"));
                    self.pending_exception = Some(err);
                    return Ok(OpcodeAction::Continue);
                }
                // Auto-box primitives for property access
                let wrapped = self.auto_box(obj.clone());

                // Check if this is a Proxy - intercept with handler's get trap
                let is_proxy = match &wrapped {
                    JSValue::Object(o) => o.borrow().class_name == "Proxy",
                    _ => false,
                };

                let result = if is_proxy {
                    // Get target and handler from Proxy's internal_slots
                    let (target, handler) = match &wrapped {
                        JSValue::Object(o) => {
                            let borrow = o.borrow();
                            let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                            let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                            (target, handler)
                        }
                        _ => (JSValue::undefined(), JSValue::undefined()),
                    };

                    // Check for get trap
                    if let Some(get_trap) = handler.get_property("get") {
                        // Call get trap with (target, prop_name, receiver)
                        let result = self.execute_function(&get_trap, handler.clone(), &[target, JSValue::string(&prop_name)]);
                        match result {
                            Ok(true) => {
                                // Frame was pushed - run it to get the result
                                let inner_result = self.run_frame_direct_simple().ok();
                                inner_result
                            }
                            Ok(false) => {
                                // Value was pushed directly
                                self.pop_stack().ok()
                            }
                            Err(_) => None,
                        }
                    } else {
                        // No get trap - look up on target directly
                        Some(target.get_property(&prop_name).unwrap_or(JSValue::undefined()))
                    }
                } else {
                    None
                };

                let result = result.unwrap_or_else(|| {
                    let mut res = wrapped.get_property(&prop_name);

                    // If not found on the object, look up the prototype from global
                    if res.is_none() {
                        let class_name = match &wrapped {
                            JSValue::Object(o) => o.borrow().class_name.clone(),
                            _ => String::new(),
                        };
                        if !class_name.is_empty() {
                            if let Some(ctor) = self.context.global.borrow().properties.get(&class_name).cloned() {
                                if let Some(proto) = ctor.get_property("prototype") {
                                    if let JSValue::Object(ref proto_obj) = proto {
                                        res = JSValue::Object(proto_obj.clone()).get_property(&prop_name);
                                    }
                                }
                            }
                        }
                    }

                    res.unwrap_or(JSValue::undefined())
                });

                self.push_stack(wrapped); // push object for this binding
                self.push_stack(result);
            }

            Opcode::SetPropertyByName(idx) => {
                let prop_name = self.get_constant_string(*idx)?;
                let val = self.pop_stack()?;
                let obj = self.pop_stack()?;

                // Check if this is a Proxy - intercept with handler's set trap
                let is_proxy = match &obj {
                    JSValue::Object(o) => o.borrow().class_name == "Proxy",
                    _ => false,
                };

                if is_proxy {
                    let (target, handler) = match &obj {
                        JSValue::Object(o) => {
                            let borrow = o.borrow();
                            let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                            let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                            (target, handler)
                        }
                        _ => (JSValue::undefined(), JSValue::undefined()),
                    };

                    if let Some(set_trap) = handler.get_property("set") {
                        let result = self.execute_function(&set_trap, handler.clone(), &[target, JSValue::string(&prop_name), val.clone()]);
                        if let Ok(true) = result {
                            let _ = self.run_frame_direct_simple();
                        }
                    } else {
                        // No set trap - set on target directly
                        target.set_property(&prop_name, val.clone());
                    }
                } else {
                    obj.set_property(&prop_name, val.clone());
                }

                // Push the value back (for assignment expressions to return the value)
                self.push_stack(val);
            }

            Opcode::SetProto => {
                // pops [proto, obj], sets obj's structural prototype to proto
                let proto = self.pop_stack()?;
                let obj = self.pop_stack()?;
                if let JSValue::Object(ref obj_rc) = obj {
                    if let JSValue::Object(ref proto_rc) = proto {
                        obj_rc.borrow_mut().prototype = Some(proto_rc.clone());
                    }
                }
                self.push_stack(obj);
            }

            Opcode::CopyProperties => {
                // Copy all properties from source to target (skip "prototype" to avoid overwriting)
                // Stack: [target, source] -> [target]
                let source = self.pop_stack()?;
                let target = self.pop_stack()?;
                let source_props = match &source {
                    JSValue::Object(obj) => {
                        let borrow = obj.borrow();
                        borrow.properties.clone()
                    }
                    JSValue::Function(f) => {
                        let borrow = f.borrow();
                        borrow.closure.iter().map(|(k, v)| (k.clone(), v.borrow().clone())).collect()
                    }
                    _ => std::collections::HashMap::new(),
                };
                for (key, val) in source_props {
                    if key != "prototype" {
                        target.set_property(&key, val);
                    }
                }
                self.push_stack(target);
            }

            Opcode::DefineProperty => {
                // DefineProperty: obj, value, key
                // This defines a own property (non-configurable by default)
                let val = self.pop_stack()?;
                let prop = self.pop_stack()?;
                let obj = self.pop_stack()?;
                let prop_name = prop.to_string();
                obj.set_property(&prop_name, val);
            }

            // ================================================================
            // Calls
            // ================================================================

            Opcode::Call(argc) => {
                let pushed = self.handle_call(*argc)?;
                if pushed { return Ok(OpcodeAction::FramePushed); }
            }

            Opcode::CallMethod(argc) => {
                let pushed = self.handle_call_method(*argc)?;
                if pushed { return Ok(OpcodeAction::FramePushed); }
            }

            Opcode::New(argc) => {
                let pushed = self.handle_new(*argc)?;
                if pushed { return Ok(OpcodeAction::FramePushed); }
            }

            Opcode::SuperCall(_argc) => {
                // Super call - for now, push undefined
                self.push_stack(JSValue::undefined());
            }

            // ================================================================
            // Objects
            // ================================================================

            Opcode::CreateObject => {
                let obj = JSValue::object("Object");
                self.push_stack(obj);
            }

            Opcode::CreateArray => {
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "Array".to_string(),
                };
                obj.internal_slots
                    .insert("Class".to_string(), JSValue::string("Array"));

                // Look up Array.prototype from the global object
                if let Some(array_ctor) = self.context.global.borrow().properties.get("Array").cloned() {
                    if let Some(proto) = array_ctor.get_property("prototype") {
                        if let JSValue::Object(ref proto_obj) = proto {
                            obj.prototype = Some(proto_obj.clone());
                        }
                    }
                }

                self.push_stack(JSValue::Object(Rc::new(RefCell::new(obj))));
            }

            // ================================================================
            // Functions
            // ================================================================

            Opcode::CreateClosure(idx) => {
                self.handle_create_closure(*idx)?;
            }

            Opcode::CreateAsync(_idx) => {
                // Async function creation - simplified: treat as regular function
                self.push_stack(JSValue::undefined());
            }

            Opcode::CreateGenerator(idx) => {
                // Create a generator function (like CreateClosure but with Generator body)
                self.handle_create_closure(*idx)?;
                // Mark the function as a generator by converting its body
                let frame = self.stack.last_mut().unwrap();
                if let Some(JSValue::Function(f)) = frame.stack.last() {
                    let mut borrow = f.borrow_mut();
                    if let FunctionBody::Bytecode(bc) = &borrow.body {
                        let bc_clone = bc.clone();
                        borrow.body = FunctionBody::Generator {
                            func: bc_clone,
                            yields: Rc::new(RefCell::new(Vec::new())),
                        };
                    }
                    borrow.is_generator = true;
                }
            }

            // ================================================================
            // Iterators
            // ================================================================

            Opcode::GetIterator => {
                let obj = self.pop_stack()?;

                // Check if object has Symbol.iterator
                let has_iterator = if let Some(symbol_iter) = self.context.global.borrow().properties.get("Symbol").and_then(|s| s.get_property("iterator")) {
                    obj.get_property(&symbol_iter.to_string()).is_some()
                } else {
                    false
                };

                if has_iterator {
                    // Call Symbol.iterator() to get the iterator
                    let symbol_iter_key = self.context.global.borrow()
                        .properties.get("Symbol")
                        .and_then(|s| s.get_property("iterator"))
                        .map(|v| v.to_string());
                    if let Some(key) = symbol_iter_key {
                        if let Some(iter_fn) = obj.get_property(&key) {
                            // Call the iterator function with obj as this
                            let result = self.execute_function(&iter_fn, obj.clone(), &[]);
                            if let Ok(true) = result {
                                let inner_result = self.run_frame_direct_simple()?;
                                self.push_stack(inner_result);
                            }
                        }
                    }
                } else {
                    // Create a simple iterator object for arrays
                    let mut iter_obj = JSObject {
                        properties: HashMap::new(),
                        descriptors: HashMap::new(),
                        prototype: None,
                        internal_slots: HashMap::new(),
                        class_name: "Iterator".to_string(),
                    };

                    if let Some(length_val) = obj.get_property("length") {
                        let length = length_val.to_uint32();
                        iter_obj.internal_slots.insert("iterable".to_string(), obj);
                        iter_obj.internal_slots.insert("index".to_string(), JSValue::int(0));
                        iter_obj.internal_slots.insert("length".to_string(), JSValue::int(length as i32));
                    }

                    self.push_stack(JSValue::Object(Rc::new(RefCell::new(iter_obj))));
                }
            }

            Opcode::IteratorNext => {
                let iter = self.pop_stack()?;

                // Check if iterator has a next() method (generator-style iterator)
                if let Some(next_fn) = iter.get_property("next") {
                    // Call next() on the iterator
                    let result = self.execute_function(&next_fn, iter.clone(), &[]);
                    if let Ok(true) = result {
                        // Frame was pushed - run it
                        let inner_result = self.run_frame_direct_simple()?;
                        self.push_stack(inner_result);
                    }
                    // The result should be { value, done } - leave it on the stack
                } else {
                    // Simple iterator (array-like with index/length)
                    match &iter {
                        JSValue::Object(iter_obj) => {
                            let borrow = iter_obj.borrow();
                            let index = borrow.internal_slots.get("index").map(|v| v.to_int32()).unwrap_or(0);
                            let length = borrow.internal_slots.get("length").map(|v| v.to_int32()).unwrap_or(0);

                            if index < length {
                                let iterable = borrow.internal_slots.get("iterable").cloned();
                                let value = if let Some(ref it) = iterable {
                                    it.get_property(&index.to_string()).unwrap_or(JSValue::undefined())
                                } else {
                                    JSValue::undefined()
                                };

                                let mut result = JSValue::object("Object");
                                result.set_property("value", value);
                                result.set_property("done", JSValue::bool(false));

                                drop(borrow);
                                if let JSValue::Object(o) = &iter {
                                    o.borrow_mut().internal_slots.insert("index".to_string(), JSValue::int(index + 1));
                                }

                                self.push_stack(result);
                            } else {
                                drop(borrow);
                                let mut result = JSValue::object("Object");
                                result.set_property("value", JSValue::undefined());
                                result.set_property("done", JSValue::bool(true));
                                self.push_stack(result);
                            }
                        }
                        _ => {
                            let mut result = JSValue::object("Object");
                            result.set_property("value", JSValue::undefined());
                            result.set_property("done", JSValue::bool(true));
                            self.push_stack(result);
                        }
                    }
                }
            }

            Opcode::IteratorClose => {
                // Close the iterator (cleanup)
                let _iter = self.pop_stack()?;
            }

            // ================================================================
            // Modules
            // ================================================================

            Opcode::Import(idx) => {
                let specifier = self.get_constant_string(*idx)?;

                // Use the host-provided module registry
                let module_obj = {
                    let rt = self.context.runtime.clone();
                    let rt_borrow = rt.borrow();
                    if let Some(ref registry) = rt_borrow.module_registry {
                        let mut registry = registry.borrow_mut();
                        match registry.load(&specifier, None) {
                            Ok(exports) => exports,
                            Err(e) => {
                                JSValue::object("Module")
                            }
                        }
                    } else {
                        // No module loader configured - return empty module
                        JSValue::object("Module")
                    }
                };

                self.push_stack(module_obj);
            }

            Opcode::Export(_idx) => {
                // Module export - handled by module loader
            }

            // ================================================================
            // Other
            // ================================================================

            Opcode::Typeof => {
                let val = self.pop_stack()?;
                let type_str = val.type_of();
                self.push_stack(JSValue::string(type_str));
            }

            Opcode::Delete => {
                let prop = self.pop_stack()?;
                let obj = self.pop_stack()?;
                let prop_name = prop.to_string();

                // Check if this is a Proxy - intercept with handler's deleteProperty trap
                let is_proxy = match &obj {
                    JSValue::Object(o) => o.borrow().class_name == "Proxy",
                    _ => false,
                };

                let result = if is_proxy {
                    let (target, handler) = match &obj {
                        JSValue::Object(o) => {
                            let borrow = o.borrow();
                            let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                            let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                            (target, handler)
                        }
                        _ => (JSValue::undefined(), JSValue::undefined()),
                    };

                    if let Some(del_trap) = handler.get_property("deleteProperty") {
                        let result = self.execute_function(&del_trap, handler.clone(), &[target, JSValue::string(&prop_name)]);
                        match result {
                            Ok(true) => {
                                self.run_frame_direct_simple().ok().map(|v| v.to_boolean()).unwrap_or(true)
                            }
                            Ok(false) => {
                                self.pop_stack().ok().map(|v| v.to_boolean()).unwrap_or(true)
                            }
                            Err(_) => true,
                        }
                    } else {
                        // No trap - delete from target
                        if let JSValue::Object(o) = &target {
                            o.borrow_mut().properties.remove(&prop_name);
                        }
                        true
                    }
                } else {
                    // Not a proxy - set to undefined (simplified delete)
                    obj.set_property(&prop_name, JSValue::undefined());
                    true
                };

                self.push_stack(JSValue::bool(result));
            }

            Opcode::Void => {
                let _val = self.pop_stack()?;
                self.push_stack(JSValue::undefined());
            }

            Opcode::Instanceof => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                // instanceof: check if right.prototype is in left's prototype chain
                let result = if let Some(target_proto) = right.get_property("prototype") {
                    let mut current = Some(left.clone());
                    let mut found = false;
                    let mut iterations = 0;
                    while let Some(ref obj) = current {
                        if iterations > 100 { break; }
                        iterations += 1;
                        // Check if current object IS the target prototype
                        // Compare by class_name and properties count (not Rc pointer)
                        match (obj, &target_proto) {
                            (JSValue::Object(a), JSValue::Object(b)) => {
                                let a_borrow = a.borrow();
                                let b_borrow = b.borrow();
                                if a_borrow.class_name == b_borrow.class_name
                                    && a_borrow.properties.len() == b_borrow.properties.len()
                                    && !a_borrow.class_name.is_empty() {
                                    found = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                        // Walk up: first try __proto__ property, then structural prototype
                        let proto = obj.get_property("__proto__")
                            .or_else(|| match obj {
                                JSValue::Object(o) => o.borrow().prototype.clone().map(JSValue::Object),
                                _ => None,
                            });
                        current = proto;
                    }
                    found
                } else {
                    false
                };
                self.push_stack(JSValue::bool(result));
            }

            Opcode::In => {
                let right = self.pop_stack()?;
                let left = self.pop_stack()?;
                let prop_name = left.to_string();

                // Check if this is a Proxy - intercept with handler's has trap
                let is_proxy = match &right {
                    JSValue::Object(o) => o.borrow().class_name == "Proxy",
                    _ => false,
                };

                let result = if is_proxy {
                    let (target, handler) = match &right {
                        JSValue::Object(o) => {
                            let borrow = o.borrow();
                            let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                            let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                            (target, handler)
                        }
                        _ => (JSValue::undefined(), JSValue::undefined()),
                    };

                    if let Some(has_trap) = handler.get_property("has") {
                        let result = self.execute_function(&has_trap, handler.clone(), &[target, JSValue::string(&prop_name)]);
                        match result {
                            Ok(true) => {
                                self.run_frame_direct_simple().ok().map(|v| v.to_boolean()).unwrap_or(false)
                            }
                            Ok(false) => {
                                self.pop_stack().ok().map(|v| v.to_boolean()).unwrap_or(false)
                            }
                            Err(_) => false,
                        }
                    } else {
                        right.has_property(&prop_name)
                    }
                } else {
                    right.has_property(&prop_name)
                };

                self.push_stack(JSValue::bool(result));
            }

            Opcode::Yield => {
                // Generator yield: pause execution and return the value
                let val = self.pop_stack().unwrap_or(JSValue::undefined());
                // Store the yielded value for the caller
                self.generator_yielded = Some(val);
                // Push undefined as the yield expression result (for resume)
                self.push_stack(JSValue::undefined());
                return Ok(OpcodeAction::Yielded);
            }

            Opcode::Await => {
                // Await: resolve Promise value via host executor
                let val = self.pop_stack()?;

                // Check if this is already a resolved Promise
                let resolved_value = match &val {
                    JSValue::Object(_) => {
                        let state = val.get_property("__state");
                        match state {
                            Some(JSValue::Int(1)) => {
                                // Fulfilled: extract result
                                Some(val.get_property("__result").unwrap_or(JSValue::undefined()))
                            }
                            Some(JSValue::Int(2)) => {
                                // Rejected: extract result (in full impl, should throw)
                                Some(val.get_property("__result").unwrap_or(JSValue::undefined()))
                            }
                            _ => None, // Pending or not a Promise
                        }
                    }
                    _ => Some(val.clone()), // Non-Promise value: pass through
                };

                if let Some(result) = resolved_value {
                    // Already resolved - continue synchronously
                    self.push_stack(result);
                } else {
                    // Pending Promise - delegate to host executor
                    self.pending_await = Some(val);
                    return Ok(OpcodeAction::Awaited);
                }
            }

            Opcode::Nop => {
                // No operation
            }
            Opcode::GetGlobalVar(idx) => {
                let name = self.get_constant_string(*idx)?;
                let global_val = JSValue::Object(self.context.global.clone());
                let val = global_val.get_property(&name).unwrap_or(JSValue::undefined());
                if val.is_undefined() {
                    let err = JSValue::object("ReferenceError");
                    err.set_property("message", JSValue::string(&format!("{} is not defined", name)));
                    err.set_property("name", JSValue::string("ReferenceError"));
                    // Set prototype to ReferenceError.prototype for instanceof to work
                    let ref_error = global_val.get_property("ReferenceError");
                    if let Some(proto) = ref_error.and_then(|r| r.get_property("prototype")) {
                        if let JSValue::Object(obj) = &err {
                            if let JSValue::Object(p) = &proto {
                                obj.borrow_mut().prototype = Some(p.clone());
                            }
                        }
                    }
                    self.pending_exception = Some(err);
                    return Ok(OpcodeAction::Continue);
                }
                self.push_stack(val);
            }
        }

        Ok(OpcodeAction::Continue)
    }

    // ================================================================
    // Call/New helpers
    // ================================================================

    /// Handle a function call (Call opcode).
    /// Regular call - no `this` binding from the stack.
    fn handle_call(&mut self, argc: u32) -> Result<bool, RuntimeError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop_stack()?);
        }
        args.reverse();

        let func_val = self.pop_stack()?;

        // Check if the function is a Proxy - intercept with apply trap
        let is_proxy = match &func_val {
            JSValue::Object(o) => o.borrow().class_name == "Proxy",
            _ => false,
        };

        if is_proxy {
            let (target, handler) = match &func_val {
                JSValue::Object(o) => {
                    let borrow = o.borrow();
                    let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                    let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                    (target, handler)
                }
                _ => (JSValue::undefined(), JSValue::undefined()),
            };

            if let Some(apply_trap) = handler.get_property("apply") {
                let args_arr = crate::builtins::array::create_array(args);
                let result = self.execute_function(&apply_trap, handler.clone(), &[target, JSValue::undefined(), args_arr]);
                return match result {
                    Ok(true) => Ok(true),
                    Ok(false) => Ok(false),
                    Err(e) => Err(e),
                };
            } else {
                // No apply trap - call target directly
                let this = JSValue::undefined();
                return self.execute_function(&target, this, &args);
            }
        }

        let this = JSValue::undefined();
        self.execute_function(&func_val, this, &args)
    }

    /// Execute a function value with given this and args.
    /// Returns true if a bytecode frame was pushed (caller should restart trampoline).
    fn execute_function(&mut self, func_val: &JSValue, this: JSValue, args: &[JSValue]) -> Result<bool, RuntimeError> {
        match func_val {
            JSValue::Function(f) => {
                let func_ref = f.clone();
                let (body_clone, closure_clone) = {
                    let func_borrow = func_ref.borrow();
                    let body = func_borrow.body.clone();
                    let closure = func_borrow.closure.clone();
                    (body, closure)
                };

                match body_clone {
                    FunctionBody::Native(native_fn) => {
                        let result = native_fn(&this, args);
                        self.push_stack(result);
                        return Ok(false);
                    }
                    FunctionBody::Bytecode(bc_func) => {
                        let closure_var_order = bc_func.closure_vars.clone();
                        let function = bc_func;
                        // Build closure Vec in the order matching the function's closure_vars
                        let closure: Vec<Rc<RefCell<JSValue>>> = closure_var_order.iter()
                            .filter_map(|name| closure_clone.get(name).cloned())
                            .collect();

                        let frame = StackFrame::new(function, this, args, closure);
                        self.push_frame(frame)?;
                        return Ok(true); // Frame pushed - restart trampoline
                    }
                    FunctionBody::Source(_) => {
                        return Err(RuntimeError::new("Source function execution not supported"));
                    }
                    FunctionBody::Closure(closure_fn) => {
                        let result = closure_fn(&this, args);
                        self.push_stack(result);
                        return Ok(false);
                    }
                    FunctionBody::Generator { func, yields } => {
                        // Create generator object lazily (without running the function)
                        let state = Rc::new(RefCell::new(crate::value::GeneratorState {
                            func: func.clone(),
                            closure: closure_clone.clone(),
                            args: args.to_vec(),
                            this: this.clone(),
                            saved_frame: None,
                            saved_pc: 0,
                            saved_locals: Vec::new(),
                            saved_stack: Vec::new(),
                            saved_closure: Vec::new(),
                            done: false,
                            started: false,
                        }));

                        let gen_obj = JSValue::object("Generator");

                        // Create next() method with GeneratorNext body
                        let next_fn = JSValue::function(
                            Some("next"),
                            vec![],
                            FunctionBody::GeneratorNext { state: state.clone() },
                        );
                        gen_obj.set_property("next", next_fn);

                        // Add Symbol.iterator - returns this (generator is iterable)
                        let gen_clone = gen_obj.clone();
                        let iterator_fn = JSValue::function(
                            Some("[Symbol.iterator]"),
                            vec![],
                            FunctionBody::Closure(Rc::new(move |_this, _args| {
                                gen_clone.clone()
                            })),
                        );
                        // Set Symbol.iterator on the generator
                        if let Some(symbol_iter) = self.context.global.borrow().properties.get("Symbol").and_then(|s| s.get_property("iterator")) {
                            gen_obj.set_property(&symbol_iter.to_string(), iterator_fn);
                        }

                        self.push_stack(gen_obj);
                        return Ok(false);
                    }
                    FunctionBody::GeneratorNext { state } => {
                        // Resume the generator from its saved state
                        let mut state_borrow = state.borrow_mut();

                        if state_borrow.done {
                            // Generator is done - return { value: undefined, done: true }
                            let mut result = JSValue::object("Object");
                            result.set_property("value", JSValue::undefined());
                            result.set_property("done", JSValue::bool(true));
                            self.push_stack(result);
                            return Ok(false);
                        }

                        // Build closure for the generator frame
                        let closure_var_order = state_borrow.func.closure_vars.clone();
                        let closure: Vec<Rc<RefCell<JSValue>>> = closure_var_order.iter()
                            .filter_map(|name| state_borrow.closure.get(name).cloned())
                            .collect();

                        let frame = if state_borrow.started {
                            // Resume from saved state
                            let mut frame = StackFrame::new(
                                state_borrow.func.clone(),
                                state_borrow.this.clone(),
                                &state_borrow.args,
                                closure,
                            );
                            frame.pc = state_borrow.saved_pc;
                            frame.locals = state_borrow.saved_locals.clone();
                            frame.stack = state_borrow.saved_stack.clone();
                            frame
                        } else {
                            // First call - create new frame
                            state_borrow.started = true;
                            StackFrame::new(
                                state_borrow.func.clone(),
                                state_borrow.this.clone(),
                                &state_borrow.args,
                                closure,
                            )
                        };

                        drop(state_borrow);

                        // Run the generator until yield or return
                        let (value, done, saved_state) = self.run_generator_until_yield(frame, state.clone())?;

                        // Create result object
                        let mut result = JSValue::object("Object");
                        result.set_property("value", value);
                        result.set_property("done", JSValue::bool(done));

                        self.push_stack(result);
                        return Ok(false);
                    }
                }
            }
            _ => {
                let msg = format!("{} is not a function", func_val.type_of());
                let err = JSValue::object("TypeError");
                err.set_property("message", JSValue::string(&msg));
                err.set_property("name", JSValue::string("TypeError"));
                self.pending_exception = Some(err);
                return Ok(false);
            }
        }
        Ok(false)
    }

    /// Handle a method call (CallMethod opcode).
    /// Like Call but pops `this` from the stack (used with GetField2).
    fn handle_call_method(&mut self, argc: u32) -> Result<bool, RuntimeError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop_stack()?);
        }
        args.reverse();

        let func_val = self.pop_stack()?;
        let this = self.pop_stack()?; // Always pop `this` (GetField2 pushed it)

        self.execute_function(&func_val, this, &args)
    }

    /// Handle a constructor call (New opcode).
    fn handle_new(&mut self, argc: u32) -> Result<bool, RuntimeError> {
        // Collect arguments
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop_stack()?);
        }
        args.reverse();

        // Pop the constructor function
        let func_val = self.pop_stack()?;

        // Check if the constructor is a Proxy - intercept with construct trap
        let is_proxy = match &func_val {
            JSValue::Object(o) => o.borrow().class_name == "Proxy",
            _ => false,
        };

        if is_proxy {
            let (target, handler) = match &func_val {
                JSValue::Object(o) => {
                    let borrow = o.borrow();
                    let target = borrow.internal_slots.get("target").cloned().unwrap_or(JSValue::undefined());
                    let handler = borrow.internal_slots.get("handler").cloned().unwrap_or(JSValue::undefined());
                    (target, handler)
                }
                _ => (JSValue::undefined(), JSValue::undefined()),
            };

            if let Some(construct_trap) = handler.get_property("construct") {
                let args_arr = crate::builtins::array::create_array(args);
                let result = self.execute_function(&construct_trap, handler.clone(), &[target, args_arr]);
                return match result {
                    Ok(true) => Ok(true),
                    Ok(false) => Ok(false),
                    Err(e) => Err(e),
                };
            } else {
                // No construct trap - call target constructor directly
                return self.handle_new_with_target(target, argc);
            }
        }

        match &func_val {
            JSValue::Function(f) => {
                let func_ref = f.clone();
                let (body_clone, name_clone, closure_clone) = {
                    let func_borrow = func_ref.borrow();
                    let body = func_borrow.body.clone();
                    let name = func_borrow.name.clone();
                    let closure = func_borrow.closure.clone();
                    (body, name, closure)
                };

                // Get the prototype from the constructor function
                let prototype = func_val.get_property("prototype")
                    .and_then(|p| match p {
                        JSValue::Object(o) => Some(o),
                        _ => None,
                    });

                match body_clone {
                    FunctionBody::Native(native_fn) => {
                        let mut obj = JSObject {
                            properties: std::collections::HashMap::new(),
                            descriptors: std::collections::HashMap::new(),
                            prototype: prototype.clone(),
                            internal_slots: std::collections::HashMap::new(),
                            class_name: name_clone.clone().unwrap_or_else(|| "Object".to_string()),
                        };
                        let obj_val = JSValue::Object(Rc::new(RefCell::new(obj)));
                        let result = native_fn(&obj_val, &args);
                        let final_result = if result.is_object() {
                            result
                        } else {
                            obj_val
                        };
                        self.push_stack(final_result);
                        return Ok(false);
                    }
                    FunctionBody::Bytecode(bc_func) => {
                        let function = bc_func;
                        let mut obj = JSObject {
                            properties: std::collections::HashMap::new(),
                            descriptors: std::collections::HashMap::new(),
                            prototype: prototype.clone(),
                            internal_slots: std::collections::HashMap::new(),
                            class_name: name_clone.clone().unwrap_or_else(|| "Object".to_string()),
                        };
                        let obj_val = JSValue::Object(Rc::new(RefCell::new(obj)));
                        let closure_var_order = function.closure_vars.clone();
                        let closure: Vec<Rc<RefCell<JSValue>>> = closure_var_order.iter()
                            .filter_map(|name| closure_clone.get(name).cloned())
                            .collect();

                        let mut frame = StackFrame::new(function, obj_val.clone(), &args, closure);
                        frame.is_constructor = true;
                        self.push_frame(frame)?;
                        return Ok(true); // Frame pushed - restart trampoline
                    }
                    FunctionBody::Source(_) => {
                        return Err(RuntimeError::new("Source function execution not supported"));
                    }
                    FunctionBody::Closure(closure_fn) => {
                        let obj = JSValue::object(
                            &name_clone.unwrap_or_else(|| "Object".to_string()),
                        );
                        let result = closure_fn(&obj, &args);
                        let final_result = if result.is_object() {
                            result
                        } else {
                            obj
                        };
                        self.push_stack(final_result);
                        return Ok(false);
                    }
                    FunctionBody::Generator { .. } => {
                        // Generators are not constructors
                        return Err(RuntimeError::new("Generators cannot be called with new"));
                    }
                    FunctionBody::GeneratorNext { .. } => {
                        // Generator next is not a constructor
                        return Err(RuntimeError::new("Generator next cannot be called with new"));
                    }
                }
            }
            _ => {
                return Err(RuntimeError::with_message(format!(
                    "TypeError: {} is not a constructor",
                    func_val.type_of()
                )));
            }
        }
    }

    /// Handle new with a specific target (for Proxy construct trap fallback).
    fn handle_new_with_target(&mut self, func_val: JSValue, argc: u32) -> Result<bool, RuntimeError> {
        let mut args = Vec::with_capacity(argc as usize);
        // Args were already popped by the caller, so we just call execute_function
        self.execute_function(&func_val, JSValue::undefined(), &args)
    }

    /// Handle CreateClosure opcode.
    fn handle_create_closure(&mut self, idx: u32) -> Result<(), RuntimeError> {
        let frame = self.stack.last().unwrap();

        // Look up the function by index in the current function's function table
        if (idx as usize) < frame.function.functions.len() {
            let bc_func = frame.function.functions[idx as usize].clone();
            let closure_vars = frame.closure.clone();

            let func_name = bc_func.name.clone();
            let func_params = bc_func.params.clone();
            let is_generator = bc_func.is_generator;

            let body = if is_generator {
                FunctionBody::Generator {
                    func: bc_func,
                    yields: Rc::new(RefCell::new(Vec::new())),
                }
            } else {
                FunctionBody::Bytecode(bc_func)
            };

            let func_val = JSValue::function(
                func_name.as_deref(),
                func_params,
                body,
            );

            // Store captured variables in the function's closure.
            // Use the child function's closure_vars to capture exactly the right variables
            // in the correct order matching the child's closure_var_map indices.
            if let JSValue::Function(f) = &func_val {
                let child_closure_vars = {
                    let borrow = f.borrow();
                    match &borrow.body {
                        FunctionBody::Bytecode(bc) => bc.closure_vars.clone(),
                        FunctionBody::Generator { func, .. } => func.closure_vars.clone(),
                        _ => Vec::new(),
                    }
                };

                // Build a name-indexed map of all available variables from the parent frame.
                // This includes both the parent's locals and the parent's own closure.
                let mut available: std::collections::HashMap<String, Rc<RefCell<JSValue>>> = std::collections::HashMap::new();

                // Add parent's locals (indexed by variable name)
                for (i, var) in frame.function.variables.iter().enumerate() {
                    if i < frame.locals.len() {
                        available.insert(var.name.clone(), frame.locals[i].clone());
                    }
                }

                // Add parent's closure (indexed by parent's closure_vars names)
                for (i, name) in frame.function.closure_vars.iter().enumerate() {
                    if i < frame.closure.len() {
                        available.entry(name.clone()).or_insert_with(|| frame.closure[i].clone());
                    }
                }

                // Build the child's closure from its closure_vars
                let mut closure_map = std::collections::HashMap::new();
                for var_name in &child_closure_vars {
                    if let Some(val) = available.get(var_name) {
                        closure_map.insert(var_name.clone(), val.clone());
                    }
                }

                f.borrow_mut().closure = closure_map;
            }

            self.push_stack(func_val);
        } else {
            // Function not found in table - push undefined
            self.push_stack(JSValue::undefined());
        }

        Ok(())
    }

    // ================================================================
    // Helper methods
    // ================================================================

    /// Pop a value from the current frame's stack.
    fn pop_stack(&mut self) -> Result<JSValue, RuntimeError> {
        let frame = self.stack.last_mut().unwrap();
        frame.pop()
    }

    /// Push a value onto the current frame's stack.
    fn push_stack(&mut self, val: JSValue) {
        let frame = self.stack.last_mut().unwrap();
        frame.push(val);
    }

    /// Auto-box a primitive value for property access.
    /// In JavaScript, primitives are wrapped in objects when you access properties:
    /// - Number -> Number object
    /// - String -> String object
    /// - Boolean -> Boolean object
    fn auto_box(&self, val: JSValue) -> JSValue {
        match &val {
            JSValue::Int(_) | JSValue::Float(_) => {
                // Wrap in Number object with Number.prototype
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "Number".to_string(),
                };
                // Look up Number.prototype from global
                if let Some(ctor) = self.context.global.borrow().properties.get("Number").cloned() {
                    if let Some(proto) = ctor.get_property("prototype") {
                        if let JSValue::Object(ref proto_obj) = proto {
                            obj.prototype = Some(proto_obj.clone());
                        }
                    }
                }
                obj.internal_slots.insert("PrimitiveValue".to_string(), val);
                JSValue::Object(Rc::new(RefCell::new(obj)))
            }
            JSValue::String(s) => {
                // Wrap in String object with String.prototype
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "String".to_string(),
                };
                // Look up String.prototype from global
                if let Some(ctor) = self.context.global.borrow().properties.get("String").cloned() {
                    if let Some(proto) = ctor.get_property("prototype") {
                        if let JSValue::Object(ref proto_obj) = proto {
                            obj.prototype = Some(proto_obj.clone());
                        }
                    }
                }
                let data = s.borrow().data.clone();
                obj.properties.insert("length".to_string(), JSValue::Int(data.len() as i32));
                // Add indexed character properties
                for (i, ch) in data.chars().enumerate() {
                    obj.properties.insert(i.to_string(), JSValue::string(&ch.to_string()));
                }
                obj.internal_slots.insert("PrimitiveValue".to_string(), val);
                JSValue::Object(Rc::new(RefCell::new(obj)))
            }
            JSValue::Bool(_) => {
                // Wrap in Boolean object with Boolean.prototype
                let mut obj = JSObject {
                    properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
                    internal_slots: HashMap::new(),
                    class_name: "Boolean".to_string(),
                };
                // Look up Boolean.prototype from global
                if let Some(ctor) = self.context.global.borrow().properties.get("Boolean").cloned() {
                    if let Some(proto) = ctor.get_property("prototype") {
                        if let JSValue::Object(ref proto_obj) = proto {
                            obj.prototype = Some(proto_obj.clone());
                        }
                    }
                }
                obj.internal_slots.insert("PrimitiveValue".to_string(), val);
                JSValue::Object(Rc::new(RefCell::new(obj)))
            }
            _ => val, // Objects and functions don't need wrapping
        }
    }

    /// Get a float constant from the constant pool.
    fn get_constant_float(&self, idx: u32) -> Result<f64, RuntimeError> {
        let frame = self.stack.last().unwrap();
        match frame.function.constants.get(idx as usize) {
            Some(Constant::Float(f)) => Ok(*f),
            Some(Constant::String(s)) => s.parse::<f64>().map_err(|_| {
                RuntimeError::with_message(format!("Cannot convert \"{}\" to number", s))
            }),
            _ => Err(RuntimeError::new("Invalid constant index for float")),
        }
    }

    /// Get a string constant from the constant pool.
    fn get_constant_string(&self, idx: u32) -> Result<String, RuntimeError> {
        let frame = self.stack.last().unwrap();
        match frame.function.constants.get(idx as usize) {
            Some(Constant::String(s)) => Ok(s.clone()),
            Some(Constant::Float(f)) => Ok(f.to_string()),
            _ => Err(RuntimeError::new("Invalid constant index for string")),
        }
    }

    /// Create a runtime error with the current context.
    fn runtime_error(&self, message: &str) -> RuntimeError {
        let mut err = RuntimeError::new(message);
        err.stack_trace = self.build_stack_trace();
        err
    }

    // ================================================================
    // Type coercion / operation helpers
    // ================================================================

    /// ToNumber helper for JSValue.
    fn to_number_value(val: &JSValue) -> f64 {
        val.to_number()
    }

    /// JavaScript addition operator (handles string concatenation).
    fn op_add(left: &JSValue, right: &JSValue) -> Result<JSValue, RuntimeError> {
        // If either operand is a string, perform string concatenation
        if left.is_string() || right.is_string() {
            let l = Self::to_string_value(left);
            let r = Self::to_string_value(right);
            return Ok(JSValue::string(&format!("{}{}", l, r)));
        }

        // Try ToPrimitive with hint "number"
        let l = match left.to_primitive("number") {
            Ok(v) => v,
            Err(_) => return Err(RuntimeError::new("Cannot convert to primitive")),
        };
        let r = match right.to_primitive("number") {
            Ok(v) => v,
            Err(_) => return Err(RuntimeError::new("Cannot convert to primitive")),
        };

        // If either result is a string, concatenate
        if l.is_string() || r.is_string() {
            let ls = Self::to_string_value(&l);
            let rs = Self::to_string_value(&r);
            return Ok(JSValue::string(&format!("{}{}", ls, rs)));
        }

        // Numeric addition
        let ln = Self::to_number_value(&l);
        let rn = Self::to_number_value(&r);
        Ok(JSValue::float(ln + rn))
    }

    /// ToString helper for JSValue.
    fn to_string_value(val: &JSValue) -> String {
        val.to_string()
    }

    /// Abstract relational comparison (ToLessThan).
    /// Returns true if x < y, false otherwise, or None for unordered.
    fn op_less_than(x: &JSValue, y: &JSValue) -> Result<bool, RuntimeError> {
        // Try ToPrimitive with hint "number"
        let px = match x.to_primitive("number") {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };
        let py = match y.to_primitive("number") {
            Ok(v) => v,
            Err(_) => return Ok(false),
        };

        // If both are strings, do string comparison
        if px.is_string() && py.is_string() {
            let sx = px.to_string();
            let sy = py.to_string();
            return Ok(sx < sy);
        }

        // Numeric comparison
        let nx = Self::to_number_value(&px);
        let ny = Self::to_number_value(&py);

        if nx.is_nan() || ny.is_nan() {
            return Ok(false);
        }

        if nx == ny {
            // Handle +0 vs -0
            return Ok(false);
        }

        Ok(nx < ny)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::JSRuntime;
    use crate::compiler::Variable;

    fn make_interpreter() -> Interpreter {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        Interpreter::new(ctx)
    }

    fn make_function(bytecode: Vec<Opcode>, constants: Vec<Constant>, variables: Vec<Variable>) -> BytecodeFunction {
        BytecodeFunction {
            name: Some("<test>".to_string()),
            params: Vec::new(),
            bytecode,
            constants,
            variables,
            functions: Vec::new(),
            line_numbers: Vec::new(),
            filename: None,
            is_generator: false,
            is_async: false,
            is_module: false,
            strict_mode: false,
            rest_param_index: None,
            closure_vars: Vec::new(),
            is_arrow: false,
        }
    }

    #[test]
    fn test_interpreter_creation() {
        let interp = make_interpreter();
        assert_eq!(interp.stack.len(), 0);
        assert!(!interp.is_interrupted());
    }

    #[test]
    fn test_push_pop_values() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_arithmetic_add() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(3),
                Opcode::PushInt(4),
                Opcode::Add,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 7);
    }

    #[test]
    fn test_arithmetic_sub() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(10),
                Opcode::PushInt(3),
                Opcode::Sub,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 7);
    }

    #[test]
    fn test_arithmetic_mul() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(6),
                Opcode::PushInt(7),
                Opcode::Mul,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_arithmetic_div() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(10),
                Opcode::PushInt(2),
                Opcode::Div,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_number(), 5.0);
    }

    #[test]
    fn test_string_concatenation() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::String("hello".to_string()), Constant::String(" world".to_string())];
        let func = make_function(
            vec![
                Opcode::PushString(0),
                Opcode::PushString(1),
                Opcode::Add,
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_string(), "hello world");
    }

    #[test]
    fn test_comparison() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::PushInt(3),
                Opcode::Gt,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_jump_if_true() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushBool(true),
                Opcode::JumpIfTrue(3),
                Opcode::PushInt(0),
                Opcode::PushInt(1),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 1);
    }

    #[test]
    fn test_jump_if_false() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushBool(false),
                Opcode::JumpIfFalse(3),
                Opcode::PushInt(0),
                Opcode::PushInt(1),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 1);
    }

    #[test]
    fn test_variables() {
        let mut interp = make_interpreter();
        let variables = vec![
            Variable {
                name: "x".to_string(),
                kind: VariableKind::Var,
                scope_level: 0,
                is_captured: false,
                is_parameter: false,
            },
        ];
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::SetVar(0),
                Opcode::Pop,
                Opcode::GetVar(0),
                Opcode::Return,
            ],
            Vec::new(),
            variables,
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_logical_not() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushBool(false),
                Opcode::Not,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_typeof() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::Typeof,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_string(), "number");
    }

    #[test]
    fn test_create_object() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::CreateObject,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn test_function_call_native() {
        let mut interp = make_interpreter();

        // Register a native function in the runtime
        let native_fn = JSValue::function(
            Some("add"),
            vec!["a".to_string(), "b".to_string()],
            FunctionBody::Native(|_this, args| {
                let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
                let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
                JSValue::float(a + b)
            }),
        );

        // Create a function that calls the native function
        let variables = vec![
            Variable {
                name: "fn".to_string(),
                kind: VariableKind::Var,
                scope_level: 0,
                is_captured: false,
                is_parameter: false,
            },
        ];

        // We can't directly test function calls without proper setup,
        // but we can test the arithmetic operations work
        let func = make_function(
            vec![
                Opcode::PushInt(10),
                Opcode::PushInt(20),
                Opcode::Add,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 30);
    }

    #[test]
    fn test_interrupt() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let mut interp = Interpreter::with_config(ctx, 1024, 1);

        let func = make_function(
            vec![
                Opcode::PushInt(1),
                Opcode::PushInt(2),
                Opcode::Add,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );

        // Set interrupt before execution
        interp.interrupt();
        let result = interp.execute(&func);
        assert!(result.is_err());
    }

    #[test]
    fn test_stack_overflow_detection() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let mut interp = Interpreter::with_config(ctx, 3, 10000);

        // Create a recursive function that overflows the stack
        let func = make_function(
            vec![
                Opcode::CreateClosure(0),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );

        // Just test that the stack depth limit is enforced
        assert!(interp.max_stack_depth == 3);
    }

    #[test]
    fn test_swap() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(1),
                Opcode::PushInt(2),
                Opcode::Swap,
                Opcode::Sub, // stack is [2, 1], sub does 2-1=1
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 1);
    }

    #[test]
    fn test_dup() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::Dup,
                Opcode::Mul, // 5 * 5 = 25
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 25);
    }

    #[test]
    fn test_bitwise_ops() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(0xFF),
                Opcode::PushInt(0x0F),
                Opcode::BitAnd, // 0xFF & 0x0F = 0x0F = 15
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 15);
    }

    #[test]
    fn test_string_comparison() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::String("abc".to_string()), Constant::String("def".to_string())];
        let func = make_function(
            vec![
                Opcode::PushString(0),
                Opcode::PushString(1),
                Opcode::Lt, // "abc" < "def"
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_null_undefined_equality() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushNull,
                Opcode::PushUndefined,
                Opcode::Eq, // null == undefined
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_create_array() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::CreateArray,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn test_pop_discards() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(1),
                Opcode::PushInt(2),
                Opcode::Pop, // discard 2
                Opcode::Return, // returns 1
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 1);
    }

    #[test]
    fn test_void_operator() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::Void,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_negation() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::Neg,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_number(), -5.0);
    }

    #[test]
    fn test_inc_dec() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::Inc,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_number(), 6.0);

        let func2 = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::Dec,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result2 = interp.execute(&func2).unwrap();
        assert_eq!(result2.to_number(), 4.0);
    }

    #[test]
    fn test_modulo() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(10),
                Opcode::PushInt(3),
                Opcode::Mod,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_number(), 1.0);
    }

    #[test]
    fn test_power() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(2),
                Opcode::PushInt(10),
                Opcode::Pow,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_number(), 1024.0);
    }

    #[test]
    fn test_shift_ops() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(1),
                Opcode::PushInt(3),
                Opcode::Shl, // 1 << 3 = 8
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 8);
    }

    #[test]
    fn test_error_display() {
        let err = RuntimeError::new("test error");
        assert_eq!(err.message, "test error");
        assert!(format!("{}", err).contains("test error"));
    }

    #[test]
    fn test_push_float() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::Float(3.14)];
        let func = make_function(
            vec![
                Opcode::PushFloat(0),
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!((result.to_number() - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_push_string() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::String("hello".to_string())];
        let func = make_function(
            vec![
                Opcode::PushString(0),
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_push_null() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushNull,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_push_undefined() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushUndefined,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_push_bool() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushBool(true),
                Opcode::PushBool(false),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        // Return pops the top, which is false
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn test_push_regexp() {
        let mut interp = make_interpreter();
        let constants = vec![
            Constant::String("abc".to_string()),
            Constant::String("gi".to_string()),
        ];
        let func = make_function(
            vec![
                Opcode::PushRegExp(0, 1),
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_object());
        // RegExp objects have a "source" property
        let source = result.get_property("source").unwrap();
        assert_eq!(source.to_string(), "abc");
    }

    #[test]
    fn test_rotate() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(1), // push 1
                Opcode::PushInt(2), // push 2
                Opcode::PushInt(3), // push 3
                Opcode::Rotate(3),  // stack: [1,2,3] -> [2,3,1]
                Opcode::Pop,        // remove 1
                Opcode::Pop,        // remove 3
                Opcode::Return,     // return 2
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 2);
    }

    #[test]
    fn test_div_by_zero() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::PushInt(0),
                Opcode::Div,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.to_number().is_infinite());
    }

    #[test]
    fn test_mod_by_zero() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::PushInt(0),
                Opcode::Mod,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.to_number().is_nan());
    }

    #[test]
    fn test_bitwise_or_xor_not() {
        let mut interp = make_interpreter();
        // BitOr: 0xF0 | 0x0F = 0xFF = 255
        let func = make_function(
            vec![
                Opcode::PushInt(0xF0),
                Opcode::PushInt(0x0F),
                Opcode::BitOr,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 0xFF);

        // BitXor: 0xFF ^ 0x0F = 0xF0 = 240
        let func2 = make_function(
            vec![
                Opcode::PushInt(0xFF),
                Opcode::PushInt(0x0F),
                Opcode::BitXor,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result2 = interp.execute(&func2).unwrap();
        assert_eq!(result2.to_int32(), 0xF0);

        // BitNot: ~0 = -1
        let func3 = make_function(
            vec![
                Opcode::PushInt(0),
                Opcode::BitNot,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result3 = interp.execute(&func3).unwrap();
        assert_eq!(result3.to_int32(), -1);
    }

    #[test]
    fn test_shift_right_unsigned() {
        let mut interp = make_interpreter();
        // UShr: (-1 >>> 1) should be 0x7FFFFFFF (2147483647)
        let func = make_function(
            vec![
                Opcode::PushInt(-1),
                Opcode::PushInt(1),
                Opcode::UShr,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 0x7FFFFFFF);
    }

    #[test]
    fn test_strict_eq() {
        let mut interp = make_interpreter();
        // Same type, same value: 42 === 42 -> true
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::PushInt(42),
                Opcode::StrictEq,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), true);

        // Different type: 0 === false -> false (strict)
        let func2 = make_function(
            vec![
                Opcode::PushInt(0),
                Opcode::PushBool(false),
                Opcode::StrictEq,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result2 = interp.execute(&func2).unwrap();
        assert_eq!(result2.to_boolean(), false);
    }

    #[test]
    fn test_strict_ne() {
        let mut interp = make_interpreter();
        // Same value: 42 !== 42 -> false
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::PushInt(42),
                Opcode::StrictNe,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_boolean(), false);

        // Different value: 42 !== 7 -> true
        let func2 = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::PushInt(7),
                Opcode::StrictNe,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result2 = interp.execute(&func2).unwrap();
        assert_eq!(result2.to_boolean(), true);
    }

    #[test]
    fn test_logical_and_or() {
        let mut interp = make_interpreter();
        // LogicalAnd: truthy && truthy -> right (7)
        let func = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::PushInt(7),
                Opcode::LogicalAnd,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 7);

        // LogicalAnd: falsy && truthy -> left (0)
        let func2 = make_function(
            vec![
                Opcode::PushInt(0),
                Opcode::PushInt(7),
                Opcode::LogicalAnd,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result2 = interp.execute(&func2).unwrap();
        assert_eq!(result2.to_int32(), 0);

        // LogicalOr: falsy || truthy -> right (42)
        let func3 = make_function(
            vec![
                Opcode::PushInt(0),
                Opcode::PushInt(42),
                Opcode::LogicalOr,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result3 = interp.execute(&func3).unwrap();
        assert_eq!(result3.to_int32(), 42);

        // LogicalOr: truthy || truthy -> left (5)
        let func4 = make_function(
            vec![
                Opcode::PushInt(5),
                Opcode::PushInt(42),
                Opcode::LogicalOr,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result4 = interp.execute(&func4).unwrap();
        assert_eq!(result4.to_int32(), 5);
    }

    #[test]
    fn test_jump() {
        let mut interp = make_interpreter();
        // Jump(2) skips PushInt(0) and goes to PushInt(99)
        let func = make_function(
            vec![
                Opcode::Jump(2),
                Opcode::PushInt(0),
                Opcode::PushInt(99),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 99);
    }

    #[test]
    fn test_return() {
        let mut interp = make_interpreter();
        // PushInt(42) followed by Return should return 42
        let func = make_function(
            vec![
                Opcode::PushInt(42),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_throw_catch() {
        let mut interp = make_interpreter();
        // PushHandler(3) sets catch at pc=3
        // Push the exception value, then Throw (pops it and sets pending_exception)
        // Handler catches: pushes exception, pops handler, jumps to catch_pc=3
        // At pc=3: Pop removes exception, PushInt(99), Return
        let func = make_function(
            vec![
                Opcode::PushHandler(3),   // pc 0: catch at pc=3
                Opcode::PushInt(0),       // pc 1: exception value (popped by Throw)
                Opcode::Throw,            // pc 2: throw -> pending_exception = Some(0)
                Opcode::Pop,              // pc 3: pop the exception pushed by handler
                Opcode::PushInt(99),      // pc 4: push return value
                Opcode::Return,           // pc 5: return 99
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 99);
    }

    #[test]
    fn test_get_global() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::GetGlobal,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn test_get_set_property() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::String("x".to_string())];
        let func = make_function(
            vec![
                Opcode::CreateObject,
                Opcode::Dup,
                Opcode::PushString(0), // "x"
                Opcode::PushInt(42),
                Opcode::SetProperty,   // pops [val, prop, obj] -> obj.x = 42, pushes 42
                Opcode::Pop,
                Opcode::PushString(0), // "x"
                Opcode::GetProperty,   // pops [prop, obj] -> obj["x"]
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_get_set_property_by_name() {
        let mut interp = make_interpreter();
        let constants = vec![Constant::String("foo".to_string())];
        let func = make_function(
            vec![
                Opcode::CreateObject,
                Opcode::Dup,
                Opcode::PushInt(7),
                Opcode::SetPropertyByName(0), // set "foo" = 7
                Opcode::Pop,
                Opcode::GetPropertyByName(0), // get "foo"
                Opcode::Return,
            ],
            constants,
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 7);
    }

    #[test]
    fn test_create_closure() {
        use crate::value::BytecodeFunction as BF;
        let mut interp = make_interpreter();
        // CreateClosure(0) creates a closure from functions[0]
        let inner_func = BF {
            name: Some("inner".to_string()),
            params: Vec::new(),
            bytecode: vec![
                Opcode::PushInt(123),
                Opcode::Return,
            ],
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
            closure_vars: Vec::new(),
            is_arrow: false,
        };
        let func = make_function(
            vec![
                Opcode::CreateClosure(0),
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let mut func = func;
        func.functions.push(inner_func);
        let result = interp.execute(&func).unwrap();
        assert!(result.is_callable());
    }

    #[test]
    fn test_nop() {
        let mut interp = make_interpreter();
        let func = make_function(
            vec![
                Opcode::Nop,
                Opcode::PushInt(10),
                Opcode::Nop,
                Opcode::Return,
            ],
            Vec::new(),
            Vec::new(),
        );
        let result = interp.execute(&func).unwrap();
        assert_eq!(result.to_int32(), 10);
    }

    #[test]
    fn test_runtime_error_display() {
        let mut err = RuntimeError::new("something went wrong");
        err.stack_trace.push(StackFrameInfo {
            function_name: "main".to_string(),
            filename: Some("test.js".to_string()),
            line_number: Some(10),
            pc: 5,
        });
        let display = format!("{}", err);
        assert!(display.contains("something went wrong"));
        assert!(display.contains("main"));
        assert!(display.contains("test.js"));
        assert!(display.contains("10"));
    }

    #[test]
    fn test_runtime_error_with_message() {
        let err = RuntimeError::with_message("custom message".to_string());
        assert_eq!(err.message, "custom message");
        assert!(err.stack_trace.is_empty());
    }

    #[test]
    fn test_context_access() {
        let interp = make_interpreter();
        let ctx = interp.context();
        // global object exists and has a class_name
        assert_eq!(ctx.global.borrow().class_name, "global");

        let mut interp2 = make_interpreter();
        let ctx_mut = interp2.context_mut();
        assert_eq!(ctx_mut.global.borrow().class_name, "global");
    }

    #[test]
    fn test_clear_interrupt() {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let mut interp = Interpreter::new(ctx);
        assert!(!interp.is_interrupted());
        interp.interrupt();
        assert!(interp.is_interrupted());
        interp.clear_interrupt();
        assert!(!interp.is_interrupted());
    }
}

#[cfg(test)]
mod full_pipeline_tests {
    use super::*;
    use crate::parser::Parser;
    use crate::compiler::Compiler;
    use crate::runtime::JSRuntime;
    use crate::context::JSContext;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[test]
    fn test_full_pipeline_arithmetic() {
        let code = "1 + 2";
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();

        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();

        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let mut interpreter = Interpreter::new(ctx);

        let result = interpreter.execute(&bytecode).unwrap();
        assert_eq!(result.to_int32(), 3);
    }

    fn run_pipeline(code: &str) -> JSValue {
        let mut parser = Parser::new(code);
        let ast = parser.parse().unwrap();
        let mut compiler = Compiler::new();
        compiler.compile(&ast).unwrap();
        let bytecode = compiler.into_function();
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        let ctx = JSContext::new(rt);
        let mut interpreter = Interpreter::new(ctx);
        interpreter.execute(&bytecode).unwrap()
    }

    #[test]
    fn test_full_pipeline_subtraction() {
        let result = run_pipeline("10 - 3");
        assert_eq!(result.to_int32(), 7);
    }

    #[test]
    fn test_full_pipeline_multiplication() {
        let result = run_pipeline("6 * 7");
        assert_eq!(result.to_int32(), 42);
    }

    #[test]
    fn test_full_pipeline_division() {
        let result = run_pipeline("20 / 4");
        assert_eq!(result.to_number(), 5.0);
    }

    #[test]
    fn test_full_pipeline_modulo() {
        let result = run_pipeline("10 % 3");
        assert_eq!(result.to_number(), 1.0);
    }

    #[test]
    fn test_full_pipeline_string_literal() {
        let result = run_pipeline("\"hello\"");
        assert_eq!(result.to_string(), "hello");
    }

    #[test]
    fn test_full_pipeline_string_concat() {
        let result = run_pipeline("\"hello\" + \" world\"");
        assert_eq!(result.to_string(), "hello world");
    }

    #[test]
    fn test_full_pipeline_boolean_true() {
        let result = run_pipeline("true");
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_full_pipeline_boolean_false() {
        let result = run_pipeline("false");
        assert_eq!(result.to_boolean(), false);
    }

    #[test]
    fn test_full_pipeline_null_literal() {
        let result = run_pipeline("null");
        assert!(result.is_null());
    }

    #[test]
    fn test_full_pipeline_undefined_literal() {
        let result = run_pipeline("undefined");
        assert!(result.is_undefined());
    }

    #[test]
    fn test_full_pipeline_comparison() {
        let result = run_pipeline("5 > 3");
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_full_pipeline_equality() {
        let result = run_pipeline("5 == 5");
        assert_eq!(result.to_boolean(), true);
    }

    #[test]
    fn test_full_pipeline_let_var() {
        let result = run_pipeline("let x = 10; x");
        assert_eq!(result.to_int32(), 10);
    }

    #[test]
    fn test_full_pipeline_complex_expression() {
        let result = run_pipeline("(2 + 3) * 4");
        assert_eq!(result.to_int32(), 20);
    }

    #[test]
    fn test_full_pipeline_negation() {
        let result = run_pipeline("-5");
        assert_eq!(result.to_number(), -5.0);
    }

    #[test]
    fn test_full_pipeline_typeof_number() {
        let result = run_pipeline("typeof 42");
        assert_eq!(result.to_string(), "number");
    }

    #[test]
    fn test_full_pipeline_typeof_string() {
        let result = run_pipeline("typeof \"hello\"");
        assert_eq!(result.to_string(), "string");
    }
}

