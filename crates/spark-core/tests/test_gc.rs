//! Comprehensive tests for gc.rs - GarbageCollector

use spark_core::gc::GarbageCollector;
use spark_core::value::{JSObject, JSValue, JSFunction, FunctionBody};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn make_object(class_name: &str) -> Rc<RefCell<JSObject>> {
    Rc::new(RefCell::new(JSObject {
        properties: HashMap::new(),
        descriptors: HashMap::new(),
        prototype: None,
        internal_slots: HashMap::new(),
        class_name: class_name.to_string(),
    }))
}

fn make_function(name: &str) -> Rc<RefCell<JSFunction>> {
    Rc::new(RefCell::new(JSFunction {
        name: Some(name.to_string()),
        params: vec![],
        body: FunctionBody::Native(|_, _| JSValue::undefined()),
        closure: HashMap::new(),
        is_constructor: false,
        is_async: false,
        is_generator: false,
    }))
}

#[test]
fn test_gc_new_empty() {
    let gc = GarbageCollector::new();
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_default() {
    let gc = GarbageCollector::default();
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_track_object() {
    let mut gc = GarbageCollector::new();
    gc.track_object(make_object("Test"));
    assert_eq!(gc.count(), 1);
}

#[test]
fn test_gc_track_function() {
    let mut gc = GarbageCollector::new();
    gc.track_function(make_function("fn"));
    assert_eq!(gc.count(), 1);
}

#[test]
fn test_gc_track_multiple() {
    let mut gc = GarbageCollector::new();
    gc.track_object(make_object("A"));
    gc.track_object(make_object("B"));
    gc.track_function(make_function("fn"));
    assert_eq!(gc.count(), 3);
}

#[test]
fn test_gc_collect_removes_unreferenced() {
    let mut gc = GarbageCollector::new();
    gc.track_object(make_object("Test"));
    assert_eq!(gc.count(), 1);

    gc.collect();
    // Object has no external Rc refs, should be removed
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_collect_keeps_referenced() {
    let mut gc = GarbageCollector::new();
    let obj = make_object("Test");
    gc.track_object(obj.clone());
    assert_eq!(gc.count(), 1);

    gc.collect();
    // obj still holds a reference, should be kept
    assert_eq!(gc.count(), 1);
}

#[test]
fn test_gc_set_get_threshold() {
    let mut gc = GarbageCollector::new();
    let default_threshold = gc.get_threshold();
    assert!(default_threshold > 0);

    gc.set_threshold(100);
    assert_eq!(gc.get_threshold(), 100);
}

#[test]
fn test_gc_auto_collect_at_threshold() {
    let mut gc = GarbageCollector::new();
    gc.set_threshold(3);

    gc.track_object(make_object("A"));
    gc.track_object(make_object("B"));
    // At this point count=2, threshold=3, no auto-collect yet
    assert_eq!(gc.count(), 2);

    gc.track_object(make_object("C"));
    // count >= threshold, auto-collect should trigger
    // Unreferenced objects get collected
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_collect_mixed_referenced_unreferenced() {
    let mut gc = GarbageCollector::new();
    let kept = make_object("Kept");
    gc.track_object(kept.clone());
    gc.track_object(make_object("Dropped"));
    gc.track_function(make_function("dropped_fn"));
    assert_eq!(gc.count(), 3);

    gc.collect();
    // Only kept should survive
    assert_eq!(gc.count(), 1);
}

#[test]
fn test_gc_collect_on_empty() {
    let mut gc = GarbageCollector::new();
    gc.collect();
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_function_collect_removes_unreferenced() {
    let mut gc = GarbageCollector::new();
    gc.track_function(make_function("fn"));
    assert_eq!(gc.count(), 1);

    gc.collect();
    assert_eq!(gc.count(), 0);
}

#[test]
fn test_gc_function_collect_keeps_referenced() {
    let mut gc = GarbageCollector::new();
    let func = make_function("fn");
    gc.track_function(func.clone());
    assert_eq!(gc.count(), 1);

    gc.collect();
    assert_eq!(gc.count(), 1);
}
