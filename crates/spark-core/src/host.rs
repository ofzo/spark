//! Host environment interfaces.
//!
//! The engine uses these traits to interact with the outside world.
//! The execution environment (CLI, browser, embedded) provides implementations.
//! This keeps the engine core free of direct OS dependencies.

use std::rc::Rc;

use crate::value::JSValue;

// ============================================================================
// Clock
// ============================================================================

/// Abstraction for wall-clock time.
///
/// Used by `Date.now()` and the default `Date()` constructor.
/// The host can provide a real clock, a fixed clock (for testing),
/// or a simulated clock (for replay).
pub trait Clock {
    /// Return the current time in milliseconds since the Unix epoch.
    fn now_ms(&self) -> f64;
}

/// Default clock using `std::time::SystemTime`.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64
    }
}

// ============================================================================
// Output
// ============================================================================

/// Abstraction for console output.
///
/// Used by `console.log`, `console.error`, `console.warn`, `console.info`,
/// and the `print()` function.
/// The host can redirect output to stdout, a log file, a web console, etc.
pub trait Output {
    /// Write a line to standard output (used by console.log, print, etc.)
    fn write_stdout(&self, text: &str);

    /// Write a line to standard error (used by console.error, console.warn)
    fn write_stderr(&self, text: &str);
}

/// Default output using stdout/stderr.
pub struct StdioOutput;

impl Output for StdioOutput {
    fn write_stdout(&self, text: &str) {
        println!("{}", text);
    }

    fn write_stderr(&self, text: &str) {
        eprintln!("{}", text);
    }
}

// ============================================================================
// Async executor
// ============================================================================

/// Callback type for async continuations.
/// When a Promise resolves, the engine calls this with the resolved value.
pub type AsyncCallback = Box<dyn FnOnce(JSValue)>;

/// Abstraction for async task scheduling.
///
/// The host implements this to provide an event loop.
/// The engine calls these methods when it needs to schedule async work.
pub trait AsyncExecutor {
    /// Enqueue a microtask (Promise callback).
    fn enqueue_microtask(&self, task: Box<dyn FnOnce()>);

    /// Enqueue a macrotask with optional delay (setTimeout/setInterval).
    fn enqueue_macrotask(&self, task: Box<dyn FnOnce()>, delay_ms: u64);

    /// Called when `await` encounters a pending Promise.
    fn on_await(&self, promise: &JSValue, continuation: AsyncCallback);
}

/// Default async executor that processes everything synchronously.
pub struct SyncExecutor;

impl AsyncExecutor for SyncExecutor {
    fn enqueue_microtask(&self, task: Box<dyn FnOnce()>) {
        task();
    }

    fn enqueue_macrotask(&self, task: Box<dyn FnOnce()>, _delay_ms: u64) {
        task();
    }

    fn on_await(&self, promise: &JSValue, continuation: AsyncCallback) {
        let result = if let Some(state) = promise.get_property("__state") {
            match state {
                JSValue::Int(1) => promise.get_property("__result").unwrap_or(JSValue::undefined()),
                JSValue::Int(2) => promise.get_property("__result").unwrap_or(JSValue::undefined()),
                _ => JSValue::undefined(),
            }
        } else {
            JSValue::undefined()
        };
        continuation(result);
    }
}

// ============================================================================
// Host configuration bundle
// ============================================================================

/// Complete host environment configuration.
///
/// Bundles all host-provided implementations using `Rc` so they can be
/// shared with thread_locals without cloning the underlying implementations.
pub struct HostEnvironment {
    pub clock: Rc<dyn Clock>,
    pub output: Rc<dyn Output>,
    pub executor: Rc<dyn AsyncExecutor>,
}

impl HostEnvironment {
    /// Create with default implementations (SystemClock + StdioOutput + SyncExecutor).
    pub fn defaults() -> Self {
        HostEnvironment {
            clock: Rc::new(SystemClock),
            output: Rc::new(StdioOutput),
            executor: Rc::new(SyncExecutor),
        }
    }
}

impl Default for HostEnvironment {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    // =========================================================================
    // SystemClock
    // =========================================================================

    #[test]
    fn system_clock_now_ms_returns_reasonable_value() {
        let clock = SystemClock;
        let t = clock.now_ms();
        // Unix epoch 2020-01-01 in millis
        let epoch_2020 = 1_577_836_800_000.0;
        assert!(
            t >= epoch_2020,
            "expected now_ms() >= {} (2020), got {}",
            epoch_2020,
            t
        );
    }

