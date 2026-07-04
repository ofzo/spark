#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! RegExp built-in.
//!
//! Implements the JavaScript RegExp constructor and its methods.
//! Stores pattern/flags in internal_slots. exec/test implement a basic
//! backtracking regex engine supporting character classes, quantifiers,
//! alternation, grouping, anchors, and common escape sequences.

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// RegExp internal representation (parsed pattern)
// ============================================================================

#[derive(Debug, Clone)]
enum RegexToken {
    Char(char),
    Dot,
    Digit,
    Word,
    Space,
    NotDigit,
    NotWord,
    NotSpace,
    Start,
    End,
    Group(Vec<RegexToken>),
    Alternation(Vec<Vec<RegexToken>>),
    Quantified {
        token: Box<RegexToken>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
    CharClass(Vec<CharRange>, bool), // (ranges, negated)
    BackRef(u32),
}

#[derive(Debug, Clone)]
struct CharRange {
    lo: char,
    hi: char,
}

// ============================================================================
// Parser
// ============================================================================

struct RegexParser {
    chars: Vec<char>,
    pos: usize,
    group_count: u32,
}

impl RegexParser {
    fn new(pattern: &str) -> Self {
        RegexParser {
            chars: pattern.chars().collect(),
            pos: 0,
            group_count: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn parse_alternation(&mut self) -> Vec<RegexToken> {
        let mut branches: Vec<Vec<RegexToken>> = Vec::new();
        loop {
            let branch = self.parse_branch_as_vec();
            branches.push(branch);
            if self.peek() == Some('|') {
                self.advance();
            } else {
                break;
            }
        }
        if branches.len() == 1 {
            branches.remove(0)
        } else {
            vec![RegexToken::Alternation(branches)]
        }
    }

    fn parse_branch_as_vec(&mut self) -> Vec<RegexToken> {
        let mut tokens = Vec::new();
        while self.peek().is_some() && self.peek() != Some('|') && self.peek() != Some(')') {
            if let Some(token) = self.parse_quantified() {
                tokens.push(token);
            }
        }
        tokens
    }

    fn parse_branch(&mut self) -> RegexToken {
        let mut tokens = Vec::new();
        while self.peek().is_some() && self.peek() != Some('|') && self.peek() != Some(')') {
            if let Some(token) = self.parse_quantified() {
                tokens.push(token);
            }
        }
        if tokens.len() == 1 {
            tokens.remove(0)
        } else {
            // Wrap multiple tokens in an implicit group
            RegexToken::Group(tokens)
        }
    }

    fn parse_quantified(&mut self) -> Option<RegexToken> {
        let token = self.parse_atom()?;
        let min;
        let max;
        let greedy = true;

        match self.peek() {
            Some('*') => {
                self.advance();
                min = 0;
                max = None;
            }
            Some('+') => {
                self.advance();
                min = 1;
                max = None;
            }
            Some('?') => {
                self.advance();
                min = 0;
                max = Some(1);
            }
            Some('{') => {
                let saved = self.pos;
                self.advance();
                if let Some(n) = self.parse_number() {
                    if self.peek() == Some('}') {
                        self.advance();
                        min = n;
                        max = Some(n);
                    } else if self.peek() == Some(',') {
                        self.advance();
                        let m = self.parse_number();
                        if self.peek() == Some('}') {
                            self.advance();
                            min = n;
                            max = m;
                        } else {
                            self.pos = saved;
                            return Some(token);
                        }
                    } else {
                        self.pos = saved;
                        return Some(token);
                    }
                } else {
                    self.pos = saved;
                    return Some(token);
                }
            }
            _ => return Some(token),
        }

        // Check for lazy quantifier
        if self.peek() == Some('?') {
            self.advance();
            return Some(RegexToken::Quantified {
                token: Box::new(token),
                min,
                max,
                greedy: false,
            });
        }

        Some(RegexToken::Quantified {
            token: Box::new(token),
            min,
            max,
            greedy,
        })
    }

    fn parse_number(&mut self) -> Option<u32> {
        let start = self.pos;
        let mut n: u32 = 0;
        let mut found = false;
        while let Some(ch) = self.peek() {
            if ch >= '0' && ch <= '9' {
                n = n.checked_mul(10)?.checked_add((ch as u32) - ('0' as u32))?;
                self.advance();
                found = true;
            } else {
                break;
            }
        }
        if found { Some(n) } else { None }
    }

    fn parse_atom(&mut self) -> Option<RegexToken> {
        let ch = self.advance()?;
        match ch {
            '(' => {
                self.group_count += 1;
                let inner = self.parse_alternation();
                self.expect(')');
                Some(RegexToken::Group(inner))
            }
            '[' => Some(self.parse_char_class()),
            '.' => Some(RegexToken::Dot),
            '^' => Some(RegexToken::Start),
            '$' => Some(RegexToken::End),
            '\\' => self.parse_escape(),
            '?' => Some(RegexToken::Quantified {
                token: Box::new(RegexToken::Char('?')),
                min: 0,
                max: Some(1),
                greedy: true,
            }),
            _ => Some(RegexToken::Char(ch)),
        }
    }

    fn parse_escape(&mut self) -> Option<RegexToken> {
        let ch = self.advance()?;
        match ch {
            'd' => Some(RegexToken::Digit),
            'D' => Some(RegexToken::NotDigit),
            'w' => Some(RegexToken::Word),
            'W' => Some(RegexToken::NotWord),
            's' => Some(RegexToken::Space),
            'S' => Some(RegexToken::NotSpace),
            'n' => Some(RegexToken::Char('\n')),
            'r' => Some(RegexToken::Char('\r')),
            't' => Some(RegexToken::Char('\t')),
            '0' => Some(RegexToken::Char('\0')),
            'b' => Some(RegexToken::Start), // simplified
            'B' => Some(RegexToken::End),   // simplified
            'f' => Some(RegexToken::Char('\x0C')),
            'v' => Some(RegexToken::Char('\x0B')),
            'x' => {
                // Hex escape \xNN
                let hi = self.advance()?;
                let lo = self.advance()?;
                let val = hex_val(hi)? * 16 + hex_val(lo)?;
                Some(RegexToken::Char(val as u8 as char))
            }
            'u' => {
                // Unicode escape \uNNNN
                let mut val: u32 = 0;
                for _ in 0..4 {
                    let ch = self.advance()?;
                    val = val * 16 + hex_val(ch)? as u32;
                }
                char::from_u32(val).map(RegexToken::Char)
            }
            '1'..='9' => {
                let num = (ch as u32) - ('0' as u32);
                // Check if this is a backreference
                if self.peek().map_or(false, |c| c >= '0' && c <= '9') {
                    let next = self.advance().unwrap();
                    let num2 = (next as u32) - ('0' as u32);
                    Some(RegexToken::BackRef(num * 10 + num2))
                } else {
                    Some(RegexToken::BackRef(num))
                }
            }
            _ => Some(RegexToken::Char(ch)),
        }
    }

    fn parse_char_class(&mut self) -> RegexToken {
        let mut ranges = Vec::new();
        let mut negated = false;

        if self.peek() == Some('^') {
            self.advance();
            negated = true;
        }

        let mut first = true;
        while let Some(ch) = self.peek() {
            if ch == ']' && !first {
                self.advance();
                break;
            }
            first = false;
            let c = self.advance().unwrap();
            if c == '\\' {
                if let Some(escaped) = self.advance() {
                    match escaped {
                        'd' => {
                            ranges.push(CharRange { lo: '0', hi: '9' });
                            continue;
                        }
                        'w' => {
                            ranges.push(CharRange { lo: 'a', hi: 'z' });
                            ranges.push(CharRange { lo: 'A', hi: 'Z' });
                            ranges.push(CharRange { lo: '0', hi: '9' });
                            ranges.push(CharRange { lo: '_', hi: '_' });
                            continue;
                        }
                        's' => {
                            ranges.push(CharRange { lo: ' ', hi: ' ' });
                            ranges.push(CharRange { lo: '\t', hi: '\r' });
                            continue;
                        }
                        'n' => {
                            ranges.push(CharRange { lo: '\n', hi: '\n' });
                            continue;
                        }
                        'r' => {
                            ranges.push(CharRange { lo: '\r', hi: '\r' });
                            continue;
                        }
                        't' => {
                            ranges.push(CharRange { lo: '\t', hi: '\t' });
                            continue;
                        }
                        'x' => {
                            let hi = self.advance().and_then(hex_val).unwrap_or(0);
                            let lo = self.advance().and_then(hex_val).unwrap_or(0);
                            let c = (hi * 16 + lo) as u8 as char;
                            ranges.push(CharRange { lo: c, hi: c });
                            continue;
                        }
                        'u' => {
                            let mut val: u32 = 0;
                            for _ in 0..4 {
                                if let Some(ch) = self.advance() {
                                    val = val * 16 + hex_val(ch).unwrap_or(0) as u32;
                                }
                            }
                            if let Some(c) = char::from_u32(val) {
                                ranges.push(CharRange { lo: c, hi: c });
                            }
                            continue;
                        }
                        _ => {
                            ranges.push(CharRange { lo: escaped, hi: escaped });
                            continue;
                        }
                    }
                }
            }
            // Check for range
            if self.peek() == Some('-') && self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] != ']' {
                self.advance(); // skip -
                if let Some(end_ch) = self.advance() {
                    let end = if end_ch == '\\' {
                        self.advance().unwrap_or(end_ch)
                    } else {
                        end_ch
                    };
                    ranges.push(CharRange { lo: c, hi: end });
                } else {
                    ranges.push(CharRange { lo: c, hi: c });
                }
            } else {
                ranges.push(CharRange { lo: c, hi: c });
            }
        }

        RegexToken::CharClass(ranges, negated)
    }

    fn expect(&mut self, expected: char) {
        if self.peek() == Some(expected) {
            self.advance();
        }
    }
}

fn hex_val(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some((ch as u8) - b'0'),
        'a'..='f' => Some((ch as u8) - b'a' + 10),
        'A'..='F' => Some((ch as u8) - b'A' + 10),
        _ => None,
    }
}

// ============================================================================
// Matcher
// ============================================================================

struct RegexMatcher<'a> {
    tokens: &'a [RegexToken],
}

