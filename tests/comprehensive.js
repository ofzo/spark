// Comprehensive test suite for QuickJS Rust runtime
print("=== QuickJS Rust Runtime Test Suite ===");
print("");

// === Variables and Types ===
print("typeof 42:", typeof 42 === "number" ? "PASS" : "FAIL");
print("typeof hello:", typeof "hello" === "string" ? "PASS" : "FAIL");
print("typeof true:", typeof true === "boolean" ? "PASS" : "FAIL");
print("typeof null:", typeof null === "object" ? "PASS" : "FAIL");
print("typeof undefined:", typeof undefined === "undefined" ? "PASS" : "FAIL");
print("typeof object:", typeof {} === "object" ? "PASS" : "FAIL");
print("typeof array:", typeof [] === "object" ? "PASS" : "FAIL");
print("typeof function:", typeof function(){} === "function" ? "PASS" : "FAIL");

// === Arithmetic ===
print("addition:", 2 + 3 === 5 ? "PASS" : "FAIL");
print("subtraction:", 5 - 3 === 2 ? "PASS" : "FAIL");
print("multiplication:", 4 * 3 === 12 ? "PASS" : "FAIL");
print("division:", 10 / 2 === 5 ? "PASS" : "FAIL");
print("modulo:", 10 % 3 === 1 ? "PASS" : "FAIL");
print("precedence:", 2 + 3 * 4 === 14 ? "PASS" : "FAIL");

// === Comparison ===
print("greater than:", 5 > 3 ? "PASS" : "FAIL");
print("less than:", 3 < 5 ? "PASS" : "FAIL");
print("strict equal:", 5 === 5 ? "PASS" : "FAIL");
print("strict not equal:", 5 !== 3 ? "PASS" : "FAIL");

// === Functions ===
function add(a, b) { return a + b; }
print("function declaration:", add(3, 4) === 7 ? "PASS" : "FAIL");

var mul = function(a, b) { return a * b; };
print("function expression:", mul(3, 4) === 12 ? "PASS" : "FAIL");

var sub = (a, b) => a - b;
print("arrow function:", sub(7, 3) === 4 ? "PASS" : "FAIL");

// === Arrays ===
var arr = [1, 2, 3, 4, 5];
print("array length:", arr.length === 5 ? "PASS" : "FAIL");
print("array index:", arr[0] === 1 ? "PASS" : "FAIL");

var mapped = arr.map((x) => x * 2);
print("array map:", mapped[0] === 2 ? "PASS" : "FAIL");

var filtered = arr.filter((x) => x > 3);
print("array filter:", filtered.length === 2 ? "PASS" : "FAIL");

var reduced = arr.reduce((a, b) => a + b, 0);
print("array reduce:", reduced === 15 ? "PASS" : "FAIL");

var found = arr.find((x) => x === 3);
print("array find:", found === 3 ? "PASS" : "FAIL");

var idx = arr.findIndex((x) => x === 3);
print("array findIndex:", idx === 2 ? "PASS" : "FAIL");

print("array some:", arr.some((x) => x > 4) ? "PASS" : "FAIL");
print("array every:", arr.every((x) => x > 0) ? "PASS" : "FAIL");

var chained = arr.filter((x) => x % 2 !== 0).map((x) => x * 10);
print("chained array:", chained.length === 3 ? "PASS" : "FAIL");

// === Objects ===
var obj = {a: 1, b: 2};
obj.c = 3;
print("object property:", obj.a === 1 ? "PASS" : "FAIL");
print("object set:", obj.c === 3 ? "PASS" : "FAIL");
print("Object.keys:", Object.keys(obj).length === 3 ? "PASS" : "FAIL");

// === Strings ===
var str = "Hello, World!";
print("toUpperCase:", str.toUpperCase() === "HELLO, WORLD!" ? "PASS" : "FAIL");
print("toLowerCase:", str.toLowerCase() === "hello, world!" ? "PASS" : "FAIL");
print("indexOf:", str.indexOf("World") === 7 ? "PASS" : "FAIL");
print("slice:", str.slice(7) === "World!" ? "PASS" : "FAIL");
print("replace:", str.replace("World", "Rust") === "Hello, Rust!" ? "PASS" : "FAIL");