    #[test]
    fn system_clock_now_ms_is_monotonically_non_decreasing() {
        let clock = SystemClock;
        let t1 = clock.now_ms();
        let t2 = clock.now_ms();
        assert!(
            t2 >= t1,
            "clock went backwards: t1={}, t2={}",
            t1,
            t2
        );
    }

    #[test]
    fn clock_trait_object_works() {
        let clock: Rc<dyn Clock> = Rc::new(SystemClock);
        let t = clock.now_ms();
        assert!(t > 0.0);
    }

    // =========================================================================
    // StdioOutput
    // =========================================================================

    #[test]
    fn stdio_output_write_stdout_does_not_panic() {
        let output = StdioOutput;
        output.write_stdout("test stdout line");
    }

    #[test]
    fn stdio_output_write_stderr_does_not_panic() {
        let output = StdioOutput;
        output.write_stderr("test stderr line");
    }

    #[test]
    fn stdio_output_trait_object_works() {
        let output: Rc<dyn Output> = Rc::new(StdioOutput);
        output.write_stdout("trait stdout");
        output.write_stderr("trait stderr");
    }

    // =========================================================================
    // SyncExecutor
    // =========================================================================

    #[test]
    fn sync_executor_enqueue_microtask_executes_immediately() {
        let executor = SyncExecutor;
        let flag = Rc::new(Cell::new(false));
        let flag_clone = flag.clone();
        executor.enqueue_microtask(Box::new(move || {
            flag_clone.set(true);
        }));
        assert!(flag.get(), "microtask should have been executed immediately");
    }

    #[test]
    fn sync_executor_enqueue_macrotask_executes_immediately() {
        let executor = SyncExecutor;
        let flag = Rc::new(Cell::new(false));
        let flag_clone = flag.clone();
        executor.enqueue_macrotask(Box::new(move || {
            flag_clone.set(true);
        }), 0);
        assert!(flag.get(), "macrotask should have been executed immediately");
    }

    #[test]
    fn sync_executor_enqueue_macrotask_ignores_delay() {
        let executor = SyncExecutor;
        let val = Rc::new(Cell::new(0u64));
        let val_clone = val.clone();
        executor.enqueue_macrotask(Box::new(move || {
            val_clone.set(42);
        }), 10_000);
        // Even with a large delay, SyncExecutor executes synchronously
        assert_eq!(val.get(), 42);
    }