impl<'a> RegexMatcher<'a> {
    fn new(tokens: &'a [RegexToken]) -> Self {
        RegexMatcher { tokens }
    }

    /// Try to match the pattern against `input` starting at `pos`.
    /// Returns the end position if matched, or None.
    fn match_tokens(&self, tokens: &[RegexToken], input: &[char], pos: usize) -> Option<usize> {
        if tokens.is_empty() {
            return Some(pos);
        }
        let (first, rest) = tokens.split_first()?;
        self.match_one(first, rest, input, pos)
    }

    fn match_one(
        &self,
        token: &RegexToken,
        rest: &[RegexToken],
        input: &[char],
        pos: usize,
    ) -> Option<usize> {
        match token {
            RegexToken::Char(c) => {
                if pos < input.len() && input[pos] == *c {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Dot => {
                if pos < input.len() && input[pos] != '\n' {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Digit => {
                if pos < input.len() && input[pos].is_ascii_digit() {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::NotDigit => {
                if pos < input.len() && !input[pos].is_ascii_digit() {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Word => {
                if pos < input.len() && (input[pos].is_alphanumeric() || input[pos] == '_') {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::NotWord => {
                if pos < input.len() && !(input[pos].is_alphanumeric() || input[pos] == '_') {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Space => {
                if pos < input.len() && input[pos].is_whitespace() {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::NotSpace => {
                if pos < input.len() && !input[pos].is_whitespace() {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Start => {
                if pos == 0 {
                    self.match_tokens(rest, input, pos)
                } else {
                    None
                }
            }
            RegexToken::End => {
                if pos >= input.len() {
                    self.match_tokens(rest, input, pos)
                } else {
                    None
                }
            }
            RegexToken::CharClass(ranges, negated) => {
                if pos >= input.len() {
                    return None;
                }
                let ch = input[pos];
                let in_class = ranges.iter().any(|r| ch >= r.lo && ch <= r.hi);
                if in_class != *negated {
                    self.match_tokens(rest, input, pos + 1)
                } else {
                    None
                }
            }
            RegexToken::Group(inner) => {
                let inner_tokens = inner.as_slice();
                self.match_group(inner_tokens, rest, input, pos)
            }
            RegexToken::Alternation(branches) => {
                for branch in branches {
                    let all_tokens: Vec<&RegexToken> =
                        branch.iter().chain(rest.iter()).collect();
                    let all_tokens_owned: Vec<RegexToken> = all_tokens.into_iter().cloned().collect();
                    if let Some(end) = self.match_tokens(&all_tokens_owned, input, pos) {
                        return Some(end);
                    }
                }
                None
            }
            RegexToken::Quantified { token, min, max, greedy } => {
                self.match_quantified(token, *min, *max, *greedy, rest, input, pos)
            }
            RegexToken::BackRef(_n) => {
                // Simplified: backreferences not fully implemented
                Some(pos)
            }
        }
    }

    fn match_group(
        &self,
        group_tokens: &[RegexToken],
        rest: &[RegexToken],
        input: &[char],
        pos: usize,
    ) -> Option<usize> {
        // Try matching group then rest
        if let Some(end) = self.match_tokens(group_tokens, input, pos) {
            if let Some(final_end) = self.match_tokens(rest, input, end) {
                return Some(final_end);
            }
        }
        None
    }

    fn match_quantified(
        &self,
        token: &RegexToken,
        min: u32,
        max: Option<u32>,
        greedy: bool,
        rest: &[RegexToken],
        input: &[char],
        pos: usize,
    ) -> Option<usize> {
        let max_count = max.unwrap_or(input.len() as u32 + 1);

        if greedy {
            // Greedy: try to match as many as possible, then backtrack
            let mut count = 0;
            let mut positions = vec![pos];

            // Match minimum
            for _ in 0..min {
                if let Some(end) = self.match_one(token, &[], input, *positions.last()?) {
                    positions.push(end);
                    count += 1;
                } else {
                    return None;
                }
            }

            // Match as many as possible
            loop {
                if count >= max_count {
                    break;
                }
                if let Some(end) = self.match_one(token, &[], input, *positions.last()?) {
                    positions.push(end);
                    count += 1;
                } else {
                    break;
                }
            }

            // Try rest from most greedy match back to least (backtracking)
            while let Some(&current_pos) = positions.last() {
                if let Some(end) = self.match_tokens(rest, input, current_pos) {
                    return Some(end);
                }
                positions.pop();
            }

            None
        } else {
            // Lazy: try to match as few as possible
            let mut count = 0;
            let mut current_pos = pos;

            // Match minimum
            for _ in 0..min {
                if let Some(end) = self.match_one(token, &[], input, current_pos) {
                    current_pos = end;
                    count += 1;
                } else {
                    return None;
                }
            }

            loop {
                // Try rest first
                if let Some(end) = self.match_tokens(rest, input, current_pos) {
                    return Some(end);
                }
                if count >= max_count {
                    break;
                }
                // Match one more
                if let Some(end) = self.match_one(token, &[], input, current_pos) {
                    current_pos = end;
                    count += 1;
                } else {
                    break;
                }
            }

            // Final attempt
            self.match_tokens(rest, input, current_pos)
        }
    }
}

// ============================================================================
// Regex engine API
// ============================================================================

fn compile_pattern(pattern: &str) -> Vec<RegexToken> {
    let mut parser = RegexParser::new(pattern);
    parser.parse_alternation()
}

/// Match the pattern against the input. Returns (matched, start, end).
fn regex_match(pattern: &str, input: &str, global: bool, start_pos: usize, ignore_case: bool) -> Option<(usize, usize)> {
    let tokens = compile_pattern(pattern);
    let chars: Vec<char> = if ignore_case {
        input.to_lowercase().chars().collect()
    } else {
        input.chars().collect()
    };
    let matcher = RegexMatcher::new(&tokens);

    if global {
        // For global regex, find the first match starting at start_pos
        for i in start_pos..=chars.len() {
            if let Some(end) = matcher.match_tokens(&tokens, &chars, i) {
                return Some((i, end));
            }
        }
        None
    } else {
        // For non-global, find the first match anywhere in the string
        for i in 0..=chars.len() {
            if let Some(end) = matcher.match_tokens(&tokens, &chars, i) {
                return Some((i, end));
            }
        }
        None
    }
}

// ============================================================================
// RegExp constructor
// ============================================================================

/// RegExp(pattern, flags) constructor.
pub fn regexp_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let (pattern, flags) = {
        let p = args.get(0).map(|v| v.to_string()).unwrap_or_default();
        let f = args.get(1).map(|v| v.to_string()).unwrap_or_default();
        (p, f)
    };

    let global = flags.contains('g');
    let ignore_case = flags.contains('i');
    let multiline = flags.contains('m');
    let dot_all = flags.contains('s');
    let unicode = flags.contains('u');
    let sticky = flags.contains('y');

    let regexp = JSValue::object("RegExp");
    regexp.set_property("source", JSValue::string(&pattern));
    regexp.set_property("flags", JSValue::string(&flags));
    regexp.set_property("global", JSValue::bool(global));
    regexp.set_property("ignoreCase", JSValue::bool(ignore_case));
    regexp.set_property("multiline", JSValue::bool(multiline));
    regexp.set_property("dotAll", JSValue::bool(dot_all));
    regexp.set_property("unicode", JSValue::bool(unicode));
    regexp.set_property("sticky", JSValue::bool(sticky));
    regexp.set_property("lastIndex", JSValue::Float(0.0));
    regexp
}

// ============================================================================
// RegExp.prototype methods
// ============================================================================

/// RegExp.prototype.exec(string)
pub fn regexp_exec(this: &JSValue, args: &[JSValue]) -> JSValue {
    let input = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let source = this.get_property("source")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let flags = this.get_property("flags")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let global = flags.contains('g');
    let ignore_case = flags.contains('i');

    let start_pos = if global {
        this.get_property("lastIndex")
            .map(|v| v.to_number() as usize)
            .unwrap_or(0)
    } else {
        0
    };

    match regex_match(&source, &input, global, start_pos, ignore_case) {
        Some((start, end)) => {
            let matched_text: String = input.chars().skip(start).take(end - start).collect();

            let result = JSValue::object("Object");
            result.set_property("index", JSValue::Float(start as f64));
            result.set_property("input", JSValue::string(&input));
            result.set_property("0", JSValue::string(&matched_text));

            if global {
                this.set_property("lastIndex", JSValue::Float(end as f64));
            }

            result
        }
        None => {
            if global {
                this.set_property("lastIndex", JSValue::Float(0.0));
            }
            JSValue::null()
        }
    }
}

/// RegExp.prototype.test(string)
pub fn regexp_test(this: &JSValue, args: &[JSValue]) -> JSValue {
    let input = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    let source = this.get_property("source")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let flags = this.get_property("flags")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let global = flags.contains('g');
    let ignore_case = flags.contains('i');

    let start_pos = if global {
        this.get_property("lastIndex")
            .map(|v| v.to_number() as usize)
            .unwrap_or(0)
    } else {
        0
    };

    let found = regex_match(&source, &input, global, start_pos, ignore_case).is_some();

    if global && !found {
        this.set_property("lastIndex", JSValue::Float(0.0));
    }

    JSValue::bool(found)
}

/// RegExp.prototype.toString()
pub fn regexp_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let source = this.get_property("source")
        .map(|v| v.to_string())
        .unwrap_or_default();
    let flags = this.get_property("flags")
        .map(|v| v.to_string())
        .unwrap_or_default();
    JSValue::string(&format!("/{}/{}", source, flags))
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the RegExp constructor and prototype.
pub fn init_regexp(ctx: &mut JSContext) {
    let regexp_ctor = JSValue::function(
        Some("RegExp"),
        vec!["pattern".to_string(), "flags".to_string()],
        FunctionBody::Native(regexp_constructor),
    );

    // Create RegExp.prototype
    let prototype = JSValue::object("RegExp");

    let methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("exec", regexp_exec),
        ("test", regexp_test),
        ("toString", regexp_to_string),
    ];

    for &(name, func) in methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // Add property accessors for source, flags, etc.
    prototype.set_property("source", JSValue::string("(?:)"));
    prototype.set_property("flags", JSValue::string(""));
    prototype.set_property("global", JSValue::bool(false));
    prototype.set_property("ignoreCase", JSValue::bool(false));
    prototype.set_property("multiline", JSValue::bool(false));
    prototype.set_property("dotAll", JSValue::bool(false));
    prototype.set_property("unicode", JSValue::bool(false));
    prototype.set_property("sticky", JSValue::bool(false));
    prototype.set_property("lastIndex", JSValue::Float(0.0));

    regexp_ctor.set_property("prototype", prototype);

    ctx.global
        .borrow_mut()
        .properties
        .insert("RegExp".to_string(), regexp_ctor);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    fn make_ctx() -> JSContext {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        JSContext::new(rt)
    }

    #[test]
    fn test_regexp_constructor() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("hello"), JSValue::string("gi")]);
        assert_eq!(re.get_property("source").unwrap().to_string(), "hello");
        assert_eq!(re.get_property("flags").unwrap().to_string(), "gi");
        assert_eq!(re.get_property("global").unwrap().to_boolean(), true);
        assert_eq!(re.get_property("ignoreCase").unwrap().to_boolean(), true);
    }

    #[test]
    fn test_regexp_exec_basic() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("world"), JSValue::string("")]);
        let result = regexp_exec(&re, &[JSValue::string("hello world!")]);
        match &result {
            JSValue::Object(obj) => {
                let borrow = obj.borrow();
                assert_eq!(borrow.properties.get("0").unwrap().to_string(), "world");
                assert_eq!(borrow.properties.get("index").unwrap().to_number(), 6.0);
            }
            _ => panic!("Expected match result"),
        }
    }

    #[test]
    fn test_regexp_exec_no_match() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("xyz"), JSValue::string("")]);
        let result = regexp_exec(&re, &[JSValue::string("hello world")]);
        assert!(result.is_null());
    }

    #[test]
    fn test_regexp_test() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("ell"), JSValue::string("")]);
        let result = regexp_test(&re, &[JSValue::string("hello")]);
        assert!(result.to_boolean());
    }

    #[test]
    fn test_regexp_to_string() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("abc"), JSValue::string("gi")]);
        let result = regexp_to_string(&re, &[]);
        assert_eq!(result.to_string(), "/abc/gi");
    }

    #[test]
    fn test_regexp_char_class() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("[a-z]+"), JSValue::string("")]);
        let result = regexp_exec(&re, &[JSValue::string("abc123def")]);
        match &result {
            JSValue::Object(obj) => {
                let borrow = obj.borrow();
                assert_eq!(borrow.properties.get("0").unwrap().to_string(), "abc");
            }
            _ => panic!("Expected match"),
        }
    }

    #[test]
    fn test_regexp_quantifiers() {
        let this = JSValue::undefined();
        let re = regexp_constructor(&this, &[JSValue::string("a+"), JSValue::string("")]);
        let result = regexp_exec(&re, &[JSValue::string("aaabbb")]);
        match &result {
            JSValue::Object(obj) => {
                let borrow = obj.borrow();
                assert_eq!(borrow.properties.get("0").unwrap().to_string(), "aaa");
            }
            _ => panic!("Expected match"),
        }
    }

    #[test]
    fn test_init_regexp() {
        let mut ctx = make_ctx();
        init_regexp(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("RegExp").is_some());
    }
}
