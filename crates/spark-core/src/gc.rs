#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! Garbage collector.
//!
//! Implements a simple mark-and-sweep garbage collector using Rust's
//! reference counting (Rc) as the foundation.

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, JSFunction};

/// Garbage collector state.
pub struct GarbageCollector {
    /// Objects tracked by the GC
    tracked: Vec<GcObject>,
    /// Threshold for triggering GC
    threshold: usize,
    /// Current allocation count
    allocation_count: usize,
}

/// A tracked GC object.
enum GcObject {
    Object(Rc<RefCell<JSObject>>),
    Function(Rc<RefCell<JSFunction>>),
}

impl GarbageCollector {
    /// Create a new garbage collector.
    pub fn new() -> Self {
        GarbageCollector {
            tracked: Vec::new(),
            threshold: 1000,
            allocation_count: 0,
        }
    }

    /// Track an object for garbage collection.
    pub fn track_object(&mut self, obj: Rc<RefCell<JSObject>>) {
        self.tracked.push(GcObject::Object(obj));
        self.allocation_count += 1;
        if self.allocation_count >= self.threshold {
            self.collect();
        }
    }

    /// Track a function for garbage collection.
    pub fn track_function(&mut self, func: Rc<RefCell<JSFunction>>) {
        self.tracked.push(GcObject::Function(func));
        self.allocation_count += 1;
        if self.allocation_count >= self.threshold {
            self.collect();
        }
    }

    /// Run garbage collection.
    pub fn collect(&mut self) {
        // Remove objects that have no more references
        self.tracked.retain(|obj| match obj {
            GcObject::Object(o) => Rc::strong_count(o) > 1,
            GcObject::Function(f) => Rc::strong_count(f) > 1,
        });
        self.allocation_count = 0;
    }

    /// Get the number of tracked objects.
    pub fn count(&self) -> usize {
        self.tracked.len()
    }

    /// Set the GC threshold.
    pub fn set_threshold(&mut self, threshold: usize) {
        self.threshold = threshold;
    }

    /// Get the GC threshold.
    pub fn get_threshold(&self) -> usize {
        self.threshold
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::JSObject;
    use std::collections::HashMap;

    #[test]
    fn test_gc_creation() {
        let gc = GarbageCollector::new();
        assert_eq!(gc.count(), 0);
    }

    #[test]
    fn test_gc_tracking() {
        let mut gc = GarbageCollector::new();
        let obj = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));

        gc.track_object(obj.clone());
        assert_eq!(gc.count(), 1);
    }

    #[test]
    fn test_gc_collection() {
        let mut gc = GarbageCollector::new();
        gc.set_threshold(2);

        let obj1 = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        gc.track_object(obj1);

        let obj2 = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        gc.track_object(obj2);

        // GC should have run
        assert_eq!(gc.count(), 0);
    }

    #[test]
    fn test_gc_default() {
        let gc = GarbageCollector::default();
        assert_eq!(gc.count(), 0);
    }

    #[test]
    fn test_gc_track_function() {
        use crate::value::FunctionBody;
        let mut gc = GarbageCollector::new();
        let func = Rc::new(RefCell::new(JSFunction {
            name: Some("test_fn".to_string()),
            params: vec![],
            body: FunctionBody::Native(|_this, _args| JSValue::undefined()),
            closure: HashMap::new(),
            is_constructor: false,
            is_async: false,
            is_generator: false,
        }));
        gc.track_function(func);
        assert_eq!(gc.count(), 1);
    }

    #[test]
    fn test_gc_set_get_threshold() {
        let mut gc = GarbageCollector::new();
        assert_eq!(gc.get_threshold(), 1000);
        gc.set_threshold(500);
        assert_eq!(gc.get_threshold(), 500);
    }

    #[test]
    fn test_gc_collection_keeps_referenced() {
        let mut gc = GarbageCollector::new();
        let obj = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        let _keep_ref = obj.clone();
        gc.track_object(obj);
        gc.collect();
        // Object still has an external reference (_keep_ref), so it is kept
        assert_eq!(gc.count(), 1);
    }

    #[test]
    fn test_gc_collection_removes_unreferenced() {
        let mut gc = GarbageCollector::new();
        let obj = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        gc.track_object(obj);
        // No external references remain (the Rc was passed by value and not cloned)
        gc.collect();
        assert_eq!(gc.count(), 0);
    }

    #[test]
    fn test_gc_auto_collect() {
        use crate::value::FunctionBody;
        let mut gc = GarbageCollector::new();
        gc.set_threshold(1);

        // Track first object — triggers collect (count 1 >= threshold 1)
        let obj1 = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        gc.track_object(obj1);
        // After auto-collect, allocation_count resets; first object had no external
        // references so it was removed.
        assert_eq!(gc.count(), 0);

        // Track second object with an external reference — auto-collect fires
        let obj2 = Rc::new(RefCell::new(JSObject {
            properties: HashMap::new(),
            descriptors: HashMap::new(),
            prototype: None,
            internal_slots: HashMap::new(),
            class_name: "Object".to_string(),
        }));
        let _keep = obj2.clone();
        gc.track_object(obj2);
        // obj2 has an external reference, so it survives auto-collect
        assert_eq!(gc.count(), 1);
    }
}