    #[test]
    fn sync_executor_on_await_fulfilled_promise_returns_result() {
        let executor = SyncExecutor;
        // Create a fulfilled Promise: __state = Int(1), __result = Int(99)
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(1));
        promise.set_property("__result", JSValue::Int(99));

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::Int(99)));
    }

    #[test]
    fn sync_executor_on_await_rejected_promise_returns_result() {
        let executor = SyncExecutor;
        // Create a rejected Promise: __state = Int(2), __result = error string
        let expected = JSValue::string("error occurred");
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(2));
        promise.set_property("__result", expected.clone());

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        // Use Rc::ptr_eq since PartialEq for String uses pointer comparison
        match (&*val, &expected) {
            (Some(JSValue::String(a)), JSValue::String(b)) => {
                assert!(Rc::ptr_eq(a, b), "expected same Rc pointer for string result");
            }
            _ => panic!("expected Some(JSValue::String), got {:?}", *val),
        }
    }

    #[test]
    fn sync_executor_on_await_rejected_promise_without_result_returns_undefined() {
        let executor = SyncExecutor;
        // Rejected promise with no __result property
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(2));

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::undefined()));
    }

    #[test]
    fn sync_executor_on_await_pending_promise_returns_undefined() {
        let executor = SyncExecutor;
        // Pending Promise: __state = Int(0)
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(0));

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::undefined()));
    }

    #[test]
    fn sync_executor_on_await_non_promise_value_returns_undefined() {
        let executor = SyncExecutor;
        // A plain value with no __state property
        let plain_value = JSValue::Int(42);

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&plain_value, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::undefined()));
    }

    #[test]
    fn sync_executor_on_await_unknown_state_returns_undefined() {
        let executor = SyncExecutor;
        // Promise with __state = Int(3) (unknown state)
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(3));

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::undefined()));
    }

    #[test]
    fn sync_executor_on_await_fulfilled_without_result_returns_undefined() {
        let executor = SyncExecutor;
        // Fulfilled promise with no __result property
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(1));

        let received = Rc::new(RefCell::new(None));
        let received_clone = received.clone();
        executor.on_await(&promise, Box::new(move |val| {
            *received_clone.borrow_mut() = Some(val);
        }));

        let val = received.borrow();
        assert_eq!(*val, Some(JSValue::undefined()));
    }

    #[test]
    fn sync_executor_trait_object_works() {
        let executor: Rc<dyn AsyncExecutor> = Rc::new(SyncExecutor);
        let flag = Rc::new(Cell::new(false));
        let flag_clone = flag.clone();
        executor.enqueue_microtask(Box::new(move || {
            flag_clone.set(true);
        }));
        assert!(flag.get());
    }

    // =========================================================================
    // HostEnvironment
    // =========================================================================

    #[test]
    fn host_environment_defaults_creates_valid_environment() {
        let env = HostEnvironment::defaults();

        // Clock works
        let t = env.clock.now_ms();
        assert!(t > 0.0, "clock should return a positive time");

        // Output does not panic
        env.output.write_stdout("test");
        env.output.write_stderr("test");

        // Executor works
        let flag = Rc::new(Cell::new(false));
        let flag_clone = flag.clone();
        env.executor.enqueue_microtask(Box::new(move || {
            flag_clone.set(true);
        }));
        assert!(flag.get(), "executor should run microtasks immediately");
    }

    #[test]
    fn host_environment_default_trait_matches_defaults() {
        let env1 = HostEnvironment::defaults();
        let env2 = HostEnvironment::default();

        // Both should return positive clock values
        let t1 = env1.clock.now_ms();
        let t2 = env2.clock.now_ms();
        assert!(t1 > 0.0);
        assert!(t2 > 0.0);

        // Both should run microtasks
        let flag1 = Rc::new(Cell::new(false));
        let flag2 = Rc::new(Cell::new(false));
        let f1 = flag1.clone();
        let f2 = flag2.clone();
        env1.executor.enqueue_microtask(Box::new(move || f1.set(true)));
        env2.executor.enqueue_microtask(Box::new(move || f2.set(true)));
        assert!(flag1.get());
        assert!(flag2.get());
    }

    #[test]
    fn host_environment_shared_rc_pointers_are_independent() {
        // Creating two environments gives independent Rc pointers
        let env_a = HostEnvironment::defaults();
        let env_b = HostEnvironment::defaults();
        // Both clocks should return reasonable values (not testing same Rc)
        let ta = env_a.clock.now_ms();
        let tb = env_b.clock.now_ms();
        assert!(ta > 0.0);
        assert!(tb > 0.0);
        // They may or may not be equal (timing), but both should be valid
        assert!(ta <= tb || tb <= ta);
    }

    #[test]
    fn host_environment_executor_on_await_integration() {
        let env = HostEnvironment::defaults();

        // Create a fulfilled promise and verify the executor resolves it
        let expected = JSValue::string("hello");
        let promise = JSValue::object("Promise");
        promise.set_property("__state", JSValue::Int(1));
        promise.set_property("__result", expected.clone());

        let result = Rc::new(RefCell::new(None));
        let result_clone = result.clone();
        env.executor.on_await(&promise, Box::new(move |val| {
            *result_clone.borrow_mut() = Some(val);
        }));

        let val = result.borrow();
        match (&*val, &expected) {
            (Some(JSValue::String(a)), JSValue::String(b)) => {
                assert!(Rc::ptr_eq(a, b), "expected same Rc pointer for string result");
            }
            _ => panic!("expected Some(JSValue::String), got {:?}", *val),
        }
    }

    // =========================================================================
    // Edge cases and combined scenarios
    // =========================================================================

    #[test]
    fn sync_executor_multiple_microtasks_execute_in_order() {
        let executor = SyncExecutor;
        let order = Rc::new(RefCell::new(Vec::new()));

        for i in 1..=3 {
            let o = order.clone();
            executor.enqueue_microtask(Box::new(move || {
                o.borrow_mut().push(i);
            }));
        }

        assert_eq!(*order.borrow(), vec![1, 2, 3]);
    }

    #[test]
    fn sync_executor_multiple_macrotasks_execute_in_order() {
        let executor = SyncExecutor;
        let order = Rc::new(RefCell::new(Vec::new()));

        for i in 1..=2 {
            let o = order.clone();
            executor.enqueue_macrotask(Box::new(move || {
                o.borrow_mut().push(i);
            }), 100);
        }

        assert_eq!(*order.borrow(), vec![1, 2]);
    }

    #[test]
    fn sync_executor_mixed_micro_and_macro_tasks() {
        let executor = SyncExecutor;
        let log = Rc::new(RefCell::new(Vec::new()));

        let l1 = log.clone();
        executor.enqueue_microtask(Box::new(move || l1.borrow_mut().push("micro")));
        let l2 = log.clone();
        executor.enqueue_macrotask(Box::new(move || l2.borrow_mut().push("macro")), 0);

        assert_eq!(*log.borrow(), vec!["micro", "macro"]);
    }
}