// === Math ===
print("Math.max:", Math.max(5, 3, 8) === 8 ? "PASS" : "FAIL");
print("Math.min:", Math.min(5, 3, 8) === 3 ? "PASS" : "FAIL");
print("Math.floor:", Math.floor(3.7) === 3 ? "PASS" : "FAIL");
print("Math.ceil:", Math.ceil(3.2) === 4 ? "PASS" : "FAIL");
print("Math.abs:", Math.abs(-42) === 42 ? "PASS" : "FAIL");

// === JSON ===
var json = JSON.stringify({x: 1, y: [2, 3]});
print("JSON.stringify:", typeof json === "string" ? "PASS" : "FAIL");
var parsed = JSON.parse(json);
print("JSON.parse:", parsed.x === 1 ? "PASS" : "FAIL");

// === Map ===
var m = new Map();
m.set("key", "value");
m.set("num", 42);
print("Map get:", m.get("key") === "value" ? "PASS" : "FAIL");
print("Map has:", m.has("key") ? "PASS" : "FAIL");
print("Map size:", m.size === 2 ? "PASS" : "FAIL");

// === Set ===
var s = new Set();
s.add(1);
s.add(2);
s.add(1);
print("Set size:", s.size === 2 ? "PASS" : "FAIL");
print("Set has:", s.has(1) ? "PASS" : "FAIL");

// === RegExp ===
var r = /hello/;
print("RegExp test:", r.test("hello world") ? "PASS" : "FAIL");
print("RegExp no match:", !r.test("goodbye") ? "PASS" : "FAIL");
print("RegExp source:", r.source === "hello" ? "PASS" : "FAIL");

// === Date ===
print("Date.now:", typeof Date.now() === "number" ? "PASS" : "FAIL");

// === this binding ===
var obj2 = {value: 42, getValue: function() { return this.value; }};
print("this binding:", obj2.getValue() === 42 ? "PASS" : "FAIL");

// === Promise (synchronous) ===
var p1 = Promise.resolve(42);
print("Promise resolve:", p1.__state === 1 ? "PASS" : "FAIL");
print("Promise value:", p1.__result === 42 ? "PASS" : "FAIL");

var p2 = Promise.reject("err");
print("Promise reject:", p2.__state === 2 ? "PASS" : "FAIL");

// === Closures ===
function makeCounter() {
    var count = 0;
    return function() { count = count + 1; return count; };
}
var counter = makeCounter();
print("closure 1:", counter() === 1 ? "PASS" : "FAIL");
print("closure 2:", counter() === 2 ? "PASS" : "FAIL");
print("closure 3:", counter() === 3 ? "PASS" : "FAIL");

// === Logical operators ===
print("1 && 2:", (1 && 2) === 2 ? "PASS" : "FAIL");
print("0 && 2:", (0 && 2) === 0 ? "PASS" : "FAIL");
print("1 || 2:", (1 || 2) === 1 ? "PASS" : "FAIL");
print("0 || 2:", (0 || 2) === 2 ? "PASS" : "FAIL");

// Logical in function args
function check(a, b) { return a === "hello" && b === 2; }
print("logical in args:", check("hello", 1 && 2) ? "PASS" : "FAIL");

// === Increment/Decrement ===
var inc = 0;
inc++;
print("i++:", inc === 1 ? "PASS" : "FAIL");
++inc;
print("++i:", inc === 2 ? "PASS" : "FAIL");
inc--;
print("i--:", inc === 1 ? "PASS" : "FAIL");

// === For loop ===
var sum = 0;
for (var i = 1; i <= 5; i++) {
    sum = sum + i;
}
print("for loop sum:", sum === 15 ? "PASS" : "FAIL");

// === Async/Await ===
async function asyncAdd(a, b) {
    return a + b;
}
var p = asyncAdd(3, 4);
print("async result:", p.__result === 7 ? "PASS" : "FAIL");

// === Generators ===
function* gen() {
    yield 1;
    yield 2;
    yield 3;
}
var g = gen();
var r1 = g.next();
print("gen next:", r1.value === 1 && r1.done === false ? "PASS" : "FAIL");
var r2 = g.next();
print("gen next2:", r2.value === 2 ? "PASS" : "FAIL");
var r3 = g.next();
print("gen next3:", r3.value === 3 ? "PASS" : "FAIL");
var r4 = g.next();
print("gen done:", r4.done === true ? "PASS" : "FAIL");

// === Modules ===
import { greeting, add } from "/tmp/test_mod.js";
print("module greeting:", greeting === "Hello from module!" ? "PASS" : "FAIL");
print("module add:", add(3, 4) === 7 ? "PASS" : "FAIL");

print("");
print("=== All tests completed ===");
