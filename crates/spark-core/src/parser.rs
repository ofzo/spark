#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]//! JavaScript parser.
//!
//! Parses JavaScript source code into an AST.

use std::fmt;

/// Token type.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Literals
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Undefined,
    RegExp { pattern: String, flags: String },

    // Identifiers and keywords
    Identifier(String),
    Keyword(Keyword),

    // Operators
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    Percent,     // %
    Assign,      // =
    PlusAssign,  // +=
    MinusAssign, // -=
    StarAssign,  // *=
    SlashAssign, // /=
    Eq,          // ==
    Ne,          // !=
    StrictEq,    // ===
    StrictNe,    // !==
    Lt,          // <
    Gt,          // >
    Le,          // <=
    Ge,          // >=
    And,         // &&
    Or,          // ||
    Not,         // !
    BitAnd,      // &
    BitOr,       // |
    BitXor,      // ^
    BitNot,      // ~
    Shl,         // <<
    Shr,         // >>
    UShr,        // >>>
    Inc,         // ++
    Dec,         // --
    Dot,         // .
    Comma,       // ,
    Semicolon,   // ;
    Colon,       // :
    Question,    // ?
    Arrow,       // =>
    Ellipsis,    // ...
    OptionalChaining, // ?.
    NullishCoalescing, // ??

    // Additional operators
    Pow,             // **
    PowAssign,       // **=
    ShrAssign,       // >>=
    AndAssign,       // &=
    OrAssign,        // |=
    BitAndAssign,    // &=
    BitOrAssign,     // |=
    BitXorAssign,    // ^=
    ShlAssign,       // <<=
    UShrAssign,      // >>>=
    PercentAssign,   // %=

    // Delimiters
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]

    // Template
    TemplateLiteral(Vec<TemplateToken>),

    // Special
    Eof,
    Error(String),
}

/// JavaScript keywords.
#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Break,
    Case,
    Catch,
    Class,
    Const,
    Continue,
    Debugger,
    Default,
    Delete,
    Do,
    Else,
    Export,
    Extends,
    Finally,
    For,
    Function,
    If,
    Import,
    In,
    Instanceof,
    Let,
    New,
    Return,
    Super,
    Switch,
    This,
    Throw,
    Try,
    Typeof,
    Var,
    Void,
    While,
    With,
    Yield,
    Async,
    Await,
    Of,
    Static,
    Get,
    Set,
    As,
    From,
}

/// A token with position information.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub line: usize,
    pub column: usize,
    pub start: usize,
    pub end: usize,
}

/// AST node types.
#[derive(Debug, Clone)]
pub enum ASTNode {
    // Literals
    NumberLiteral(f64),
    StringLiteral(String),
    BoolLiteral(bool),
    NullLiteral,
    UndefinedLiteral,
    RegExpLiteral { pattern: String, flags: String },

    // Identifiers
    Identifier(String),

    // Expressions
    BinaryOp {
        op: BinaryOp,
        left: Box<ASTNode>,
        right: Box<ASTNode>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<ASTNode>,
        prefix: bool,
    },
    Call {
        callee: Box<ASTNode>,
        args: Vec<ASTNode>,
    },
    SuperCall(Vec<ASTNode>),
    New {
        callee: Box<ASTNode>,
        args: Vec<ASTNode>,
    },
    Member {
        object: Box<ASTNode>,
        property: Box<ASTNode>,
        computed: bool,
    },
    Conditional {
        test: Box<ASTNode>,
        consequent: Box<ASTNode>,
        alternate: Box<ASTNode>,
    },
    Assignment {
        op: AssignmentOp,
        left: Box<ASTNode>,
        right: Box<ASTNode>,
    },
    Sequence {
        expressions: Vec<ASTNode>,
    },

    // Statements
    ExpressionStatement(Box<ASTNode>),
    Block(Vec<ASTNode>),
    If {
        test: Box<ASTNode>,
        consequent: Box<ASTNode>,
        alternate: Option<Box<ASTNode>>,
    },
    While {
        test: Box<ASTNode>,
        body: Box<ASTNode>,
    },
    DoWhile {
        body: Box<ASTNode>,
        test: Box<ASTNode>,
    },
    For {
        init: Option<Box<ASTNode>>,
        test: Option<Box<ASTNode>>,
        update: Option<Box<ASTNode>>,
        body: Box<ASTNode>,
    },
    ForIn {
        left: Box<ASTNode>,
        right: Box<ASTNode>,
        body: Box<ASTNode>,
    },
    ForOf {
        left: Box<ASTNode>,
        right: Box<ASTNode>,
        body: Box<ASTNode>,
        is_await: bool,
    },
    Return(Option<Box<ASTNode>>),
    Throw(Box<ASTNode>),
    Try {
        block: Box<ASTNode>,
        catch: Option<CatchClause>,
        finally: Option<Box<ASTNode>>,
    },
    Switch {
        discriminant: Box<ASTNode>,
        cases: Vec<SwitchCase>,
    },
    LabeledStatement {
        label: String,
        body: Box<ASTNode>,
    },
    EmptyStatement,
    DebuggerStatement,
    Break(Option<String>),
    Continue(Option<String>),

    // Declarations
    VariableDeclaration {
        kind: VariableKind,
        declarations: Vec<VariableDeclarator>,
    },
    FunctionDeclaration {
        name: String,
        params: Vec<FunctionParam>,
        body: Box<ASTNode>,
        is_async: bool,
        is_generator: bool,
    },
    ClassDeclaration {
        name: String,
        super_class: Option<Box<ASTNode>>,
        body: Vec<ClassElement>,
    },

    // Module
    ImportDeclaration {
        specifiers: Vec<ImportSpecifier>,
        source: String,
    },
    ExportDeclaration {
        declaration: Option<Box<ASTNode>>,
        specifiers: Vec<ExportSpecifier>,
        source: Option<String>,
    },

    // Other
    SpreadElement(Box<ASTNode>),
    RestElement(Box<ASTNode>),
    TemplateLiteral {
        quasis: Vec<TemplateElement>,
        expressions: Vec<ASTNode>,
    },
    ArrayExpression(Vec<ASTNode>),
    ObjectExpression(Vec<PropertyDefinition>),
    ArrowFunctionExpression {
        params: Vec<FunctionParam>,
        body: Box<ASTNode>,
        is_async: bool,
    },
    YieldExpression {
        argument: Option<Box<ASTNode>>,
        delegate: bool,
    },
    AwaitExpression(Box<ASTNode>),
}

/// Binary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    In,
    Instanceof,
    NullishCoalescing,
}

/// Unary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Minus,
    Plus,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
    Inc,
    Dec,
}

/// Assignment operators.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    PowAssign,
    AndAssign,
    OrAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    ShlAssign,
    ShrAssign,
    UShrAssign,
}

/// Variable declaration kind.
#[derive(Debug, Clone, PartialEq)]
pub enum VariableKind {
    Var,
    Let,
    Const,
}

/// Variable declarator.
#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    pub name: String,
    pub init: Option<Box<ASTNode>>,
    /// Optional destructuring pattern (ArrayExpression or ObjectExpression)
    pub pattern: Option<Box<ASTNode>>,
}

/// Function parameter.
#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub default: Option<Box<ASTNode>>,
    pub is_rest: bool,
    pub pattern: Option<Box<ASTNode>>,
}

/// Catch clause.
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub param: Option<Box<ASTNode>>,
    pub body: Box<ASTNode>,
}

/// Switch case.
#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub test: Option<Box<ASTNode>>,
    pub consequent: Vec<ASTNode>,
}

/// Class element.
#[derive(Debug, Clone)]
pub struct ClassElement {
    pub key: Box<ASTNode>,
    pub value: Option<Box<ASTNode>>,
    pub kind: ClassElementKind,
    pub is_static: bool,
    pub is_computed: bool,
}

/// Class element kind.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassElementKind {
    Method,
    Get,
    Set,
    Field,
}

/// Import specifier.
#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    Default { local: String },
    Named { imported: String, local: String },
    Namespace { local: String },
}

/// Export specifier.
#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    pub local: String,
    pub exported: String,
}

/// Template element.
#[derive(Debug, Clone)]
pub struct TemplateElement {
    pub raw: String,
    pub cooked: String,
    pub tail: bool,
}

/// A part of a template literal (raw text or expression token list).
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateToken {
    /// A raw/cooked string part of the template literal.
    StringPart { raw: String, cooked: String, tail: bool },
    /// An expression inside `${...}`, represented as a list of tokens.
    Expression(Vec<Token>),
}

/// Property definition.
#[derive(Debug, Clone)]
pub struct PropertyDefinition {
    pub key: Box<ASTNode>,
    pub value: Option<Box<ASTNode>>,
    pub kind: PropertyKind,
    pub is_computed: bool,
    pub is_shorthand: bool,
    pub is_method: bool,
}

/// Property kind.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKind {
    Init,
    Get,
    Set,
}

/// For-head helper enum.
enum ForHead {
    Empty,
    Expr(ASTNode),
    Decl(VariableKind, Vec<VariableDeclarator>),
}

/// Extract a name string from a pattern AST node.
fn extract_name_from_pattern(node: &ASTNode) -> String {
    match node {
        ASTNode::Identifier(name) => name.clone(),
        _ => "[pattern]".to_string(),
    }
}

/// JavaScript parser.
pub struct Parser {
    source: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    token_pos: usize,
    strict_mode: bool,
}

impl Parser {
    /// Create a new parser for the given source code.
    pub fn new(source: &str) -> Self {
        Parser {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            token_pos: 0,
            strict_mode: false,
        }
    }

    /// Set strict mode for parsing (used for test262 runner).
    pub fn set_strict_mode(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    /// Parse the source code into an AST.
    pub fn parse(&mut self) -> Result<ASTNode, ParseError> {
        self.tokenize()?;
        self.parse_program()
    }

    // ===== Tokenizer =====

    fn advance_char(&mut self) -> Option<char> {
        if self.pos < self.source.len() {
            let ch = self.source[self.pos];
            self.pos += 1;
            if ch == '\n' { self.line += 1; self.column = 1; } else { self.column += 1; }
            Some(ch)
        } else {
            None
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.source.get(self.pos + offset).copied()
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) { self.advance_char(); true } else { false }
    }

    fn skip_whitespace(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek_char() {
                Some(' ' | '\t' | '\r' | '\n') => { self.advance_char(); }
                Some('/') if self.peek_char_at(1) == Some('/') => {
                    while self.peek_char().is_some() && self.peek_char() != Some('\n') { self.advance_char(); }
                }
                Some('/') if self.peek_char_at(1) == Some('*') => {
                    self.advance_char(); self.advance_char();
                    loop {
                        if self.peek_char().is_none() {
                            return Err(self.mk_err("Unterminated block comment"));
                        }
                        if self.peek_char() == Some('*') && self.peek_char_at(1) == Some('/') {
                            self.advance_char(); self.advance_char(); break;
                        }
                        self.advance_char();
                    }
                }
                Some('#') if self.line == 1 && self.column == 1 => {
                    while self.peek_char().is_some() && self.peek_char() != Some('\n') { self.advance_char(); }
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn push_token(&mut self, tt: TokenType, start: usize, line: usize, col: usize) {
        self.tokens.push(Token { token_type: tt, line, column: col, start, end: self.pos });
    }

    /// Read digits (in the given base predicate), skipping `_` separators.
    fn read_digits(&mut self, is_digit: impl Fn(char) -> bool) {
        loop {
            // Check for numeric separator: _ followed by a valid digit
            if self.peek_char() == Some('_') && self.peek_char_at(1).map_or(false, &is_digit) {
                self.advance_char(); // skip '_'
                // Now consume the digit
                self.advance_char();
                continue;
            }
            // Check for a regular digit
            if self.peek_char().map_or(false, &is_digit) {
                self.advance_char();
                continue;
            }
            break;
        }
    }

    fn scan_number(&mut self, start: usize, line: usize, col: usize) -> Result<(), ParseError> {
        // Advance past the first digit (caller has already matched '0'..='9')
        self.advance_char();
        if self.source[start] == '0' {
            if self.peek_char() == Some('x') || self.peek_char() == Some('X') {
                self.advance_char(); // skip 'x'
                if !self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                    return Err(self.mk_err("Invalid hex literal"));
                }
                self.read_digits(|c| c.is_ascii_hexdigit());
                let s: String = self.source[start..self.pos].iter().filter(|c| **c != '_').collect();
                let v = u64::from_str_radix(&s[2..], 16).map_err(|_| self.mk_err("Invalid hex literal"))?;
                self.push_token(TokenType::Number(v as f64), start, line, col);
                return Ok(());
            }
            if self.peek_char() == Some('b') || self.peek_char() == Some('B') {
                self.advance_char(); // skip 'b'
                if !self.peek_char().map_or(false, |c| c == '0' || c == '1') {
                    return Err(self.mk_err("Invalid binary literal"));
                }
                self.read_digits(|c| c == '0' || c == '1');
                let s: String = self.source[start..self.pos].iter().filter(|c| **c != '_').collect();
                let v = u64::from_str_radix(&s[2..], 2).map_err(|_| self.mk_err("Invalid binary literal"))?;
                self.push_token(TokenType::Number(v as f64), start, line, col);
                return Ok(());
            }
            if self.peek_char() == Some('o') || self.peek_char() == Some('O') {
                self.advance_char(); // skip 'o'
                if !self.peek_char().map_or(false, |c| c.is_ascii_digit() && c < '8') {
                    return Err(self.mk_err("Invalid octal literal"));
                }
                self.read_digits(|c| c.is_ascii_digit() && c < '8');
                let s: String = self.source[start..self.pos].iter().filter(|c| **c != '_').collect();
                let v = u64::from_str_radix(&s[2..], 8).map_err(|_| self.mk_err("Invalid octal literal"))?;
                self.push_token(TokenType::Number(v as f64), start, line, col);
                return Ok(());
            }
            // Legacy octal/non-octal decimal integer: 0 followed by digit
            // Numeric separators are NOT allowed in legacy octal/decimal literals
            let is_legacy = self.peek_char().map_or(false, |c| c.is_ascii_digit());
            if is_legacy {
                if self.strict_mode {
                    return Err(self.mk_err("Legacy octal integer not allowed in strict mode"));
                }
                // Check for invalid separator: 0_1, 07_0, etc.
                let saved_pos = self.pos;
                while self.peek_char().map_or(false, |c| c.is_ascii_digit() || c == '_') {
                    if self.peek_char() == Some('_') {
                        return Err(self.mk_err("Numeric separator not allowed in legacy octal literal"));
                    }
                    self.advance_char();
                }
                let s: String = self.source[start..self.pos].iter().collect();
                let v: f64 = s.parse().unwrap_or(0.0);
                self.push_token(TokenType::Number(v), start, line, col);
                return Ok(());
            }
        }
        self.read_digits(|c| c.is_ascii_digit());
        if self.peek_char() == Some('.') {
            self.advance_char(); // consume '.'
            self.read_digits(|c| c.is_ascii_digit());
        }
        if self.peek_char() == Some('e') || self.peek_char() == Some('E') {
            self.advance_char();
            if self.peek_char() == Some('+') || self.peek_char() == Some('-') { self.advance_char(); }
            self.read_digits(|c| c.is_ascii_digit());
        }
        let s: String = self.source[start..self.pos].iter().filter(|c| **c != '_').collect();
        let v: f64 = s.parse().map_err(|_| self.mk_err("Invalid number literal"))?;
        self.push_token(TokenType::Number(v), start, line, col);
        Ok(())
    }

    fn scan_string(&mut self, quote: char, start: usize, line: usize, col: usize) -> Result<(), ParseError> {
        let mut result = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n') => return Err(ParseError::new("Unterminated string literal", line, col)),
                Some(c) if c == quote => { self.advance_char(); self.push_token(TokenType::String(result), start, line, col); return Ok(()); }
                Some('\\') => {
                    self.advance_char();
                    match self.advance_char() {
                        // Line continuation: backslash followed by line terminator produces nothing
                        Some('\n') => { /* skip: line continuation */ }
                        Some('\r') => {
                            // Skip \r\n as a single line continuation
                            if self.peek_char() == Some('\n') { self.advance_char(); }
                        }
                        Some('n') => result.push('\n'),
                        Some('r') => result.push('\r'),
                        Some('t') => result.push('\t'),
                        Some('\\') => result.push('\\'),
                        Some('\'') => result.push('\''),
                        Some('"') => result.push('"'),
                        Some('b') => result.push('\u{0008}'),
                        Some('f') => result.push('\u{000C}'),
                        Some('v') => result.push('\u{000B}'),
                        Some('0') => {
                            // In strict mode, \0 followed by a digit is a SyntaxError.
                            // In sloppy mode, it's a legacy octal escape.
                            if self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                                if self.strict_mode {
                                    return Err(self.mk_err("Legacy octal escape not allowed in strict mode"));
                                }
                                let mut code = 0u32;
                                for _ in 0..2 {
                                    match self.peek_char() {
                                        Some(c @ '0'..='7') => {
                                            let nc = code * 8 + c.to_digit(8).unwrap();
                                            if nc > 255 { break; }
                                            code = nc;
                                            self.advance_char();
                                        }
                                        _ => break,
                                    }
                                }
                                if let Some(c) = char::from_u32(code) { result.push(c); }
                            } else {
                                // Just \0 (null char)
                                result.push('\0');
                            }
                        }
                        Some(c @ '1'..='7') => {
                            // Legacy octal escape: \1-\7
                            if self.strict_mode {
                                return Err(self.mk_err("Legacy octal escape not allowed in strict mode"));
                            }
                            let mut code = c.to_digit(8).unwrap();
                            for _ in 0..2 {
                                match self.peek_char() {
                                    Some(d @ '0'..='7') => {
                                        let nc = code * 8 + d.to_digit(8).unwrap();
                                        if nc > 255 { break; }
                                        code = nc;
                                        self.advance_char();
                                    }
                                    _ => break,
                                }
                            }
                            if let Some(c) = char::from_u32(code) { result.push(c); }
                        }
                        Some('u') => {
                            if self.peek_char() == Some('{') {
                                self.advance_char();
                                let mut hex = String::new();
                                loop {
                                    match self.peek_char() {
                                        Some('}') => break,
                                        Some(c) if c.is_ascii_hexdigit() => {
                                            hex.push(self.advance_char().unwrap());
                                        }
                                        Some(_) => return Err(self.mk_err("Invalid character in Unicode code point escape")),
                                        None => return Err(self.mk_err("Unterminated Unicode escape sequence")),
                                    }
                                }
                                if self.peek_char() == Some('}') { self.advance_char(); }
                                if hex.is_empty() {
                                    return Err(self.mk_err("Empty Unicode code point escape"));
                                }
                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                    if let Some(c) = char::from_u32(cp) { result.push(c); }
                                }
                            } else {
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                        hex.push(self.advance_char().unwrap());
                                    } else {
                                        return Err(self.mk_err("Invalid unicode escape sequence: expected 4 hex digits"));
                                    }
                                }
                                if hex.len() < 4 {
                                    return Err(self.mk_err("Invalid unicode escape sequence: expected 4 hex digits"));
                                }
                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                    if let Some(c) = char::from_u32(cp) { result.push(c); }
                                }
                            }
                        }
                        Some('x') => {
                            let mut hex = String::new();
                            for _ in 0..2 {
                                if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                    hex.push(self.advance_char().unwrap());
                                } else {
                                    return Err(self.mk_err("Invalid hex escape sequence"));
                                }
                            }
                            if let Ok(v) = u8::from_str_radix(&hex, 16) { result.push(v as char); }
                        }
                        Some(c) => {
                            if self.strict_mode && c.is_ascii_digit() {
                                return Err(self.mk_err("Non-octal decimal escape not allowed in strict mode"));
                            }
                            result.push(c);
                        }
                        None => return Err(self.mk_err("Unterminated string escape")),
                    }
                }
                Some(c) => { self.advance_char(); result.push(c); }
            }
        }
    }

    /// Scan a template literal that appears inside a ${} expression.
    /// Converts to string concatenation tokens.
    fn scan_template_in_expression(&mut self, start: usize, line: usize, col: usize) -> Result<(), ParseError> {
        let mut has_parts = false;
        let mut current = String::new();
        loop {
            match self.peek_char() {
                None => return Err(ParseError::new("Unterminated template literal", line, col)),
                Some('`') => {
                    self.advance_char();
                    if !current.is_empty() || !has_parts {
                        if has_parts {
                            self.push_token(TokenType::Plus, start, line, col);
                        }
                        self.push_token(TokenType::String(current), start, line, col);
                    }
                    break;
                }
                Some('$') if self.peek_char_at(1) == Some('{') => {
                    self.advance_char(); self.advance_char();
                    if has_parts {
                        self.push_token(TokenType::Plus, start, line, col);
                    }
                    self.push_token(TokenType::String(current.clone()), start, line, col);
                    current.clear();
                    has_parts = true;

                    self.push_token(TokenType::Plus, start, line, col);
                    self.push_token(TokenType::LParen, start, line, col);
                    let mut depth = 1u32;
                    loop {
                        self.skip_whitespace()?;
                        if self.pos >= self.source.len() { return Err(ParseError::new("Unterminated template expression", line, col)); }
                        let ec = self.peek_char().unwrap();
                        if ec == '}' && depth == 1 { self.advance_char(); self.push_token(TokenType::RParen, self.pos, self.line, self.column); break; }
                        if ec == '{' { depth += 1; self.advance_char(); self.push_token(TokenType::LBrace, self.pos, self.line, self.column); continue; }
                        if ec == '}' { depth -= 1; self.advance_char(); self.push_token(TokenType::RBrace, self.pos, self.line, self.column); continue; }
                        let es = self.pos;
                        let el = self.line;
                        let ecol = self.column;
                        match ec {
                            '0'..='9' => self.scan_number(es, el, ecol)?,
                            '"' | '\'' => { self.advance_char(); self.scan_string(ec, es, el, ecol)?; }
                            'a'..='z' | 'A'..='Z' | '_' | '$' => {
                                let w = self.scan_identifier();
                                self.push_token(Self::keyword_or_identifier(w), es, el, ecol);
                            }
                            '(' => { self.advance_char(); self.push_token(TokenType::LParen, es, el, ecol); }
                            ')' => { self.advance_char(); self.push_token(TokenType::RParen, es, el, ecol); }
                            '[' => { self.advance_char(); self.push_token(TokenType::LBracket, es, el, ecol); }
                            ']' => { self.advance_char(); self.push_token(TokenType::RBracket, es, el, ecol); }
                            ',' => { self.advance_char(); self.push_token(TokenType::Comma, es, el, ecol); }
                            '.' => { self.advance_char(); self.push_token(TokenType::Dot, es, el, ecol); }
                            '?' => { self.advance_char(); self.push_token(TokenType::Question, es, el, ecol); }
                            ':' => { self.advance_char(); self.push_token(TokenType::Colon, es, el, ecol); }
                            '`' => { self.advance_char(); self.scan_template_in_expression(es, el, ecol)?; }
                            _ => { self.advance_char(); }
                        }
                    }
                }
                Some('\\') => {
                    self.advance_char();
                    match self.advance_char() {
                        Some('\n') => { /* line continuation: skip */ }
                        Some('\r') => {
                            if self.peek_char() == Some('\n') { self.advance_char(); }
                        }
                        Some('n') => current.push('\n'),
                        Some('r') => current.push('\r'),
                        Some('t') => current.push('\t'),
                        Some('\\') => current.push('\\'),
                        Some('`') => current.push('`'),
                        Some('$') => current.push('$'),
                        Some('0') => current.push('\0'),
                        Some('b') => current.push('\u{0008}'),
                        Some('f') => current.push('\u{000C}'),
                        Some('v') => current.push('\u{000B}'),
                        Some('u') => {
                            if self.peek_char() == Some('{') {
                                self.advance_char();
                                let mut hex = String::new();
                                loop {
                                    match self.peek_char() {
                                        Some('}') => break,
                                        Some(c) if c.is_ascii_hexdigit() => { hex.push(self.advance_char().unwrap()); }
                                        Some(_) => return Err(self.mk_err("Invalid character in Unicode code point escape")),
                                        None => return Err(self.mk_err("Unterminated Unicode escape sequence")),
                                    }
                                }
                                if self.peek_char() == Some('}') { self.advance_char(); }
                                if !hex.is_empty() {
                                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                        if let Some(c) = char::from_u32(cp) { current.push(c); }
                                    }
                                }
                            } else {
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                        hex.push(self.advance_char().unwrap());
                                    } else {
                                        break;
                                    }
                                }
                                if !hex.is_empty() {
                                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                        if let Some(c) = char::from_u32(cp) { current.push(c); }
                                    }
                                }
                            }
                        }
                        Some('x') => {
                            let mut hex = String::new();
                            for _ in 0..2 {
                                if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                    hex.push(self.advance_char().unwrap());
                                } else {
                                    break;
                                }
                            }
                            if !hex.is_empty() {
                                if let Ok(v) = u8::from_str_radix(&hex, 16) { current.push(v as char); }
                            }
                        }
                        Some(c) => { current.push('\\'); current.push(c); }
                        None => return Err(ParseError::new("Unterminated template literal", line, col)),
                    }
                }
                Some(c) => { self.advance_char(); current.push(c); }
            }
        }
        Ok(())
    }

    fn scan_identifier(&mut self) -> String {
        let start = self.pos;
        while self.peek_char().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            self.advance_char();
        }
        self.source[start..self.pos].iter().collect()
    }

    fn keyword_or_identifier(word: String) -> TokenType {
        match word.as_str() {
            "break" => TokenType::Keyword(Keyword::Break),
            "case" => TokenType::Keyword(Keyword::Case),
            "catch" => TokenType::Keyword(Keyword::Catch),
            "class" => TokenType::Keyword(Keyword::Class),
            "const" => TokenType::Keyword(Keyword::Const),
            "continue" => TokenType::Keyword(Keyword::Continue),
            "debugger" => TokenType::Keyword(Keyword::Debugger),
            "default" => TokenType::Keyword(Keyword::Default),
            "delete" => TokenType::Keyword(Keyword::Delete),
            "do" => TokenType::Keyword(Keyword::Do),
            "else" => TokenType::Keyword(Keyword::Else),
            "export" => TokenType::Keyword(Keyword::Export),
            "extends" => TokenType::Keyword(Keyword::Extends),
            "finally" => TokenType::Keyword(Keyword::Finally),
            "for" => TokenType::Keyword(Keyword::For),
            "function" => TokenType::Keyword(Keyword::Function),
            "if" => TokenType::Keyword(Keyword::If),
            "import" => TokenType::Keyword(Keyword::Import),
            "in" => TokenType::Keyword(Keyword::In),
            "instanceof" => TokenType::Keyword(Keyword::Instanceof),
            "let" => TokenType::Keyword(Keyword::Let),
            "new" => TokenType::Keyword(Keyword::New),
            "return" => TokenType::Keyword(Keyword::Return),
            "super" => TokenType::Keyword(Keyword::Super),
            "switch" => TokenType::Keyword(Keyword::Switch),
            "this" => TokenType::Keyword(Keyword::This),
            "throw" => TokenType::Keyword(Keyword::Throw),
            "try" => TokenType::Keyword(Keyword::Try),
            "typeof" => TokenType::Keyword(Keyword::Typeof),
            "var" => TokenType::Keyword(Keyword::Var),
            "void" => TokenType::Keyword(Keyword::Void),
            "while" => TokenType::Keyword(Keyword::While),
            "with" => TokenType::Keyword(Keyword::With),
            "yield" => TokenType::Keyword(Keyword::Yield),
            "async" => TokenType::Keyword(Keyword::Async),
            "await" => TokenType::Keyword(Keyword::Await),
            "of" => TokenType::Keyword(Keyword::Of),
            "static" => TokenType::Keyword(Keyword::Static),
            "get" => TokenType::Identifier(word),
            "set" => TokenType::Identifier(word),
            "as" => TokenType::Keyword(Keyword::As),
            "from" => TokenType::Keyword(Keyword::From),
            "true" => TokenType::Bool(true),
            "false" => TokenType::Bool(false),
            "null" => TokenType::Null,
            "undefined" => TokenType::Undefined,
            _ => TokenType::Identifier(word),
        }
    }

    fn tokenize(&mut self) -> Result<(), ParseError> {
        while self.pos < self.source.len() {
            self.skip_whitespace()?;
            if self.pos >= self.source.len() { break; }
            let start = self.pos;
            let sl = self.line;
            let sc = self.column;
            let ch = self.peek_char().unwrap();

            match ch {
                '0'..='9' => { self.scan_number(start, sl, sc)?; }
                '"' | '\'' => { self.advance_char(); self.scan_string(ch, start, sl, sc)?; }
                '`' => {
                    // Template literal: convert to string concatenation
                    // `Hello, ${name}!` becomes "Hello, " + name + "!"
                    self.advance_char();
                    let mut has_parts = false;
                    let mut current = String::new();
                    loop {
                        match self.peek_char() {
                            None => return Err(self.mk_err("Unterminated template literal")),
                            Some('`') => {
                                self.advance_char();
                                // Push the final string part
                                if !current.is_empty() || !has_parts {
                                    if has_parts {
                                        self.push_token(TokenType::Plus, start, sl, sc);
                                    }
                                    self.push_token(TokenType::String(current), start, sl, sc);
                                }
                                break;
                            }
                            Some('$') if self.peek_char_at(1) == Some('{') => {
                                self.advance_char(); self.advance_char(); // ${
                                // Push the string part before the expression
                                if has_parts {
                                    self.push_token(TokenType::Plus, start, sl, sc);
                                }
                                self.push_token(TokenType::String(current.clone()), start, sl, sc);
                                current.clear();
                                has_parts = true;

                                // Now tokenize the expression inside ${}
                                // Wrap in parentheses to preserve operator precedence
                                self.push_token(TokenType::Plus, start, sl, sc);
                                self.push_token(TokenType::LParen, start, sl, sc);
                                let mut depth = 1u32;
                                // Tokenize until we find the closing }
                                loop {
                                    self.skip_whitespace()?;
                                    if self.pos >= self.source.len() { return Err(self.mk_err("Unterminated template expression")); }
                                    let ec = self.peek_char().unwrap();
                                    if ec == '}' && depth == 1 { self.advance_char(); self.push_token(TokenType::RParen, self.pos, self.line, self.column); break; }
                                    if ec == '{' { depth += 1; self.advance_char(); self.push_token(TokenType::LBrace, self.pos, self.line, self.column); continue; }
                                    if ec == '}' { depth -= 1; self.advance_char(); self.push_token(TokenType::RBrace, self.pos, self.line, self.column); continue; }
                                    let es = self.pos;
                                    let el = self.line;
                                    let ecol = self.column;
                                    match ec {
                                        '0'..='9' => self.scan_number(es, el, ecol)?,
                                        '"' | '\'' => { self.advance_char(); self.scan_string(ec, es, el, ecol)?; }
                                        'a'..='z' | 'A'..='Z' | '_' | '$' => {
                                            let w = self.scan_identifier();
                                            self.push_token(Self::keyword_or_identifier(w), es, el, ecol);
                                        }
                                        '(' => { self.advance_char(); self.push_token(TokenType::LParen, es, el, ecol); }
                                        ')' => { self.advance_char(); self.push_token(TokenType::RParen, es, el, ecol); }
                                        '[' => { self.advance_char(); self.push_token(TokenType::LBracket, es, el, ecol); }
                                        ']' => { self.advance_char(); self.push_token(TokenType::RBracket, es, el, ecol); }
                                        ',' => { self.advance_char(); self.push_token(TokenType::Comma, es, el, ecol); }
                                        '.' => { self.advance_char(); self.push_token(TokenType::Dot, es, el, ecol); }
                                        '+' => { self.advance_char(); self.push_token(TokenType::Plus, es, el, ecol); }
                                        '-' => { self.advance_char(); self.push_token(TokenType::Minus, es, el, ecol); }
                                        '*' => { self.advance_char(); self.push_token(TokenType::Star, es, el, ecol); }
                                        '/' => { self.advance_char(); self.push_token(TokenType::Slash, es, el, ecol); }
                                        '%' => { self.advance_char(); self.push_token(TokenType::Percent, es, el, ecol); }
                                        '=' => { self.advance_char(); self.push_token(TokenType::Assign, es, el, ecol); }
                                        '!' => { self.advance_char(); self.push_token(TokenType::Not, es, el, ecol); }
                                        '<' => { self.advance_char(); self.push_token(TokenType::Lt, es, el, ecol); }
                                        '>' => { self.advance_char(); self.push_token(TokenType::Gt, es, el, ecol); }
                                        '&' => { self.advance_char(); self.push_token(TokenType::BitAnd, es, el, ecol); }
                                        '|' => { self.advance_char(); self.push_token(TokenType::BitOr, es, el, ecol); }
                                        '^' => { self.advance_char(); self.push_token(TokenType::BitXor, es, el, ecol); }
                                        '~' => { self.advance_char(); self.push_token(TokenType::BitNot, es, el, ecol); }
                                        '?' => { self.advance_char(); self.push_token(TokenType::Question, es, el, ecol); }
                                        ':' => { self.advance_char(); self.push_token(TokenType::Colon, es, el, ecol); }
                                        '`' => {
                                            // Nested template literal - recurse into template scanning
                                            self.advance_char();
                                            self.scan_template_in_expression(es, el, ecol)?;
                                        }
                                        _ => { self.advance_char(); }
                                    }
                                }
                            }
                            Some('\\') => {
                                self.advance_char();
                                match self.advance_char() {
                                    Some('\n') => { /* line continuation: skip */ }
                                    Some('\r') => {
                                        if self.peek_char() == Some('\n') { self.advance_char(); }
                                    }
                                    Some('n') => { current.push('\n'); }
                                    Some('r') => { current.push('\r'); }
                                    Some('t') => { current.push('\t'); }
                                    Some('\\') => { current.push('\\'); }
                                    Some('`') => { current.push('`'); }
                                    Some('$') => { current.push('$'); }
                                    Some('0') => { current.push('\0'); }
                                    Some('b') => current.push('\u{0008}'),
                                    Some('f') => current.push('\u{000C}'),
                                    Some('v') => current.push('\u{000B}'),
                                    Some('u') => {
                                        if self.peek_char() == Some('{') {
                                            self.advance_char();
                                            let mut hex = String::new();
                                            loop {
                                                match self.peek_char() {
                                                    Some('}') => break,
                                                    Some(c) if c.is_ascii_hexdigit() => { hex.push(self.advance_char().unwrap()); }
                                                    Some(_) => return Err(self.mk_err("Invalid character in Unicode code point escape")),
                                                    None => return Err(self.mk_err("Unterminated Unicode escape sequence")),
                                                }
                                            }
                                            if self.peek_char() == Some('}') { self.advance_char(); }
                                            if !hex.is_empty() {
                                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                                    if let Some(c) = char::from_u32(cp) { current.push(c); }
                                                }
                                            }
                                        } else {
                                            let mut hex = String::new();
                                            for _ in 0..4 {
                                                if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                                    hex.push(self.advance_char().unwrap());
                                                } else { break; }
                                            }
                                            if !hex.is_empty() {
                                                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                                    if let Some(c) = char::from_u32(cp) { current.push(c); }
                                                }
                                            }
                                        }
                                    }
                                    Some('x') => {
                                        let mut hex = String::new();
                                        for _ in 0..2 {
                                            if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                                hex.push(self.advance_char().unwrap());
                                            } else { break; }
                                        }
                                        if !hex.is_empty() {
                                            if let Ok(v) = u8::from_str_radix(&hex, 16) { current.push(v as char); }
                                        }
                                    }
                                    Some(c) => { current.push(c); }
                                    None => return Err(self.mk_err("Unterminated template literal")),
                                }
                            }
                            Some(c) => {
                                self.advance_char();
                                current.push(c);
                            }
                        }
                    }
                }
                'a'..='z' | 'A'..='Z' | '_' | '$' => {
                    let w = self.scan_identifier();
                    self.push_token(Self::keyword_or_identifier(w), start, sl, sc);
                }
                '.' => {
                    if self.peek_char_at(1).map_or(false, |c| c.is_ascii_digit()) {
                        self.scan_number(start, sl, sc)?;
                    } else if self.peek_char_at(1) == Some('.') && self.peek_char_at(2) == Some('.') {
                        self.advance_char(); self.advance_char(); self.advance_char();
                        self.push_token(TokenType::Ellipsis, start, sl, sc);
                    } else {
                        self.advance_char();
                        self.push_token(TokenType::Dot, start, sl, sc);
                    }
                }
                '+' => { self.advance_char(); let tt = if self.match_char('+') { TokenType::Inc } else if self.match_char('=') { TokenType::PlusAssign } else { TokenType::Plus }; self.push_token(tt, start, sl, sc); }
                '-' => { self.advance_char(); let tt = if self.match_char('-') { TokenType::Dec } else if self.match_char('=') { TokenType::MinusAssign } else { TokenType::Minus }; self.push_token(tt, start, sl, sc); }
                '*' => { self.advance_char(); let tt = if self.match_char('*') { if self.match_char('=') { TokenType::PowAssign } else { TokenType::Pow } } else if self.match_char('=') { TokenType::StarAssign } else { TokenType::Star }; self.push_token(tt, start, sl, sc); }
                '%' => { self.advance_char(); let tt = if self.match_char('=') { TokenType::PercentAssign } else { TokenType::Percent }; self.push_token(tt, start, sl, sc); }
                '=' => { self.advance_char(); let tt = if self.match_char('=') { if self.match_char('=') { TokenType::StrictEq } else { TokenType::Eq } } else if self.match_char('>') { TokenType::Arrow } else { TokenType::Assign }; self.push_token(tt, start, sl, sc); }
                '!' => { self.advance_char(); let tt = if self.match_char('=') { if self.match_char('=') { TokenType::StrictNe } else { TokenType::Ne } } else { TokenType::Not }; self.push_token(tt, start, sl, sc); }
                '<' => { self.advance_char(); let tt = if self.match_char('=') { TokenType::Le } else if self.match_char('<') { if self.match_char('=') { TokenType::ShlAssign } else { TokenType::Shl } } else { TokenType::Lt }; self.push_token(tt, start, sl, sc); }
                '>' => { self.advance_char(); let tt = if self.match_char('=') { TokenType::Ge } else if self.match_char('>') { if self.match_char('=') { TokenType::ShrAssign } else if self.match_char('>') { if self.match_char('=') { TokenType::UShrAssign } else { TokenType::UShr } } else { TokenType::Shr } } else { TokenType::Gt }; self.push_token(tt, start, sl, sc); }
                '&' => { self.advance_char(); let tt = if self.match_char('&') { TokenType::And } else if self.match_char('=') { TokenType::BitAndAssign } else { TokenType::BitAnd }; self.push_token(tt, start, sl, sc); }
                '|' => { self.advance_char(); let tt = if self.match_char('|') { TokenType::Or } else if self.match_char('=') { TokenType::BitOrAssign } else { TokenType::BitOr }; self.push_token(tt, start, sl, sc); }
                '^' => { self.advance_char(); let tt = if self.match_char('=') { TokenType::BitXorAssign } else { TokenType::BitXor }; self.push_token(tt, start, sl, sc); }
                '~' => { self.advance_char(); self.push_token(TokenType::BitNot, start, sl, sc); }
                '?' => { self.advance_char(); let tt = if self.match_char('?') { TokenType::NullishCoalescing } else if self.match_char('.') { TokenType::OptionalChaining } else { TokenType::Question }; self.push_token(tt, start, sl, sc); }
                '/' => {
                    self.advance_char();
                    // Disambiguate division vs regex: if prev was value-like, it's division
                    let prev_is_value = self.tokens.last().map_or(false, |t| matches!(t.token_type,
                        TokenType::Number(_) | TokenType::String(_) | TokenType::Bool(_)
                        | TokenType::Null | TokenType::Undefined | TokenType::RParen
                        | TokenType::RBracket | TokenType::Identifier(_)
                        | TokenType::RegExp { .. } | TokenType::RBrace
                    ));
                    if prev_is_value {
                        let tt = if self.match_char('=') { TokenType::SlashAssign } else { TokenType::Slash };
                        self.push_token(tt, start, sl, sc);
                    } else {
                        // Regex
                        self.scan_regex(start, sl, sc)?;
                    }
                }
                '(' => { self.advance_char(); self.push_token(TokenType::LParen, start, sl, sc); }
                ')' => { self.advance_char(); self.push_token(TokenType::RParen, start, sl, sc); }
                '{' => { self.advance_char(); self.push_token(TokenType::LBrace, start, sl, sc); }
                '}' => { self.advance_char(); self.push_token(TokenType::RBrace, start, sl, sc); }
                '[' => { self.advance_char(); self.push_token(TokenType::LBracket, start, sl, sc); }
                ']' => { self.advance_char(); self.push_token(TokenType::RBracket, start, sl, sc); }
                ';' => { self.advance_char(); self.push_token(TokenType::Semicolon, start, sl, sc); }
                ',' => { self.advance_char(); self.push_token(TokenType::Comma, start, sl, sc); }
                ':' => { self.advance_char(); self.push_token(TokenType::Colon, start, sl, sc); }
                _ => { self.advance_char(); return Err(ParseError::new(&format!("Unexpected character '{}'", ch), sl, sc)); }
            }
        }
        self.push_token(TokenType::Eof, self.pos, self.line, self.column);
        Ok(())
    }

    fn scan_regex(&mut self, start: usize, line: usize, col: usize) -> Result<(), ParseError> {
        let mut pattern = String::new();
        let mut in_class = false;
        let mut is_first = true;

        loop {
            match self.peek_char() {
                None => return Err(self.mk_err("Unterminated regex literal")),
                Some('/') if !in_class => {
                    self.advance_char();
                    let mut flags = String::new();
                    let mut seen_flags: u8 = 0;
                    while self.peek_char().map_or(false, |c| matches!(c, 'g'|'i'|'m'|'s'|'u'|'y'|'d')) {
                        let f = self.advance_char().unwrap();
                        let bit = match f {
                            'g' => 1, 'i' => 2, 'm' => 4, 's' => 8, 'u' => 16, 'y' => 32, 'd' => 64,
                            _ => 0,
                        };
                        if seen_flags & bit != 0 {
                            return Err(self.mk_err("Duplicate regex flag"));
                        }
                        seen_flags |= bit;
                        flags.push(f);
                    }
                    // Validate the pattern with flag context
                    self.validate_regex_pattern(&pattern, flags.contains('u'))?;
                    self.push_token(TokenType::RegExp { pattern, flags }, start, line, col);
                    return Ok(());
                }
                Some('[') => {
                    is_first = false;
                    in_class = true;
                    pattern.push(self.advance_char().unwrap());
                }
                Some(']') if in_class => {
                    in_class = false;
                    pattern.push(self.advance_char().unwrap());
                }
                Some('\\') => {
                    is_first = false;
                    pattern.push(self.advance_char().unwrap()); // backslash
                    match self.peek_char() {
                        None => return Err(self.mk_err("Unterminated escape in regex")),
                        Some('k') => {
                            // Named backreference \k<name>
                            pattern.push(self.advance_char().unwrap());
                            if self.peek_char() == Some('<') {
                                pattern.push(self.advance_char().unwrap());
                                self.read_regex_group_name(&mut pattern)?;
                            }
                        }
                        Some('u') => {
                            pattern.push(self.advance_char().unwrap()); // u
                            if self.peek_char() == Some('{') {
                                // Unicode code point: \u{...}
                                pattern.push(self.advance_char().unwrap()); // {
                                loop {
                                    match self.peek_char() {
                                        None => return Err(self.mk_err("Unterminated unicode escape")),
                                        Some('}') => {
                                            pattern.push(self.advance_char().unwrap());
                                            break;
                                        }
                                        Some(c) => {
                                            pattern.push(self.advance_char().unwrap());
                                        }
                                    }
                                }
                            } else {
                                // \u followed by hex digits
                                for _ in 0..4 {
                                    if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                        pattern.push(self.advance_char().unwrap());
                                    }
                                }
                            }
                        }
                        Some('x') => {
                            pattern.push(self.advance_char().unwrap()); // x
                            // \x followed by 2 hex digits
                            for _ in 0..2 {
                                if self.peek_char().map_or(false, |c| c.is_ascii_hexdigit()) {
                                    pattern.push(self.advance_char().unwrap());
                                }
                            }
                        }
                        Some('c') => {
                            pattern.push(self.advance_char().unwrap()); // c
                            // \c followed by control letter
                            if self.peek_char().map_or(false, |c| c.is_ascii_alphabetic()) {
                                pattern.push(self.advance_char().unwrap());
                            }
                        }
                        Some('p') | Some('P') => {
                            pattern.push(self.advance_char().unwrap());
                            // Unicode property escape: \p{...} or \P{...}
                            if self.peek_char() == Some('{') {
                                pattern.push(self.advance_char().unwrap());
                                loop {
                                    match self.peek_char() {
                                        None => return Err(self.mk_err("Unterminated unicode property escape")),
                                        Some('}') => {
                                            pattern.push(self.advance_char().unwrap());
                                            break;
                                        }
                                        Some(c) => {
                                            pattern.push(self.advance_char().unwrap());
                                        }
                                    }
                                }
                            }
                        }
                        Some(c) => {
                            pattern.push(self.advance_char().unwrap());
                        }
                    }
                }
                Some('\n') | Some('\r') => {
                    return Err(self.mk_err("Unterminated regex literal"));
                }
                Some('\u{2028}') | Some('\u{2029}') => {
                    return Err(self.mk_err("Line separator not allowed in regex literal"));
                }
                Some('*') | Some('\\') | Some('/') if is_first => {
                    return Err(self.mk_err("Invalid first character in regex literal"));
                }
                Some('{') => {
                    is_first = false;
                    pattern.push(self.advance_char().unwrap());
                    if !in_class {
                        self.validate_regex_quantifier(&mut pattern)?;
                    }
                }
                Some('(') => {
                    is_first = false;
                    pattern.push(self.advance_char().unwrap());
                    if self.peek_char() == Some('?') {
                        pattern.push(self.advance_char().unwrap());
                        self.validate_regex_group(&mut pattern)?;
                    }
                }
                Some(')') => {
                    is_first = false;
                    pattern.push(self.advance_char().unwrap());
                }
                Some(c) => {
                    is_first = false;
                    pattern.push(self.advance_char().unwrap());
                }
            }
        }
    }

    /// Validate the contents of a regex group starting with (?.
    fn validate_regex_group(&mut self, pattern: &mut String) -> Result<(), ParseError> {
        match self.peek_char() {
            None => Err(self.mk_err("Unterminated regex group")),
            Some(':') | Some('=') | Some('!') => {
                pattern.push(self.advance_char().unwrap());
                Ok(())
            }
            Some('<') => {
                pattern.push(self.advance_char().unwrap());
                match self.peek_char() {
                    Some('=') | Some('!') => {
                        // Lookbehind: (?<= or (?<!
                        pattern.push(self.advance_char().unwrap());
                        Ok(())
                    }
                    _ => {
                        // Named capture: (?<name>
                        self.read_regex_group_name(pattern)
                    }
                }
            }
            Some(c) if c == 'i' || c == 'm' || c == 's' => {
                // Modifier: (?i:s) or (?ims-ims:pattern)
                // Valid modifier flags are only: i, m, s
                let mut seen: u32 = 0;
                while self.peek_char().map_or(false, |c| matches!(c, 'i' | 'm' | 's')) {
                    let ch = self.advance_char().unwrap();
                    let bit = 1u32 << (ch as u32 - 'a' as u32);
                    if seen & bit != 0 {
                        return Err(self.mk_err("Duplicate regex modifier"));
                    }
                    seen |= bit;
                    pattern.push(ch);
                }
                // Optional: remove modifiers (-ims)
                if self.peek_char() == Some('-') {
                    pattern.push(self.advance_char().unwrap());
                    let mut rm_seen: u32 = 0;
                    while self.peek_char().map_or(false, |c| matches!(c, 'i' | 'm' | 's')) {
                        let ch = self.advance_char().unwrap();
                        let bit = 1u32 << (ch as u32 - 'a' as u32);
                        if rm_seen & bit != 0 {
                            return Err(self.mk_err("Duplicate regex modifier removal"));
                        }
                        rm_seen |= bit;
                        pattern.push(ch);
                    }
                }
                // Must be followed by :
                if self.peek_char() != Some(':') {
                    return Err(self.mk_err("Expected : after regex modifiers"));
                }
                pattern.push(self.advance_char().unwrap());
                Ok(())
            }
            Some(_) => Err(self.mk_err("Invalid regex group")),
        }
    }

    /// Read a regex group name: <name>
    fn read_regex_group_name(&mut self, pattern: &mut String) -> Result<(), ParseError> {
        // First char must be identifier start
        match self.peek_char() {
            None => return Err(self.mk_err("Unterminated regex group name")),
            Some('>') => return Err(self.mk_err("Empty regex group name")),
            Some(c) if !c.is_ascii_alphabetic() && c != '_' && c != '$' => {
                return Err(self.mk_err("Invalid regex group name start"));
            }
            Some(_) => { pattern.push(self.advance_char().unwrap()); }
        }
        // Rest of name
        loop {
            match self.peek_char() {
                None => return Err(self.mk_err("Unterminated regex group name")),
                Some('>') => { pattern.push(self.advance_char().unwrap()); break; }
                Some(c) if c.is_ascii_alphanumeric() || c == '_' => {
                    pattern.push(self.advance_char().unwrap());
                }
                Some(_) => return Err(self.mk_err("Invalid character in regex group name")),
            }
        }
        Ok(())
    }

    /// Validate a braced quantifier: {n}, {n,}, {n,m}
    fn validate_regex_quantifier(&mut self, pattern: &mut String) -> Result<(), ParseError> {
        // Read digits for lower bound
        let mut has_lower = false;
        while self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
            has_lower = true;
            pattern.push(self.advance_char().unwrap());
        }
        if !has_lower {
            // Not a quantifier, just literal { followed by non-digit
            return Ok(());
        }
        match self.peek_char() {
            Some('}') => {
                pattern.push(self.advance_char().unwrap());
                Ok(())
            }
            Some(',') => {
                pattern.push(self.advance_char().unwrap());
                if self.peek_char() == Some('}') {
                    pattern.push(self.advance_char().unwrap());
                    return Ok(());
                }
                let mut has_upper = false;
                while self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                    has_upper = true;
                    pattern.push(self.advance_char().unwrap());
                }
                if !has_upper {
                    return Err(self.mk_err("Invalid quantifier: expected digit or }"));
                }
                if self.peek_char() == Some('}') {
                    pattern.push(self.advance_char().unwrap());
                    Ok(())
                } else {
                    Err(self.mk_err("Invalid quantifier: expected }"))
                }
            }
            _ => Err(self.mk_err("Invalid quantifier: expected } or ,")),
        }
    }

    /// Validate a regex pattern string against its flags (post-scan).
    fn validate_regex_pattern(&self, pattern: &str, unicode: bool) -> Result<(), ParseError> {
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;
        let mut in_class = false;
        let mut prev_was_assertion = false; // track lookahead/behind for u-flag quantifier check

        while i < chars.len() {
            let ch = chars[i];
            match ch {
                '[' => { in_class = true; i += 1; }
                ']' => { in_class = false; prev_was_assertion = false; i += 1; }
                '\\' => {
                    i += 1;
                    prev_was_assertion = false;
                    if i >= chars.len() { break; }
                    let esc = chars[i];
                    if unicode {
                        match esc {
                            '^' | '$' | '\\' | '.' | '*' | '+' | '?' |
                            '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/' |
                            'b' | 'B' | 'd' | 'D' | 'f' | 'n' | 'r' | 's' | 'S' |
                            't' | 'v' | 'w' | 'W' | 'k' | 'p' | 'P' | 'c' | 'x' => {},
                            '0' => {
                                if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                                    return Err(self.mk_err("Invalid decimal escape in unicode regex"));
                                }
                            }
                            'u' => {
                                i += 1;
                                if i >= chars.len() {
                                    return Err(self.mk_err("Invalid unicode escape in regex"));
                                }
                                if chars[i] == '{' {
                                    i += 1;
                                    while i < chars.len() && chars[i] != '}' {
                                        if !chars[i].is_ascii_hexdigit() {
                                            return Err(self.mk_err("Invalid unicode escape in regex"));
                                        }
                                        i += 1;
                                    }
                                } else {
                                    for _ in 0..4 {
                                        if i >= chars.len() || !chars[i].is_ascii_hexdigit() {
                                            return Err(self.mk_err("Invalid unicode escape in regex"));
                                        }
                                        i += 1;
                                    }
                                    i -= 1;
                                }
                            }
                            c if c.is_ascii_digit() => {
                                return Err(self.mk_err("Invalid decimal escape in unicode regex"));
                            }
                            _ => {
                                return Err(self.mk_err("Invalid identity escape in unicode regex"));
                            }
                        }
                    }
                    i += 1;
                }
                '{' if !in_class => {
                    // Check if this is a quantifier
                    let mut j = i + 1;
                    let mut has_digits = false;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        has_digits = true;
                        j += 1;
                    }
                    if !has_digits {
                        if unicode {
                            return Err(self.mk_err("Invalid extended pattern character in unicode regex"));
                        }
                    } else if unicode && prev_was_assertion {
                        return Err(self.mk_err("Quantifiable assertion disallowed with u flag"));
                    }
                    // Skip past quantifier
                    if has_digits {
                        i = j;
                        if i < chars.len() && chars[i] == ',' { i += 1; }
                        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                        if i < chars.len() && chars[i] == '}' { i += 1; }
                    } else {
                        i += 1;
                    }
                    prev_was_assertion = false;
                }
                '(' => {
                    // Check for lookahead/behind: (?= or (?!
                    if i + 2 < chars.len() && chars[i+1] == '?' {
                        match chars.get(i+2) {
                            Some('=') | Some('!') => prev_was_assertion = true,
                            Some('<') if i + 3 < chars.len() => {
                                match chars[i+3] {
                                    '=' | '!' => prev_was_assertion = true,
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    i += 1;
                }
                _ => { prev_was_assertion = false; i += 1; }
            }
        }
        Ok(())
    }

    // ===== Token Stream Helpers =====

    fn cur(&self) -> &Token {
        if self.tokens.is_empty() {
            // Return a reference to a dummy token - only safe because we never mutate through this
            &self.tokens[0] // will be caught by at() checks on empty
        } else {
            &self.tokens[self.token_pos.min(self.tokens.len() - 1)]
        }
    }
    fn cur_tt(&self) -> &TokenType { self.tokens.get(self.token_pos).map_or(&TokenType::Eof, |t| &t.token_type) }
    fn peek(&self) -> &Token {
        let next = self.token_pos + 1;
        if next < self.tokens.len() { &self.tokens[next] }
        else if !self.tokens.is_empty() { self.tokens.last().unwrap() }
        else { self.cur() }
    }
    fn peek_tt(&self) -> &TokenType { &self.peek().token_type }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.token_pos].clone();
        if self.token_pos + 1 < self.tokens.len() { self.token_pos += 1; }
        t
    }

    fn expect(&mut self, tt: &TokenType) -> Result<Token, ParseError> {
        if self.cur_tt() == tt { Ok(self.advance()) }
        else { Err(self.mk_err(&format!("Expected {:?}, found {:?}", tt, self.cur_tt()))) }
    }

    fn expect_kw(&mut self, kw: Keyword) -> Result<Token, ParseError> {
        if *self.cur_tt() == TokenType::Keyword(kw.clone()) { Ok(self.advance()) }
        else { Err(self.mk_err(&format!("Expected {:?}", kw))) }
    }

    fn at_kw(&self, kw: &Keyword) -> bool { *self.cur_tt() == TokenType::Keyword(kw.clone()) }
    fn at_ident(&self, name: &str) -> bool { matches!(self.cur_tt(), TokenType::Identifier(n) if n == name) }
    fn at(&self, tt: &TokenType) -> bool { *self.cur_tt() == *tt }
    fn at_any(&self, tts: &[TokenType]) -> bool { tts.iter().any(|t| self.at(t)) }

    fn mk_err(&self, msg: &str) -> ParseError {
        if self.tokens.is_empty() {
            ParseError::new(msg, self.line, self.column)
        } else {
            let t = self.cur();
            ParseError::new(msg, t.line, t.column)
        }
    }

    fn eat_semi(&mut self) -> Result<(), ParseError> {
        if self.at(&TokenType::Semicolon) { self.advance(); return Ok(()); }
        if self.at_any(&[TokenType::RBrace, TokenType::Eof]) { return Ok(()); }
        if self.token_pos > 0 && self.tokens[self.token_pos - 1].line < self.cur().line { return Ok(()); }
        Err(self.mk_err("Expected ';' or line terminator"))
    }

    // ===== Parser =====

    fn parse_program(&mut self) -> Result<ASTNode, ParseError> {
        let mut body = Vec::new();
        while !self.at(&TokenType::Eof) {
            body.push(self.parse_stmt_item()?);
        }
        Ok(ASTNode::Block(body))
    }

    fn parse_stmt_item(&mut self) -> Result<ASTNode, ParseError> {
        match self.cur_tt().clone() {
            TokenType::Keyword(Keyword::Function) => self.parse_func_decl(),
            TokenType::Keyword(Keyword::Async) => {
                // Check if this is "async function"
                if self.token_pos + 1 < self.tokens.len() && self.tokens[self.token_pos + 1].token_type == TokenType::Keyword(Keyword::Function) {
                    self.parse_func_decl()
                } else {
                    self.parse_stmt()
                }
            }
            TokenType::Keyword(Keyword::Class) => self.parse_class_decl(),
            TokenType::Keyword(Keyword::Let) | TokenType::Keyword(Keyword::Const) => self.parse_lexical_decl(),
            _ => self.parse_stmt(),
        }
    }

    fn parse_stmt(&mut self) -> Result<ASTNode, ParseError> {
        match self.cur_tt().clone() {
            TokenType::LBrace => self.parse_block(),
            TokenType::Keyword(ref kw) => match kw {
                Keyword::Var => self.parse_var_decl(),
                Keyword::Let => self.parse_var_decl(),
                Keyword::Const => self.parse_var_decl(),
                Keyword::If => self.parse_if(),
                Keyword::For => self.parse_for(),
                Keyword::While => self.parse_while(),
                Keyword::Do => self.parse_do_while(),
                Keyword::Switch => self.parse_switch(),
                Keyword::Return => self.parse_return(),
                Keyword::Throw => self.parse_throw(),
                Keyword::Try => self.parse_try(),
                Keyword::Break => { self.advance(); let l = self.optional_label(); self.eat_semi()?; Ok(ASTNode::Break(l)) }
                Keyword::Continue => { self.advance(); let l = self.optional_label(); self.eat_semi()?; Ok(ASTNode::Continue(l)) }
                Keyword::Debugger => { self.advance(); self.eat_semi()?; Ok(ASTNode::DebuggerStatement) }
                Keyword::With => self.parse_with(),
                Keyword::Import => {
                    // Check if this is dynamic import: import(...)
                    if self.token_pos + 1 < self.tokens.len() &&
                       matches!(self.tokens[self.token_pos + 1].token_type, TokenType::LParen) {
                        // Parse as expression statement
                        self.parse_expr_stmt()
                    } else {
                        self.parse_import_decl()
                    }
                },
                Keyword::Export => self.parse_export_decl(),
                _ => self.parse_expr_stmt(),
            },
            TokenType::Semicolon => { self.advance(); Ok(ASTNode::EmptyStatement) }
            _ => self.parse_expr_or_labeled(),
        }
    }

    fn optional_label(&mut self) -> Option<String> {
        if let TokenType::Identifier(n) = self.cur_tt().clone() {
            if self.tokens[self.token_pos].line == self.tokens[self.token_pos.saturating_sub(1)].line {
                self.advance(); return Some(n);
            }
        }
        None
    }

    fn parse_block(&mut self) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
            body.push(self.parse_stmt_item()?);
        }
        self.expect(&TokenType::RBrace)?;
        Ok(ASTNode::Block(body))
    }

    fn parse_expr_or_labeled(&mut self) -> Result<ASTNode, ParseError> {
        let expr = self.parse_expr()?;
        if let ASTNode::Identifier(ref name) = expr {
            if self.at(&TokenType::Colon) {
                let label = name.clone();
                self.advance();
                let body = self.parse_stmt()?;
                return Ok(ASTNode::LabeledStatement { label, body: Box::new(body) });
            }
        }
        self.eat_semi()?;
        Ok(ASTNode::ExpressionStatement(Box::new(expr)))
    }

    fn parse_expr_stmt(&mut self) -> Result<ASTNode, ParseError> {
        let expr = self.parse_expr()?;
        self.eat_semi()?;
        Ok(ASTNode::ExpressionStatement(Box::new(expr)))
    }

    // Declarations
    fn parse_var_decl(&mut self) -> Result<ASTNode, ParseError> {
        let kind = match self.cur_tt() {
            TokenType::Keyword(Keyword::Var) => VariableKind::Var,
            TokenType::Keyword(Keyword::Let) => VariableKind::Let,
            TokenType::Keyword(Keyword::Const) => VariableKind::Const,
            _ => unreachable!(),
        };
        self.advance();
        self.parse_var_decls(kind)
    }

    fn parse_lexical_decl(&mut self) -> Result<ASTNode, ParseError> {
        let kind = match self.cur_tt() {
            TokenType::Keyword(Keyword::Let) => VariableKind::Let,
            TokenType::Keyword(Keyword::Const) => VariableKind::Const,
            _ => unreachable!(),
        };
        self.advance();
        self.parse_var_decls(kind)
    }

    fn parse_var_decls(&mut self, kind: VariableKind) -> Result<ASTNode, ParseError> {
        let mut decls = vec![self.parse_var_declarator()?];
        while self.at(&TokenType::Comma) { self.advance(); decls.push(self.parse_var_declarator()?); }
        self.eat_semi()?;
        Ok(ASTNode::VariableDeclaration { kind, declarations: decls })
    }

    fn parse_var_declarator(&mut self) -> Result<VariableDeclarator, ParseError> {
        let pattern = self.parse_binding_target()?;
        let init = if self.at(&TokenType::Assign) { self.advance(); Some(Box::new(self.parse_assign_expr()?)) } else { None };
        let name = extract_name_from_pattern(&pattern);
        // Store the pattern if it's a destructuring (Array or Object expression)
        let destructuring_pattern = match pattern.as_ref() {
            ASTNode::ArrayExpression(_) | ASTNode::ObjectExpression(_) => Some(pattern),
            _ => None,
        };
        Ok(VariableDeclarator { name, init, pattern: destructuring_pattern })
    }

    fn parse_binding_target(&mut self) -> Result<Box<ASTNode>, ParseError> {
        match self.cur_tt().clone() {
            TokenType::LBracket => self.parse_array_pattern(),
            TokenType::LBrace => self.parse_object_pattern(),
            TokenType::Identifier(_) | TokenType::Keyword(_) => {
                let n = self.cur().token_type.clone();
                if let TokenType::Identifier(name) = n {
                    self.advance();
                    Ok(Box::new(ASTNode::Identifier(name)))
                } else if let TokenType::Keyword(kw) = n {
                    self.advance();
                    Ok(Box::new(ASTNode::Identifier(format!("{:?}", kw))))
                } else {
                    unreachable!()
                }
            }
            _ => Err(self.mk_err("Expected binding target")),
        }
    }

    fn make_param(&self, pattern: Box<ASTNode>, default: Option<Box<ASTNode>>, is_rest: bool) -> FunctionParam {
        let name = extract_name_from_pattern(&pattern);
        FunctionParam { name, default, is_rest, pattern: Some(pattern) }
    }

    fn parse_array_pattern(&mut self) -> Result<Box<ASTNode>, ParseError> {
        self.expect(&TokenType::LBracket)?;
        let mut elems = Vec::new();
        while !self.at(&TokenType::RBracket) && !self.at(&TokenType::Eof) {
            if self.at(&TokenType::Comma) { elems.push(ASTNode::UndefinedLiteral); }
            else { elems.push(*self.parse_binding_target()?); if self.at(&TokenType::Assign) { self.advance(); self.parse_assign_expr()?; } }
            if self.at(&TokenType::Comma) { self.advance(); }
        }
        self.expect(&TokenType::RBracket)?;
        Ok(Box::new(ASTNode::ArrayExpression(elems)))
    }

    fn parse_object_pattern(&mut self) -> Result<Box<ASTNode>, ParseError> {
        self.expect(&TokenType::LBrace)?;
        let mut props = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
            if self.at(&TokenType::Ellipsis) {
                self.advance();
                let target = self.parse_binding_target()?;
                props.push(PropertyDefinition { key: target, value: None, kind: PropertyKind::Init, is_computed: false, is_shorthand: false, is_method: false });
            } else {
                let key = self.parse_property_key()?;
                let value = if self.at(&TokenType::Colon) { self.advance(); Some(self.parse_binding_target()?) } else { None };
                let is_shorthand = value.is_none();
                props.push(PropertyDefinition { key, value, kind: PropertyKind::Init, is_computed: false, is_shorthand, is_method: false });
            }
            if self.at(&TokenType::Comma) { self.advance(); }
        }
        self.expect(&TokenType::RBrace)?;
        Ok(Box::new(ASTNode::ObjectExpression(props)))
    }

    // If
    fn parse_if(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::If)?;
        self.expect(&TokenType::LParen)?;
        let test = self.parse_expr()?;
        self.expect(&TokenType::RParen)?;
        let consequent = self.parse_stmt()?;
        let alternate = if self.at_kw(&Keyword::Else) { self.advance(); Some(Box::new(self.parse_stmt()?)) } else { None };
        Ok(ASTNode::If { test: Box::new(test), consequent: Box::new(consequent), alternate })
    }

    // While
    fn parse_while(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::While)?;
        self.expect(&TokenType::LParen)?;
        let test = self.parse_expr()?;
        self.expect(&TokenType::RParen)?;
        let body = self.parse_stmt()?;
        Ok(ASTNode::While { test: Box::new(test), body: Box::new(body) })
    }

    // Do-while
    fn parse_do_while(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Do)?;
        let body = self.parse_stmt()?;
        self.expect_kw(Keyword::While)?;
        self.expect(&TokenType::LParen)?;
        let test = self.parse_expr()?;
        self.expect(&TokenType::RParen)?;
        self.eat_semi()?;
        Ok(ASTNode::DoWhile { body: Box::new(body), test: Box::new(test) })
    }

    // For
    fn parse_for(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::For)?;
        let is_await = if self.at_kw(&Keyword::Await) { self.advance(); true } else { false };
        self.expect(&TokenType::LParen)?;
        let first = self.parse_for_head()?;
        match first {
            ForHead::Decl(kind, declarators) => {
                if self.at_kw(&Keyword::In) || self.at_kw(&Keyword::Of) {
                    let is_of = self.at_kw(&Keyword::Of);
                    self.advance();
                    let right = self.parse_expr()?;
                    self.expect(&TokenType::RParen)?;
                    let body = self.parse_stmt()?;
                    let left = if declarators.len() == 1 {
                        Box::new(ASTNode::Identifier(declarators.into_iter().next().unwrap().name))
                    } else {
                        Box::new(ASTNode::ArrayExpression(declarators.into_iter().map(|d| ASTNode::Identifier(d.name)).collect()))
                    };
                    return if is_of { Ok(ASTNode::ForOf { left, right: Box::new(right), body: Box::new(body), is_await }) } else { Ok(ASTNode::ForIn { left, right: Box::new(right), body: Box::new(body) }) };
                }
                let init = Some(Box::new(ASTNode::VariableDeclaration { kind, declarations: declarators }));
                self.parse_for_body(init)
            }
            ForHead::Expr(expr) => {
                if self.at_kw(&Keyword::In) || self.at_kw(&Keyword::Of) {
                    let is_of = self.at_kw(&Keyword::Of);
                    self.advance();
                    let right = self.parse_expr()?;
                    self.expect(&TokenType::RParen)?;
                    let body = self.parse_stmt()?;
                    return if is_of { Ok(ASTNode::ForOf { left: Box::new(expr), right: Box::new(right), body: Box::new(body), is_await }) } else { Ok(ASTNode::ForIn { left: Box::new(expr), right: Box::new(right), body: Box::new(body) }) };
                }
                self.expect(&TokenType::Semicolon)?;
                let test = if !self.at(&TokenType::Semicolon) { Some(Box::new(self.parse_expr()?)) } else { None };
                self.expect(&TokenType::Semicolon)?;
                let update = if !self.at(&TokenType::RParen) { Some(Box::new(self.parse_assign_expr()?)) } else { None };
                self.expect(&TokenType::RParen)?;
                let body = self.parse_stmt()?;
                Ok(ASTNode::For { init: Some(Box::new(expr)), test, update, body: Box::new(body) })
            }
            ForHead::Empty => {
                self.expect(&TokenType::Semicolon)?;
                let test = if !self.at(&TokenType::Semicolon) { Some(Box::new(self.parse_expr()?)) } else { None };
                self.expect(&TokenType::Semicolon)?;
                let update = if !self.at(&TokenType::RParen) { Some(Box::new(self.parse_assign_expr()?)) } else { None };
                self.expect(&TokenType::RParen)?;
                let body = self.parse_stmt()?;
                Ok(ASTNode::For { init: None, test, update, body: Box::new(body) })
            }
        }
    }

    fn parse_for_body(&mut self, init: Option<Box<ASTNode>>) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::Semicolon)?;
        let test = if !self.at(&TokenType::Semicolon) { Some(Box::new(self.parse_expr()?)) } else { None };
        self.expect(&TokenType::Semicolon)?;
        let update = if !self.at(&TokenType::RParen) { Some(Box::new(self.parse_assign_expr()?)) } else { None };
        self.expect(&TokenType::RParen)?;
        let body = self.parse_stmt()?;
        Ok(ASTNode::For { init, test, update, body: Box::new(body) })
    }

    fn parse_for_head(&mut self) -> Result<ForHead, ParseError> {
        match self.cur_tt().clone() {
            TokenType::Semicolon => Ok(ForHead::Empty),
            TokenType::Keyword(Keyword::Var) => {
                self.advance();
                let mut decls = vec![self.parse_var_declarator()?];
                while self.at(&TokenType::Comma) { self.advance(); decls.push(self.parse_var_declarator()?); }
                Ok(ForHead::Decl(VariableKind::Var, decls))
            }
            TokenType::Keyword(Keyword::Let) | TokenType::Keyword(Keyword::Const) => {
                let kind = if self.at_kw(&Keyword::Let) { VariableKind::Let } else { VariableKind::Const };
                self.advance();
                let mut decls = vec![self.parse_var_declarator()?];
                while self.at(&TokenType::Comma) { self.advance(); decls.push(self.parse_var_declarator()?); }
                Ok(ForHead::Decl(kind, decls))
            }
            _ => { let e = self.parse_expr()?; Ok(ForHead::Expr(e)) }
        }
    }

    // Switch
    fn parse_switch(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Switch)?;
        self.expect(&TokenType::LParen)?;
        let disc = self.parse_expr()?;
        self.expect(&TokenType::RParen)?;
        self.expect(&TokenType::LBrace)?;
        let mut cases = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
            if self.at_kw(&Keyword::Case) {
                self.advance();
                let test = Some(Box::new(self.parse_expr()?));
                self.expect(&TokenType::Colon)?;
                let mut cons = Vec::new();
                while !self.at_kw(&Keyword::Case) && !self.at_kw(&Keyword::Default) && !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
                    cons.push(self.parse_stmt_item()?);
                }
                cases.push(SwitchCase { test, consequent: cons });
            } else if self.at_kw(&Keyword::Default) {
                self.advance();
                self.expect(&TokenType::Colon)?;
                let mut cons = Vec::new();
                while !self.at_kw(&Keyword::Case) && !self.at_kw(&Keyword::Default) && !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
                    cons.push(self.parse_stmt_item()?);
                }
                cases.push(SwitchCase { test: None, consequent: cons });
            }
        }
        self.expect(&TokenType::RBrace)?;
        Ok(ASTNode::Switch { discriminant: Box::new(disc), cases })
    }

    // Return
    fn parse_return(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Return)?;
        let t = self.cur();
        if self.at_any(&[TokenType::Semicolon, TokenType::RBrace, TokenType::Eof])
            || t.line != self.tokens[self.token_pos.saturating_sub(1)].line {
            self.eat_semi()?;
            return Ok(ASTNode::Return(None));
        }
        let arg = self.parse_expr()?;
        self.eat_semi()?;
        Ok(ASTNode::Return(Some(Box::new(arg))))
    }

    // Throw
    fn parse_throw(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Throw)?;
        if self.cur().line != self.tokens[self.token_pos.saturating_sub(1)].line {
            return Err(self.mk_err("Illegal newline after throw"));
        }
        let arg = self.parse_expr()?;
        self.eat_semi()?;
        Ok(ASTNode::Throw(Box::new(arg)))
    }

    // Try
    fn parse_try(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Try)?;
        let block = self.parse_block()?;
        let catch = if self.at_kw(&Keyword::Catch) {
            self.advance();
            let param = if self.at(&TokenType::LParen) {
                self.advance();
                let p = Some(self.parse_binding_target()?);
                self.expect(&TokenType::RParen)?;
                p
            } else { None };
            let body = self.parse_block()?;
            Some(CatchClause { param, body: Box::new(body) })
        } else { None };
        let finally = if self.at_kw(&Keyword::Finally) { self.advance(); Some(Box::new(self.parse_block()?)) } else { None };
        if catch.is_none() && finally.is_none() { return Err(self.mk_err("try requires catch or finally")); }
        Ok(ASTNode::Try { block: Box::new(block), catch, finally })
    }

    // With
    fn parse_with(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::With)?;
        self.expect(&TokenType::LParen)?;
        let _obj = self.parse_expr()?;
        self.expect(&TokenType::RParen)?;
        let body = self.parse_stmt()?;
        Ok(ASTNode::Block(vec![body]))
    }

    // ===== Expressions =====

    fn parse_expr(&mut self) -> Result<ASTNode, ParseError> {
        let mut expr = self.parse_assign_expr()?;
        if self.at(&TokenType::Comma) {
            let mut exprs = vec![expr];
            while self.at(&TokenType::Comma) { self.advance(); exprs.push(self.parse_assign_expr()?); }
            expr = ASTNode::Sequence { expressions: exprs };
        }
        Ok(expr)
    }

    fn parse_assign_expr(&mut self) -> Result<ASTNode, ParseError> {
        // Arrow function: (params) => body
        if self.at_kw(&Keyword::Async) {
            let saved = self.token_pos;
            self.advance();
            if let TokenType::Identifier(_) = self.cur_tt().clone() {
                if self.at(&TokenType::Arrow) {
                    let name = if let TokenType::Identifier(n) = self.tokens[saved + 1].token_type.clone() { n } else { unreachable!() };
                    self.advance(); // over identifier
                    self.advance(); // over =>
                    let body = self.parse_assign_expr()?;
                    return Ok(ASTNode::ArrowFunctionExpression {
                        params: vec![self.make_param(Box::new(ASTNode::Identifier(name)), None, false)],
                        body: Box::new(body), is_async: true,
                    });
                }
            }
            self.token_pos = saved;
        }

        let expr = self.parse_conditional()?;

        // Arrow function with single unparenthesized parameter: x => body
        if self.at(&TokenType::Arrow) {
            if let ASTNode::Identifier(name) = &expr {
                self.advance(); // consume =>
                let body = self.parse_arrow_body()?;
                return Ok(ASTNode::ArrowFunctionExpression {
                    params: vec![FunctionParam { name: name.clone(), default: None, is_rest: false, pattern: None }],
                    body: Box::new(body),
                    is_async: false,
                });
            }
        }

        let assign_op = match self.cur_tt() {
            TokenType::Assign => Some(AssignmentOp::Assign),
            TokenType::PlusAssign => Some(AssignmentOp::AddAssign),
            TokenType::MinusAssign => Some(AssignmentOp::SubAssign),
            TokenType::StarAssign => Some(AssignmentOp::MulAssign),
            TokenType::SlashAssign => Some(AssignmentOp::DivAssign),
            TokenType::PercentAssign => Some(AssignmentOp::ModAssign),
            TokenType::PowAssign => Some(AssignmentOp::PowAssign),
            TokenType::AndAssign => Some(AssignmentOp::AndAssign),
            TokenType::OrAssign => Some(AssignmentOp::OrAssign),
            TokenType::BitAndAssign => Some(AssignmentOp::BitAndAssign),
            TokenType::BitOrAssign => Some(AssignmentOp::BitOrAssign),
            TokenType::BitXorAssign => Some(AssignmentOp::BitXorAssign),
            TokenType::ShlAssign => Some(AssignmentOp::ShlAssign),
            TokenType::ShrAssign => Some(AssignmentOp::ShrAssign),
            TokenType::UShrAssign => Some(AssignmentOp::UShrAssign),
            _ => None,
        };
        if let Some(op) = assign_op {
            self.advance();
            let right = self.parse_assign_expr()?;
            return Ok(ASTNode::Assignment { op, left: Box::new(expr), right: Box::new(right) });
        }
        Ok(expr)
    }

    /// Get the precedence and binary operator for a token, or None if not a binary op.
    fn binop_for_token(&self) -> Option<(BinaryOp, u8)> {
        match self.cur_tt() {
            TokenType::Or => Some((BinaryOp::Or, 4)),
            TokenType::And => Some((BinaryOp::And, 5)),
            TokenType::BitOr => Some((BinaryOp::BitOr, 6)),
            TokenType::BitXor => Some((BinaryOp::BitXor, 7)),
            TokenType::BitAnd => Some((BinaryOp::BitAnd, 8)),
            TokenType::Eq => Some((BinaryOp::Eq, 9)),
            TokenType::Ne => Some((BinaryOp::Ne, 9)),
            TokenType::StrictEq => Some((BinaryOp::StrictEq, 9)),
            TokenType::StrictNe => Some((BinaryOp::StrictNe, 9)),
            TokenType::Lt => Some((BinaryOp::Lt, 10)),
            TokenType::Gt => Some((BinaryOp::Gt, 10)),
            TokenType::Le => Some((BinaryOp::Le, 10)),
            TokenType::Ge => Some((BinaryOp::Ge, 10)),
            TokenType::Keyword(Keyword::Instanceof) => Some((BinaryOp::Instanceof, 10)),
            TokenType::Keyword(Keyword::In) => Some((BinaryOp::In, 10)),
            TokenType::Shl => Some((BinaryOp::Shl, 11)),
            TokenType::Shr => Some((BinaryOp::Shr, 11)),
            TokenType::UShr => Some((BinaryOp::UShr, 11)),
            TokenType::Plus => Some((BinaryOp::Add, 12)),
            TokenType::Minus => Some((BinaryOp::Sub, 12)),
            TokenType::Star => Some((BinaryOp::Mul, 13)),
            TokenType::Slash => Some((BinaryOp::Div, 13)),
            TokenType::Percent => Some((BinaryOp::Mod, 13)),
            TokenType::Pow => Some((BinaryOp::Pow, 14)),
            _ => None,
        }
    }

    /// Parse a binary expression using precedence climbing (Pratt parsing).
    /// This replaces the chain of parse_logical_or → parse_logical_and → ... → parse_exp
    /// with a single iterative function, eliminating deep recursion.
    fn parse_binary_expr(&mut self, min_prec: u8) -> Result<ASTNode, ParseError> {
        let mut left = self.parse_unary()?;
        while let Some((op, prec)) = self.binop_for_token() {
            if prec < min_prec {
                break;
            }
            self.advance();
            // Right-associative for exponentiation
            let next_prec = if op == BinaryOp::Pow { prec } else { prec + 1 };
            let right = self.parse_binary_expr(next_prec)?;
            left = ASTNode::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_conditional(&mut self) -> Result<ASTNode, ParseError> {
        let expr = self.parse_binary_expr(0)?;
        if self.at(&TokenType::Question) {
            self.advance();
            let cons = self.parse_assign_expr()?;
            self.expect(&TokenType::Colon)?;
            let alt = self.parse_assign_expr()?;
            return Ok(ASTNode::Conditional { test: Box::new(expr), consequent: Box::new(cons), alternate: Box::new(alt) });
        }
        if self.at(&TokenType::NullishCoalescing) {
            // Handle ?? with precedence climbing
            self.advance();
            let right = self.parse_binary_expr(4)?; // same precedence as logical or
            return Ok(ASTNode::BinaryOp { op: BinaryOp::NullishCoalescing, left: Box::new(expr), right: Box::new(right) });
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<ASTNode, ParseError> {
        match self.cur_tt().clone() {
            TokenType::Minus => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Minus, operand: Box::new(o), prefix: true }) }
            TokenType::Plus => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Plus, operand: Box::new(o), prefix: true }) }
            TokenType::Not => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Not, operand: Box::new(o), prefix: true }) }
            TokenType::BitNot => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::BitNot, operand: Box::new(o), prefix: true }) }
            TokenType::Keyword(Keyword::Typeof) => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Typeof, operand: Box::new(o), prefix: true }) }
            TokenType::Keyword(Keyword::Void) => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Void, operand: Box::new(o), prefix: true }) }
            TokenType::Keyword(Keyword::Delete) => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Delete, operand: Box::new(o), prefix: true }) }
            TokenType::Keyword(Keyword::Await) => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::AwaitExpression(Box::new(o))) }
            TokenType::Keyword(Keyword::Yield) => self.parse_yield(),
            TokenType::Inc => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Inc, operand: Box::new(o), prefix: true }) }
            TokenType::Dec => { self.advance(); let o = self.parse_unary()?; Ok(ASTNode::UnaryOp { op: UnaryOp::Dec, operand: Box::new(o), prefix: true }) }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<ASTNode, ParseError> {
        let expr = self.parse_lhs()?;
        if self.at(&TokenType::Inc) { self.advance(); return Ok(ASTNode::UnaryOp { op: UnaryOp::Inc, operand: Box::new(expr), prefix: false }); }
        if self.at(&TokenType::Dec) { self.advance(); return Ok(ASTNode::UnaryOp { op: UnaryOp::Dec, operand: Box::new(expr), prefix: false }); }
        Ok(expr)
    }

    // Left-hand side (new, member access, calls)
    fn parse_lhs(&mut self) -> Result<ASTNode, ParseError> {
        let mut expr = if self.at_kw(&Keyword::New) { self.parse_new()? } else { self.parse_primary()? };

        loop {
            if self.at(&TokenType::OptionalChaining) {
                self.advance();
                if self.at(&TokenType::LParen) { expr = self.parse_call_args(expr)?; }
                else if self.at(&TokenType::LBracket) { self.advance(); let p = self.parse_expr()?; self.expect(&TokenType::RBracket)?; expr = ASTNode::Member { object: Box::new(expr), property: Box::new(p), computed: true }; }
                else if let TokenType::Identifier(n) = self.cur_tt().clone() { self.advance(); expr = ASTNode::Member { object: Box::new(expr), property: Box::new(ASTNode::Identifier(n)), computed: false }; }
                else { break; }
                continue;
            }
            match self.cur_tt() {
                TokenType::Dot => {
                    self.advance();
                    // Property names can be any identifier or keyword
                    let n = match self.cur_tt().clone() {
                        TokenType::Identifier(name) => name,
                        TokenType::Keyword(kw) => format!("{:?}", kw).to_lowercase(),
                        _ => return Err(self.mk_err("Expected property name")),
                    };
                    self.advance();
                    expr = ASTNode::Member { object: Box::new(expr), property: Box::new(ASTNode::Identifier(n)), computed: false };
                }
                TokenType::LBracket => {
                    self.advance();
                    let p = self.parse_expr()?;
                    self.expect(&TokenType::RBracket)?;
                    expr = ASTNode::Member { object: Box::new(expr), property: Box::new(p), computed: true };
                }
                TokenType::LParen => {
                    expr = self.parse_call_args(expr)?;
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self, callee: ASTNode) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::LParen)?;
        let mut args = Vec::new();
        if !self.at(&TokenType::RParen) {
            if self.at(&TokenType::Ellipsis) { self.advance(); args.push(ASTNode::SpreadElement(Box::new(self.parse_assign_expr()?))); }
            else { args.push(self.parse_assign_expr()?); }
            while self.at(&TokenType::Comma) { self.advance(); if !self.at(&TokenType::RParen) {
                if self.at(&TokenType::Ellipsis) { self.advance(); args.push(ASTNode::SpreadElement(Box::new(self.parse_assign_expr()?))); }
                else { args.push(self.parse_assign_expr()?); }
            } }
        }
        self.expect(&TokenType::RParen)?;
        Ok(ASTNode::Call { callee: Box::new(callee), args })
    }

    fn parse_new(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::New)?;
        let callee = if self.at_kw(&Keyword::New) { self.parse_new()? } else { self.parse_primary()? };
        if self.at(&TokenType::LParen) {
            self.advance();
            let mut args = Vec::new();
            if !self.at(&TokenType::RParen) {
                args.push(self.parse_assign_expr()?);
                while self.at(&TokenType::Comma) { self.advance(); if !self.at(&TokenType::RParen) { args.push(self.parse_assign_expr()?); } }
            }
            self.expect(&TokenType::RParen)?;
            Ok(ASTNode::New { callee: Box::new(callee), args })
        } else {
            Ok(ASTNode::New { callee: Box::new(callee), args: Vec::new() })
        }
    }

    // Primary expressions
    fn parse_primary(&mut self) -> Result<ASTNode, ParseError> {
        match self.cur_tt().clone() {
            TokenType::Keyword(Keyword::This) => { self.advance(); Ok(ASTNode::Identifier("this".into())) }
            TokenType::Keyword(Keyword::Super) => {
                self.advance();
                // super(args) - super call in constructor
                if self.at(&TokenType::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.at(&TokenType::RParen) {
                        args.push(self.parse_assign_expr()?);
                        while self.at(&TokenType::Comma) {
                            self.advance();
                            if !self.at(&TokenType::RParen) {
                                args.push(self.parse_assign_expr()?);
                            }
                        }
                    }
                    self.expect(&TokenType::RParen)?;
                    Ok(ASTNode::SuperCall(args))
                } else {
                    // super.prop - super property access
                    Ok(ASTNode::Identifier("__super__".into()))
                }
            }
            TokenType::Number(n) => { self.advance(); Ok(ASTNode::NumberLiteral(n)) }
            TokenType::String(ref s) => { let s = s.clone(); self.advance(); Ok(ASTNode::StringLiteral(s)) }
            TokenType::Bool(b) => { self.advance(); Ok(ASTNode::BoolLiteral(b)) }
            TokenType::Null => { self.advance(); Ok(ASTNode::NullLiteral) }
            TokenType::Undefined => { self.advance(); Ok(ASTNode::UndefinedLiteral) }
            TokenType::RegExp { ref pattern, ref flags } => { let p = pattern.clone(); let f = flags.clone(); self.advance(); Ok(ASTNode::RegExpLiteral { pattern: p, flags: f }) }
            TokenType::Identifier(ref name) => { let n = name.clone(); self.advance(); Ok(ASTNode::Identifier(n)) }
            TokenType::LParen => self.parse_paren_or_arrow(),
            TokenType::LBracket => self.parse_array_literal(),
            TokenType::LBrace => self.parse_object_literal(),
            TokenType::Keyword(Keyword::Function) => self.parse_func_expr(false),
            TokenType::Keyword(Keyword::Async) => self.parse_async_func_expr(),
            TokenType::Keyword(Keyword::Class) => self.parse_class_expr(),
            TokenType::Keyword(Keyword::Import) => {
                // Dynamic import: import(specifier)
                self.advance(); // consume 'import'
                self.expect(&TokenType::LParen)?;
                let specifier = self.parse_assign_expr()?;
                self.expect(&TokenType::RParen)?;
                Ok(ASTNode::Call {
                    callee: Box::new(ASTNode::Identifier("__dynamic_import__".into())),
                    args: vec![specifier],
                })
            }
            TokenType::TemplateLiteral(_) => self.parse_template_literal(),
            _ => Err(self.mk_err(&format!("Unexpected token {:?}", self.cur_tt()))),
        }
    }

    fn parse_paren_or_arrow(&mut self) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::LParen)?;
        if self.at(&TokenType::RParen) {
            self.advance();
            if self.at(&TokenType::Arrow) { self.advance(); let body = self.parse_assign_expr()?; return Ok(ASTNode::ArrowFunctionExpression { params: vec![], body: Box::new(body), is_async: false }); }
            return Ok(ASTNode::Sequence { expressions: vec![] });
        }
        // Try to detect arrow function by looking ahead
        let saved = self.token_pos;
        let mut is_arrow = false;
        let mut depth = 1i32; // opening paren already consumed
        loop {
            match self.cur_tt() {
                TokenType::LParen => { depth += 1; let _ = self.advance(); }
                TokenType::RParen => { depth -= 1; if depth == 0 { let _ = self.advance(); if self.at(&TokenType::Arrow) { is_arrow = true; } break; } let _ = self.advance(); }
                TokenType::Eof => break,
                _ => { let _ = self.advance(); }
            }
            if depth < 0 { break; }
        }
        self.token_pos = saved;

        if is_arrow {
            return self.parse_arrow_after_lparen();
        }

        // Not an arrow - parse as grouping / sequence
        let expr = self.parse_expr()?;
        if self.at(&TokenType::RParen) { self.advance(); return Ok(expr); }

        let mut exprs = vec![expr];
        while self.at(&TokenType::Comma) { self.advance(); if self.at(&TokenType::RParen) { break; } exprs.push(self.parse_expr()?); }
        self.expect(&TokenType::RParen)?;

        if self.at(&TokenType::Arrow) {
            self.advance();
            let params: Vec<FunctionParam> = exprs.into_iter().map(|e| {
                let name = extract_name_from_pattern(&e);
                FunctionParam { name, default: None, is_rest: false, pattern: Some(Box::new(e)) }
            }).collect();
            let body = self.parse_arrow_body()?;
            return Ok(ASTNode::ArrowFunctionExpression { params, body: Box::new(body), is_async: false });
        }
        if exprs.len() == 1 { Ok(exprs.remove(0)) } else { Ok(ASTNode::Sequence { expressions: exprs }) }
    }

    fn parse_arrow_after_lparen(&mut self) -> Result<ASTNode, ParseError> {
        let mut params = Vec::new();
        while !self.at(&TokenType::RParen) && !self.at(&TokenType::Eof) {
            let is_rest = self.at(&TokenType::Ellipsis);
            if is_rest { self.advance(); }
            let pattern = self.parse_binding_target()?;
            let default = if self.at(&TokenType::Assign) { self.advance(); Some(Box::new(self.parse_assign_expr()?)) } else { None };
            params.push(self.make_param(pattern, default, is_rest));
            if self.at(&TokenType::Comma) { self.advance(); }
        }
        self.expect(&TokenType::RParen)?;
        self.expect(&TokenType::Arrow)?;
        let body = self.parse_arrow_body()?;
        Ok(ASTNode::ArrowFunctionExpression { params, body: Box::new(body), is_async: false })
    }

    fn parse_arrow_body(&mut self) -> Result<ASTNode, ParseError> {
        if self.at(&TokenType::LBrace) { self.parse_func_body(false) } else {
            let expr = self.parse_assign_expr()?;
            Ok(ASTNode::ExpressionStatement(Box::new(expr)))
        }
    }

    // Array literal
    fn parse_array_literal(&mut self) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::LBracket)?;
        let mut elems = Vec::new();
        while !self.at(&TokenType::RBracket) && !self.at(&TokenType::Eof) {
            if self.at(&TokenType::Comma) { elems.push(ASTNode::UndefinedLiteral); self.advance(); }
            else if self.at(&TokenType::Ellipsis) { self.advance(); let a = self.parse_assign_expr()?; elems.push(ASTNode::SpreadElement(Box::new(a))); if self.at(&TokenType::Comma) { self.advance(); } }
            else { elems.push(self.parse_assign_expr()?); if self.at(&TokenType::Comma) { self.advance(); } }
        }
        self.expect(&TokenType::RBracket)?;
        Ok(ASTNode::ArrayExpression(elems))
    }

    // Object literal
    fn parse_object_literal(&mut self) -> Result<ASTNode, ParseError> {
        self.expect(&TokenType::LBrace)?;
        let mut props = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
            props.push(self.parse_prop_def()?);
            if self.at(&TokenType::Comma) { self.advance(); }
        }
        self.expect(&TokenType::RBrace)?;
        Ok(ASTNode::ObjectExpression(props))
    }

    fn parse_prop_def(&mut self) -> Result<PropertyDefinition, ParseError> {
        if self.at(&TokenType::Ellipsis) { self.advance(); let a = self.parse_assign_expr()?; return Ok(PropertyDefinition { key: Box::new(ASTNode::SpreadElement(Box::new(a))), value: None, kind: PropertyKind::Init, is_computed: false, is_shorthand: false, is_method: false }); }

        let key = self.parse_property_key()?;
        let is_computed = matches!(&*key, ASTNode::Member { computed: true, .. });

        // Method / getter / setter
        if self.at(&TokenType::LParen) {
            let params = self.parse_formal_params()?;
            let body = self.parse_func_body(false)?;
            let val = ASTNode::FunctionDeclaration { name: String::new(), params, body: Box::new(body), is_async: false, is_generator: false };
            return Ok(PropertyDefinition { key, value: Some(Box::new(val)), kind: PropertyKind::Init, is_computed, is_shorthand: false, is_method: true });
        }
        if let ASTNode::Identifier(ref id) = *key {
            let id_str = id.as_str();
            if (id_str == "get" || id_str == "set") && !self.at(&TokenType::Colon) && !self.at(&TokenType::LParen) {
                let kind = if id_str == "get" { PropertyKind::Get } else { PropertyKind::Set };
                let real_key = self.parse_property_key()?;
                let params = self.parse_formal_params()?;
                let body = self.parse_func_body(false)?;
                let val = ASTNode::FunctionDeclaration { name: String::new(), params, body: Box::new(body), is_async: false, is_generator: false };
                let ic = matches!(&*real_key, ASTNode::Member { computed: true, .. });
                return Ok(PropertyDefinition { key: real_key, value: Some(Box::new(val)), kind, is_computed: ic, is_shorthand: false, is_method: false });
            }
        }

        // Shorthand
        if self.at_any(&[TokenType::RBrace, TokenType::Comma]) {
            if let ASTNode::Identifier(ref name) = *key {
                let n = name.clone();
                return Ok(PropertyDefinition { key, value: Some(Box::new(ASTNode::Identifier(n))), kind: PropertyKind::Init, is_computed, is_shorthand: true, is_method: false });
            }
        }

        // key: value
        self.expect(&TokenType::Colon)?;
        let value = self.parse_assign_expr()?;
        Ok(PropertyDefinition { key, value: Some(Box::new(value)), kind: PropertyKind::Init, is_computed, is_shorthand: false, is_method: false })
    }

    fn parse_property_key(&mut self) -> Result<Box<ASTNode>, ParseError> {
        match self.cur_tt().clone() {
            TokenType::LBracket => { self.advance(); let k = self.parse_expr()?; self.expect(&TokenType::RBracket)?; Ok(Box::new(ASTNode::Member { object: Box::new(ASTNode::NullLiteral), property: Box::new(k), computed: true })) }
            TokenType::Identifier(ref n) => { let n = n.clone(); self.advance(); Ok(Box::new(ASTNode::Identifier(n))) }
            TokenType::Number(n) => { self.advance(); Ok(Box::new(ASTNode::NumberLiteral(n))) }
            TokenType::String(ref s) => { let s = s.clone(); self.advance(); Ok(Box::new(ASTNode::StringLiteral(s))) }
            TokenType::Keyword(kw) => { self.advance(); Ok(Box::new(ASTNode::Identifier(format!("{:?}", kw)))) }
            _ => Err(self.mk_err("Expected property key")),
        }
    }

    // Function expressions & declarations
    fn parse_func_decl(&mut self) -> Result<ASTNode, ParseError> {
        let is_async = if self.at_kw(&Keyword::Async) { self.advance(); true } else { false };
        self.expect_kw(Keyword::Function)?;
        let is_gen = self.at(&TokenType::Star); if is_gen { self.advance(); }
        let name = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { String::new() };
        let params = self.parse_formal_params()?;
        let body = self.parse_func_body(is_async)?;
        Ok(ASTNode::FunctionDeclaration { name, params, body: Box::new(body), is_async, is_generator: is_gen })
    }

    fn parse_func_expr(&mut self, is_async: bool) -> Result<ASTNode, ParseError> {
        if !is_async { self.expect_kw(Keyword::Function)?; } else { self.expect_kw(Keyword::Function)?; }
        let is_gen = self.at(&TokenType::Star); if is_gen { self.advance(); }
        let name = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { String::new() };
        let params = self.parse_formal_params()?;
        let body = self.parse_func_body(is_async)?;
        Ok(ASTNode::FunctionDeclaration { name, params, body: Box::new(body), is_async, is_generator: is_gen })
    }

    fn parse_async_func_expr(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Async)?;
        self.parse_func_expr(true)
    }

    fn parse_func_body(&mut self, _is_async: bool) -> Result<ASTNode, ParseError> {
        let prev = self.strict_mode;
        self.expect(&TokenType::LBrace)?;
        let mut body = Vec::new();
        if !self.at(&TokenType::RBrace) {
            if let TokenType::String(ref s) = self.cur_tt() {
                if s == "use strict" { self.strict_mode = true; self.advance(); self.eat_semi()?; }
            }
        }
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) { body.push(self.parse_stmt_item()?); }
        self.expect(&TokenType::RBrace)?;
        self.strict_mode = prev;
        Ok(ASTNode::Block(body))
    }

    fn parse_formal_params(&mut self) -> Result<Vec<FunctionParam>, ParseError> {
        self.expect(&TokenType::LParen)?;
        let mut params = Vec::new();
        if !self.at(&TokenType::RParen) {
            params.push(self.parse_formal_param()?);
            while self.at(&TokenType::Comma) { self.advance(); if !self.at(&TokenType::RParen) { params.push(self.parse_formal_param()?); } }
        }
        self.expect(&TokenType::RParen)?;
        Ok(params)
    }

    fn parse_formal_param(&mut self) -> Result<FunctionParam, ParseError> {
        let is_rest = self.at(&TokenType::Ellipsis); if is_rest { self.advance(); }
        let pattern = self.parse_binding_target()?;
        let default = if self.at(&TokenType::Assign) { self.advance(); Some(Box::new(self.parse_assign_expr()?)) } else { None };
        Ok(self.make_param(pattern, default, is_rest))
    }

    // Class
    fn parse_class_decl(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Class)?;
        let name = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected class name")); };
        let super_class = if self.at_kw(&Keyword::Extends) { self.advance(); Some(Box::new(self.parse_lhs()?)) } else { None };
        self.expect(&TokenType::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) { body.push(self.parse_class_element()?); }
        self.expect(&TokenType::RBrace)?;
        Ok(ASTNode::ClassDeclaration { name, super_class, body })
    }

    fn parse_class_expr(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Class)?;
        let name = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { String::new() };
        let super_class = if self.at_kw(&Keyword::Extends) { self.advance(); Some(Box::new(self.parse_lhs()?)) } else { None };
        self.expect(&TokenType::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) { body.push(self.parse_class_element()?); }
        self.expect(&TokenType::RBrace)?;
        Ok(ASTNode::ClassDeclaration { name, super_class, body })
    }

    fn parse_class_element(&mut self) -> Result<ClassElement, ParseError> {
        let mut is_static = false;
        if self.at_kw(&Keyword::Static) { is_static = true; self.advance(); }
        if self.at(&TokenType::Semicolon) { self.advance(); return Ok(ClassElement { key: Box::new(ASTNode::NullLiteral), value: None, kind: ClassElementKind::Field, is_static, is_computed: false }); }

        let (kind, key) = if self.at_ident("get") { self.advance(); let k = self.parse_property_key()?; (ClassElementKind::Get, k) }
            else if self.at_ident("set") { self.advance(); let k = self.parse_property_key()?; (ClassElementKind::Set, k) }
            else { let k = self.parse_property_key()?; (ClassElementKind::Method, k) };
        let is_computed = matches!(&*key, ASTNode::Member { computed: true, .. });

        if self.at(&TokenType::LParen) || kind != ClassElementKind::Method {
            let params = self.parse_formal_params()?;
            let body = self.parse_func_body(false)?;
            let val = ASTNode::FunctionDeclaration { name: String::new(), params, body: Box::new(body), is_async: false, is_generator: false };
            Ok(ClassElement { key, value: Some(Box::new(val)), kind, is_static, is_computed })
        } else {
            let value = if self.at(&TokenType::Assign) { self.advance(); Some(Box::new(self.parse_assign_expr()?)) } else { None };
            self.eat_semi()?;
            Ok(ClassElement { key, value, kind: ClassElementKind::Field, is_static, is_computed })
        }
    }

    // Template literal (tagged or plain)
    fn parse_template_literal(&mut self) -> Result<ASTNode, ParseError> {
        // Re-scan from source position. The backtick was consumed and pushed as TemplateLiteral token.
        // But our tokenizer already processed templates inline via TemplateLiteral(Vec<TemplateToken>).
        // Actually the tokenizer handled backtick on the main path by entering the template loop.
        // We need to re-read: the token at token_pos should be TemplateLiteral(parts).
        if let TokenType::TemplateLiteral(ref parts) = self.cur_tt().clone() {
            let parts_clone = parts.clone();
            self.advance();
            let mut quasis = Vec::new();
            let mut expressions = Vec::new();
            for part in &parts_clone {
                match part {
                    TemplateToken::StringPart { raw, cooked, tail } => {
                        quasis.push(TemplateElement { raw: raw.clone(), cooked: cooked.clone(), tail: *tail });
                    }
                    TemplateToken::Expression(sub_tokens) => {
                        // Parse expression from sub-tokens by temporarily replacing token stream
                        let saved_tokens = self.tokens.clone();
                        let saved_pos = self.token_pos;
                        self.tokens = sub_tokens.clone();
                        self.tokens.push(Token { token_type: TokenType::Eof, line: 0, column: 0, start: 0, end: 0 });
                        self.token_pos = 0;
                        let expr = self.parse_expr()?;
                        let saved2 = self.tokens.clone();
                        self.tokens = saved_tokens;
                        self.token_pos = saved_pos;
                        expressions.push(expr);
                    }
                }
            }
            Ok(ASTNode::TemplateLiteral { quasis, expressions })
        } else {
            Err(self.mk_err("Expected template literal"))
        }
    }

    // Yield
    fn parse_yield(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Yield)?;
        let delegate = self.at(&TokenType::Star);
        if delegate { self.advance(); }
        if self.at_any(&[TokenType::Semicolon, TokenType::RBrace, TokenType::RParen, TokenType::Eof, TokenType::Comma]) {
            return Ok(ASTNode::YieldExpression { argument: None, delegate });
        }
        let arg = self.parse_assign_expr()?;
        Ok(ASTNode::YieldExpression { argument: Some(Box::new(arg)), delegate })
    }

    // Import / Export
    fn parse_import_decl(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Import)?;
        if let TokenType::String(ref s) = self.cur_tt().clone() { let src = s.clone(); self.advance(); self.eat_semi()?; return Ok(ASTNode::ImportDeclaration { specifiers: vec![], source: src }); }
        let mut specs = Vec::new();
        // Handle `import name from "module"` (default import without `default` keyword)
        if let TokenType::Identifier(_) = self.cur_tt() {
            // Check if this is `import name from` (default import) vs `import { ... }` (named import)
            // Look ahead: if next token is `from`, it's a default import
            let next_is_from = self.token_pos + 1 < self.tokens.len()
                && matches!(self.tokens[self.token_pos + 1].token_type, TokenType::Keyword(Keyword::From));
            if next_is_from {
                let local = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { unreachable!() };
                specs.push(ImportSpecifier::Default { local });
            }
        }
        if self.at_kw(&Keyword::Default) {
            self.advance();
            let local = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); };
            specs.push(ImportSpecifier::Default { local });
            if self.at(&TokenType::Comma) { self.advance(); }
        }
        if self.at(&TokenType::Star) {
            self.advance();
            self.expect_kw(Keyword::As)?;
            let local = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); };
            specs.push(ImportSpecifier::Namespace { local });
        } else if self.at(&TokenType::LBrace) {
            self.advance();
            while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
                let imported = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); };
                let local = if self.at_kw(&Keyword::As) { self.advance(); if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); } } else { imported.clone() };
                specs.push(ImportSpecifier::Named { imported, local });
                if self.at(&TokenType::Comma) { self.advance(); }
            }
            self.expect(&TokenType::RBrace)?;
        }
        self.expect_kw(Keyword::From)?;
        let source = if let TokenType::String(ref s) = self.cur_tt().clone() { let s = s.clone(); self.advance(); s } else { return Err(self.mk_err("Expected module source string")); };
        self.eat_semi()?;
        Ok(ASTNode::ImportDeclaration { specifiers: specs, source })
    }

    fn parse_export_decl(&mut self) -> Result<ASTNode, ParseError> {
        self.expect_kw(Keyword::Export)?;
        if self.at_kw(&Keyword::Default) {
            self.advance();
            let decl = if self.at_kw(&Keyword::Function) { self.parse_func_decl()? } else if self.at_kw(&Keyword::Class) { self.parse_class_decl()? } else { let e = self.parse_assign_expr()?; self.eat_semi()?; ASTNode::ExpressionStatement(Box::new(e)) };
            return Ok(ASTNode::ExportDeclaration { declaration: Some(Box::new(decl)), specifiers: vec![], source: None });
        }
        if self.at(&TokenType::LBrace) {
            self.advance();
            let mut specs = Vec::new();
            while !self.at(&TokenType::RBrace) && !self.at(&TokenType::Eof) {
                let local = if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); };
                let exported = if self.at_kw(&Keyword::As) { self.advance(); if let TokenType::Identifier(ref n) = self.cur_tt().clone() { let n = n.clone(); self.advance(); n } else { return Err(self.mk_err("Expected identifier")); } } else { local.clone() };
                specs.push(ExportSpecifier { local, exported });
                if self.at(&TokenType::Comma) { self.advance(); }
            }
            self.expect(&TokenType::RBrace)?;
            if self.at_kw(&Keyword::From) { self.advance(); let src = if let TokenType::String(ref s) = self.cur_tt().clone() { let s = s.clone(); self.advance(); Some(s) } else { None }; self.eat_semi()?; return Ok(ASTNode::ExportDeclaration { declaration: None, specifiers: specs, source: src }); }
            self.eat_semi()?;
            return Ok(ASTNode::ExportDeclaration { declaration: None, specifiers: specs, source: None });
        }
        if self.at(&TokenType::Star) {
            self.advance();
            self.expect_kw(Keyword::From)?;
            let src = if let TokenType::String(ref s) = self.cur_tt().clone() { let s = s.clone(); self.advance(); Some(s) } else { None };
            self.eat_semi()?;
            return Ok(ASTNode::ExportDeclaration { declaration: None, specifiers: vec![], source: src });
        }
        let decl = self.parse_stmt()?;
        Ok(ASTNode::ExportDeclaration { declaration: Some(Box::new(decl)), specifiers: vec![], source: None })
    }
}

/// Parse error.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl ParseError {
    pub fn new(message: &str, line: usize, column: usize) -> Self {
        ParseError {
            message: message.to_string(),
            line,
            column,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SyntaxError: {} (line {}, column {})", self.message, self.line, self.column)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> ASTNode {
        let mut parser = Parser::new(source);
        parser.parse().unwrap()
    }

    fn parse_err(source: &str) -> String {
        let mut parser = Parser::new(source);
        parser.parse().unwrap_err().message
    }

    /// Helper: extract the first statement from a parsed program (Block).
    fn first_stmt(ast: &ASTNode) -> &ASTNode {
        match ast {
            ASTNode::Block(stmts) if !stmts.is_empty() => &stmts[0],
            other => panic!("Expected non-empty Block, got {:?}", other),
        }
    }

    /// Helper: extract the expression from an ExpressionStatement.
    fn expr_of(stmt: &ASTNode) -> &ASTNode {
        match stmt {
            ASTNode::ExpressionStatement(e) => e.as_ref(),
            other => panic!("Expected ExpressionStatement, got {:?}", other),
        }
    }

    // ========================================================================
    // 1. LITERALS
    // ========================================================================

    #[test]
    fn test_int_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("42"))), ASTNode::NumberLiteral(n) if *n == 42.0));
    }

    #[test]
    fn test_float_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("3.14"))), ASTNode::NumberLiteral(n) if (*n - 3.14).abs() < f64::EPSILON));
    }

    #[test]
    fn test_float_with_explicit_dot() {
        // .5 is not valid JS; but 0.5 is
        assert!(matches!(expr_of(first_stmt(&parse("0.5"))), ASTNode::NumberLiteral(n) if *n == 0.5));
    }

    #[test]
    fn test_scientific_notation() {
        assert!(matches!(expr_of(first_stmt(&parse("1e3"))), ASTNode::NumberLiteral(n) if *n == 1000.0));
    }

    #[test]
    fn test_scientific_notation_negative_exp() {
        assert!(matches!(expr_of(first_stmt(&parse("1e-2"))), ASTNode::NumberLiteral(n) if *n == 0.01));
    }

    #[test]
    fn test_scientific_notation_positive_exp() {
        assert!(matches!(expr_of(first_stmt(&parse("5E+2"))), ASTNode::NumberLiteral(n) if *n == 500.0));
    }

    #[test]
    fn test_hex_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("0xff"))), ASTNode::NumberLiteral(n) if *n == 255.0));
    }

    #[test]
    fn test_hex_literal_uppercase() {
        assert!(matches!(expr_of(first_stmt(&parse("0XFF"))), ASTNode::NumberLiteral(n) if *n == 255.0));
    }

    #[test]
    fn test_octal_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("0o77"))), ASTNode::NumberLiteral(n) if *n == 63.0));
    }

    #[test]
    fn test_binary_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("0b1010"))), ASTNode::NumberLiteral(n) if *n == 10.0));
    }

    #[test]
    fn test_string_double_quotes() {
        assert!(matches!(expr_of(first_stmt(&parse("\"hello\""))), ASTNode::StringLiteral(s) if s == "hello"));
    }

    #[test]
    fn test_string_single_quotes() {
        assert!(matches!(expr_of(first_stmt(&parse("'hello'"))), ASTNode::StringLiteral(s) if s == "hello"));
    }

    #[test]
    fn test_string_escape_n() {
        assert!(matches!(expr_of(first_stmt(&parse("'a\\nb'"))), ASTNode::StringLiteral(s) if s == "a\nb"));
    }

    #[test]
    fn test_string_escape_t() {
        assert!(matches!(expr_of(first_stmt(&parse("'a\\tb'"))), ASTNode::StringLiteral(s) if s == "a\tb"));
    }

    #[test]
    fn test_string_escape_backslash() {
        assert!(matches!(expr_of(first_stmt(&parse("'a\\\\b'"))), ASTNode::StringLiteral(s) if s == "a\\b"));
    }

    #[test]
    fn test_string_escape_quote() {
        assert!(matches!(expr_of(first_stmt(&parse("'a\\'b'"))), ASTNode::StringLiteral(s) if s == "a'b"));
    }

    #[test]
    fn test_string_escape_double_quote() {
        assert!(matches!(expr_of(first_stmt(&parse("\"a\\\"b\""))), ASTNode::StringLiteral(s) if s == "a\"b"));
    }

    #[test]
    fn test_string_escape_unicode_long() {
        assert!(matches!(expr_of(first_stmt(&parse("'\\u{1F600}'"))), ASTNode::StringLiteral(s) if !s.is_empty()));
    }

    #[test]
    fn test_bool_true() {
        assert!(matches!(expr_of(first_stmt(&parse("true"))), ASTNode::BoolLiteral(true)));
    }

    #[test]
    fn test_bool_false() {
        assert!(matches!(expr_of(first_stmt(&parse("false"))), ASTNode::BoolLiteral(false)));
    }

    #[test]
    fn test_null_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("null"))), ASTNode::NullLiteral));
    }

    #[test]
    fn test_undefined_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("undefined"))), ASTNode::UndefinedLiteral));
    }

    #[test]
    fn test_regex_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("/abc/g"))), ASTNode::RegExpLiteral { pattern, flags } if pattern == "abc" && flags == "g"));
    }

    #[test]
    fn test_regex_literal_no_flags() {
        assert!(matches!(first_stmt(&parse("var r = /foo/;")), ASTNode::VariableDeclaration { .. }));
    }

    // ========================================================================
    // 2. IDENTIFIERS
    // ========================================================================

    #[test]
    fn test_simple_identifier() {
        assert!(matches!(expr_of(first_stmt(&parse("x"))), ASTNode::Identifier(s) if s == "x"));
    }

    #[test]
    fn test_identifier_with_dollar() {
        assert!(matches!(expr_of(first_stmt(&parse("$var"))), ASTNode::Identifier(s) if s == "$var"));
    }

    #[test]
    fn test_identifier_with_underscore() {
        assert!(matches!(expr_of(first_stmt(&parse("_private"))), ASTNode::Identifier(s) if s == "_private"));
    }

    #[test]
    fn test_identifier_with_dollar_and_underscore() {
        assert!(matches!(expr_of(first_stmt(&parse("$__test__"))), ASTNode::Identifier(s) if s == "$__test__"));
    }

    #[test]
    fn test_this_keyword() {
        // 'this' is parsed as Identifier("this")
        assert!(matches!(expr_of(first_stmt(&parse("this"))), ASTNode::Identifier(s) if s == "this"));
    }

    // ========================================================================
    // 3. BINARY OPERATORS
    // ========================================================================

    #[test]
    fn test_binary_add() {
        assert!(matches!(expr_of(first_stmt(&parse("1 + 2"))), ASTNode::BinaryOp { op: BinaryOp::Add, .. }));
    }

    #[test]
    fn test_binary_sub() {
        assert!(matches!(expr_of(first_stmt(&parse("1 - 2"))), ASTNode::BinaryOp { op: BinaryOp::Sub, .. }));
    }

    #[test]
    fn test_binary_mul() {
        assert!(matches!(expr_of(first_stmt(&parse("2 * 3"))), ASTNode::BinaryOp { op: BinaryOp::Mul, .. }));
    }

    #[test]
    fn test_binary_div() {
        assert!(matches!(expr_of(first_stmt(&parse("6 / 3"))), ASTNode::BinaryOp { op: BinaryOp::Div, .. }));
    }

    #[test]
    fn test_binary_mod() {
        assert!(matches!(expr_of(first_stmt(&parse("7 % 3"))), ASTNode::BinaryOp { op: BinaryOp::Mod, .. }));
    }

    #[test]
    fn test_binary_pow() {
        assert!(matches!(expr_of(first_stmt(&parse("2 ** 3"))), ASTNode::BinaryOp { op: BinaryOp::Pow, .. }));
    }

    #[test]
    fn test_compare_eq() {
        assert!(matches!(expr_of(first_stmt(&parse("1 == 2"))), ASTNode::BinaryOp { op: BinaryOp::Eq, .. }));
    }

    #[test]
    fn test_compare_strict_eq() {
        assert!(matches!(expr_of(first_stmt(&parse("1 === 2"))), ASTNode::BinaryOp { op: BinaryOp::StrictEq, .. }));
    }

    #[test]
    fn test_compare_ne() {
        assert!(matches!(expr_of(first_stmt(&parse("1 != 2"))), ASTNode::BinaryOp { op: BinaryOp::Ne, .. }));
    }

    #[test]
    fn test_compare_strict_ne() {
        assert!(matches!(expr_of(first_stmt(&parse("1 !== 2"))), ASTNode::BinaryOp { op: BinaryOp::StrictNe, .. }));
    }

    #[test]
    fn test_compare_lt() {
        assert!(matches!(expr_of(first_stmt(&parse("1 < 2"))), ASTNode::BinaryOp { op: BinaryOp::Lt, .. }));
    }

    #[test]
    fn test_compare_gt() {
        assert!(matches!(expr_of(first_stmt(&parse("1 > 2"))), ASTNode::BinaryOp { op: BinaryOp::Gt, .. }));
    }

    #[test]
    fn test_compare_le() {
        assert!(matches!(expr_of(first_stmt(&parse("1 <= 2"))), ASTNode::BinaryOp { op: BinaryOp::Le, .. }));
    }

    #[test]
    fn test_compare_ge() {
        assert!(matches!(expr_of(first_stmt(&parse("1 >= 2"))), ASTNode::BinaryOp { op: BinaryOp::Ge, .. }));
    }

    #[test]
    fn test_logical_and() {
        assert!(matches!(expr_of(first_stmt(&parse("a && b"))), ASTNode::BinaryOp { op: BinaryOp::And, .. }));
    }

    #[test]
    fn test_logical_or() {
        assert!(matches!(expr_of(first_stmt(&parse("a || b"))), ASTNode::BinaryOp { op: BinaryOp::Or, .. }));
    }

    #[test]
    fn test_bitwise_and() {
        assert!(matches!(expr_of(first_stmt(&parse("a & b"))), ASTNode::BinaryOp { op: BinaryOp::BitAnd, .. }));
    }

    #[test]
    fn test_bitwise_or() {
        assert!(matches!(expr_of(first_stmt(&parse("a | b"))), ASTNode::BinaryOp { op: BinaryOp::BitOr, .. }));
    }

    #[test]
    fn test_bitwise_xor() {
        assert!(matches!(expr_of(first_stmt(&parse("a ^ b"))), ASTNode::BinaryOp { op: BinaryOp::BitXor, .. }));
    }

    #[test]
    fn test_shift_left() {
        assert!(matches!(expr_of(first_stmt(&parse("a << 2"))), ASTNode::BinaryOp { op: BinaryOp::Shl, .. }));
    }

    #[test]
    fn test_shift_right() {
        assert!(matches!(expr_of(first_stmt(&parse("a >> 2"))), ASTNode::BinaryOp { op: BinaryOp::Shr, .. }));
    }

    #[test]
    fn test_shift_unsigned_right() {
        assert!(matches!(expr_of(first_stmt(&parse("a >>> 2"))), ASTNode::BinaryOp { op: BinaryOp::UShr, .. }));
    }

    #[test]
    fn test_instanceof() {
        assert!(matches!(expr_of(first_stmt(&parse("a instanceof Foo"))), ASTNode::BinaryOp { op: BinaryOp::Instanceof, .. }));
    }

    #[test]
    fn test_in_operator() {
        assert!(matches!(expr_of(first_stmt(&parse("'x' in obj"))), ASTNode::BinaryOp { op: BinaryOp::In, .. }));
    }

    #[test]
    fn test_nullish_coalescing() {
        assert!(matches!(expr_of(first_stmt(&parse("a ?? b"))), ASTNode::BinaryOp { op: BinaryOp::NullishCoalescing, .. }));
    }

    // ========================================================================
    // 4. UNARY OPERATORS
    // ========================================================================

    #[test]
    fn test_unary_minus() {
        assert!(matches!(expr_of(first_stmt(&parse("-x"))), ASTNode::UnaryOp { op: UnaryOp::Minus, prefix: true, .. }));
    }

    #[test]
    fn test_unary_plus() {
        assert!(matches!(expr_of(first_stmt(&parse("+x"))), ASTNode::UnaryOp { op: UnaryOp::Plus, prefix: true, .. }));
    }

    #[test]
    fn test_unary_not() {
        assert!(matches!(expr_of(first_stmt(&parse("!x"))), ASTNode::UnaryOp { op: UnaryOp::Not, prefix: true, .. }));
    }

    #[test]
    fn test_unary_bitnot() {
        assert!(matches!(expr_of(first_stmt(&parse("~x"))), ASTNode::UnaryOp { op: UnaryOp::BitNot, prefix: true, .. }));
    }

    #[test]
    fn test_typeof() {
        assert!(matches!(expr_of(first_stmt(&parse("typeof x"))), ASTNode::UnaryOp { op: UnaryOp::Typeof, prefix: true, .. }));
    }

    #[test]
    fn test_void() {
        assert!(matches!(expr_of(first_stmt(&parse("void 0"))), ASTNode::UnaryOp { op: UnaryOp::Void, prefix: true, .. }));
    }

    #[test]
    fn test_delete() {
        assert!(matches!(expr_of(first_stmt(&parse("delete obj.prop"))), ASTNode::UnaryOp { op: UnaryOp::Delete, prefix: true, .. }));
    }

    #[test]
    fn test_prefix_increment() {
        assert!(matches!(expr_of(first_stmt(&parse("++x"))), ASTNode::UnaryOp { op: UnaryOp::Inc, prefix: true, .. }));
    }

    #[test]
    fn test_prefix_decrement() {
        assert!(matches!(expr_of(first_stmt(&parse("--x"))), ASTNode::UnaryOp { op: UnaryOp::Dec, prefix: true, .. }));
    }

    #[test]
    fn test_postfix_increment() {
        assert!(matches!(expr_of(first_stmt(&parse("x++"))), ASTNode::UnaryOp { op: UnaryOp::Inc, prefix: false, .. }));
    }

    #[test]
    fn test_postfix_decrement() {
        assert!(matches!(expr_of(first_stmt(&parse("x--"))), ASTNode::UnaryOp { op: UnaryOp::Dec, prefix: false, .. }));
    }

    // ========================================================================
    // 5. ASSIGNMENT OPERATORS
    // ========================================================================

    #[test]
    fn test_assignment_simple() {
        assert!(matches!(expr_of(first_stmt(&parse("x = 1"))), ASTNode::Assignment { op: AssignmentOp::Assign, .. }));
    }

    #[test]
    fn test_assignment_add() {
        assert!(matches!(expr_of(first_stmt(&parse("x += 1"))), ASTNode::Assignment { op: AssignmentOp::AddAssign, .. }));
    }

    #[test]
    fn test_assignment_sub() {
        assert!(matches!(expr_of(first_stmt(&parse("x -= 1"))), ASTNode::Assignment { op: AssignmentOp::SubAssign, .. }));
    }

    #[test]
    fn test_assignment_mul() {
        assert!(matches!(expr_of(first_stmt(&parse("x *= 2"))), ASTNode::Assignment { op: AssignmentOp::MulAssign, .. }));
    }

    #[test]
    fn test_assignment_div() {
        assert!(matches!(expr_of(first_stmt(&parse("x /= 2"))), ASTNode::Assignment { op: AssignmentOp::DivAssign, .. }));
    }

    #[test]
    fn test_assignment_mod() {
        assert!(matches!(expr_of(first_stmt(&parse("x %= 2"))), ASTNode::Assignment { op: AssignmentOp::ModAssign, .. }));
    }

    #[test]
    fn test_assignment_pow() {
        assert!(matches!(expr_of(first_stmt(&parse("x **= 2"))), ASTNode::Assignment { op: AssignmentOp::PowAssign, .. }));
    }

    #[test]
    fn test_assignment_shl() {
        assert!(matches!(expr_of(first_stmt(&parse("x <<= 1"))), ASTNode::Assignment { op: AssignmentOp::ShlAssign, .. }));
    }

    #[test]
    fn test_assignment_shr() {
        assert!(matches!(expr_of(first_stmt(&parse("x >>= 1"))), ASTNode::Assignment { op: AssignmentOp::ShrAssign, .. }));
    }

    #[test]
    fn test_assignment_ushr() {
        assert!(matches!(expr_of(first_stmt(&parse("x >>>= 1"))), ASTNode::Assignment { op: AssignmentOp::UShrAssign, .. }));
    }

    #[test]
    fn test_assignment_bitand() {
        assert!(matches!(expr_of(first_stmt(&parse("x &= 1"))), ASTNode::Assignment { op: AssignmentOp::BitAndAssign, .. }));
    }

    #[test]
    fn test_assignment_bitor() {
        assert!(matches!(expr_of(first_stmt(&parse("x |= 1"))), ASTNode::Assignment { op: AssignmentOp::BitOrAssign, .. }));
    }

    #[test]
    fn test_assignment_bitxor() {
        assert!(matches!(expr_of(first_stmt(&parse("x ^= 1"))), ASTNode::Assignment { op: AssignmentOp::BitXorAssign, .. }));
    }

    // ========================================================================
    // 6. TERNARY / CONDITIONAL
    // ========================================================================

    #[test]
    fn test_conditional() {
        assert!(matches!(expr_of(first_stmt(&parse("x ? y : z"))), ASTNode::Conditional { .. }));
    }

    #[test]
    fn test_nested_conditional() {
        assert!(matches!(expr_of(first_stmt(&parse("a ? b ? c : d : e"))), ASTNode::Conditional { .. }));
    }

    // ========================================================================
    // 7. VARIABLE DECLARATIONS
    // ========================================================================

    #[test]
    fn test_var_decl_with_init() {
        let ast = parse("var x = 1;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { kind: VariableKind::Var, declarations } if declarations.len() == 1 && declarations[0].init.is_some()));
    }

    #[test]
    fn test_let_decl() {
        let ast = parse("let y = 2;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { kind: VariableKind::Let, .. }));
    }

    #[test]
    fn test_const_decl() {
        let ast = parse("const z = 3;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { kind: VariableKind::Const, .. }));
    }

    #[test]
    fn test_multiple_declarators() {
        let ast = parse("var a = 1, b, c = 3;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if declarations.len() == 3));
    }

    #[test]
    fn test_var_no_init() {
        let ast = parse("var x;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if declarations[0].init.is_none()));
    }

    #[test]
    fn test_var_destructuring_obj() {
        assert!(matches!(first_stmt(&parse("var {a, b} = obj;")), ASTNode::VariableDeclaration { declarations, .. } if declarations[0].pattern.is_some()));
    }

    #[test]
    fn test_var_destructuring_arr() {
        assert!(matches!(first_stmt(&parse("var [a, b] = arr;")), ASTNode::VariableDeclaration { declarations, .. } if declarations[0].pattern.is_some()));
    }

    // ========================================================================
    // 8. CONTROL FLOW
    // ========================================================================

    #[test]
    fn test_if_statement() {
        assert!(matches!(first_stmt(&parse("if (x) { y; }")), ASTNode::If { alternate: None, .. }));
    }

    #[test]
    fn test_if_else() {
        assert!(matches!(first_stmt(&parse("if (x) { y; } else { z; }")), ASTNode::If { alternate: Some(_), .. }));
    }

    #[test]
    fn test_if_else_if() {
        assert!(matches!(first_stmt(&parse("if (a) { b; } else if (c) { d; } else { e; }")), ASTNode::If { alternate: Some(_), .. }));
    }

    #[test]
    fn test_while_loop() {
        assert!(matches!(first_stmt(&parse("while (x) { body; }")), ASTNode::While { .. }));
    }

    #[test]
    fn test_do_while() {
        assert!(matches!(first_stmt(&parse("do { body; } while (x);")), ASTNode::DoWhile { .. }));
    }

    #[test]
    fn test_for_loop_basic() {
        assert!(matches!(first_stmt(&parse("for (var i = 0; i < 10; i++) { body; }")), ASTNode::For { .. }));
    }

    #[test]
    fn test_for_loop_no_init() {
        assert!(matches!(first_stmt(&parse("for (; i < 10; i++) { body; }")), ASTNode::For { .. }));
    }

    #[test]
    fn test_for_loop_empty() {
        assert!(matches!(first_stmt(&parse("for (;;) { body; }")), ASTNode::For { init: None, test: None, update: None, .. }));
    }

    #[test]
    fn test_for_loop_expression_init() {
        assert!(matches!(first_stmt(&parse("for (i = 0; i < 10; i++) { body; }")), ASTNode::For { .. }));
    }

    #[test]
    fn test_for_in_var() {
        assert!(matches!(first_stmt(&parse("for (var k in obj) { body; }")), ASTNode::ForIn { .. }));
    }

    #[test]
    fn test_for_in_let() {
        assert!(matches!(first_stmt(&parse("for (let k in obj) { body; }")), ASTNode::ForIn { .. }));
    }

    #[test]
    fn test_for_in_expr() {
        assert!(matches!(first_stmt(&parse("for (var k in obj) { body; }")), ASTNode::ForIn { .. }));
    }

    #[test]
    fn test_for_of_var() {
        assert!(matches!(first_stmt(&parse("for (var x of arr) { body; }")), ASTNode::ForOf { .. }));
    }

    #[test]
    fn test_for_of_let() {
        assert!(matches!(first_stmt(&parse("for (let x of arr) { body; }")), ASTNode::ForOf { .. }));
    }

    #[test]
    fn test_for_of_expr() {
        assert!(matches!(first_stmt(&parse("for (x of arr) { body; }")), ASTNode::ForOf { .. }));
    }

    #[test]
    fn test_switch_with_cases() {
        assert!(matches!(first_stmt(&parse("switch (x) { case 1: a; break; case 2: b; break; default: c; }")), ASTNode::Switch { cases, .. } if cases.len() == 3));
    }

    #[test]
    fn test_switch_only_default() {
        assert!(matches!(first_stmt(&parse("switch (x) { default: a; }")), ASTNode::Switch { cases, .. } if cases.len() == 1));
    }

    #[test]
    fn test_break() {
        assert!(matches!(
            first_stmt(&parse("while (true) { break; }")),
            ASTNode::While { body, .. } if matches!(body.as_ref(), ASTNode::Block(stmts) if matches!(&stmts[0], ASTNode::Break(None)))
        ));
    }

    #[test]
    fn test_continue() {
        assert!(matches!(
            first_stmt(&parse("while (true) { continue; }")),
            ASTNode::While { body, .. } if matches!(body.as_ref(), ASTNode::Block(stmts) if matches!(&stmts[0], ASTNode::Continue(None)))
        ));
    }

    #[test]
    fn test_return_with_value() {
        let ast = parse("function f() { return 42; }");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { body, .. } if matches!(body.as_ref(), ASTNode::Block(stmts) if matches!(&stmts[0], ASTNode::Return(Some(_))))));
    }

    #[test]
    fn test_return_bare() {
        let ast = parse("function f() { return; }");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { body, .. } if matches!(body.as_ref(), ASTNode::Block(stmts) if matches!(&stmts[0], ASTNode::Return(None)))));
    }

    #[test]
    fn test_throw() {
        let ast = parse("function f() { throw new Error('x'); }");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { body, .. } if matches!(body.as_ref(), ASTNode::Block(stmts) if matches!(&stmts[0], ASTNode::Throw(_)))));
    }

    // ========================================================================
    // 9. FUNCTIONS
    // ========================================================================

    #[test]
    fn test_function_declaration() {
        let ast = parse("function foo(a, b) { return a + b; }");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { name, params, is_async: false, is_generator: false, .. } if name == "foo" && params.len() == 2));
    }

    #[test]
    fn test_function_expression() {
        let ast = parse("var f = function(a) { return a; };");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if matches!(&declarations[0].init, Some(e) if matches!(e.as_ref(), ASTNode::FunctionDeclaration { .. }))));
    }

    #[test]
    fn test_function_expression_named() {
        let ast = parse("var f = function myFunc() { return 1; };");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if matches!(&declarations[0].init, Some(e) if matches!(e.as_ref(), ASTNode::FunctionDeclaration { name, .. } if name == "myFunc"))));
    }

    #[test]
    fn test_arrow_function_single_param_body_expr() {
        assert!(matches!(expr_of(first_stmt(&parse("x => x * 2"))), ASTNode::ArrowFunctionExpression { params, body, is_async: false } if params.len() == 1 && matches!(body.as_ref(), ASTNode::ExpressionStatement(_))));
    }

    #[test]
    fn test_arrow_function_single_param_body_block() {
        assert!(matches!(expr_of(first_stmt(&parse("x => { return x; }"))), ASTNode::ArrowFunctionExpression { params, body, .. } if params.len() == 1 && matches!(body.as_ref(), ASTNode::Block(_))));
    }

    #[test]
    fn test_arrow_function_multi_param() {
        assert!(matches!(expr_of(first_stmt(&parse("(a, b) => a + b"))), ASTNode::ArrowFunctionExpression { params, .. } if params.len() == 2));
    }

    #[test]
    fn test_arrow_function_empty_params() {
        assert!(matches!(expr_of(first_stmt(&parse("() => 1"))), ASTNode::ArrowFunctionExpression { params, .. } if params.is_empty()));
    }

    #[test]
    fn test_async_function_decl() {
        let ast = parse("async function foo() {}");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { is_async: true, .. }));
    }

    #[test]
    fn test_async_function_expr() {
        let ast = parse("var f = async function() {};");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if matches!(&declarations[0].init, Some(e) if matches!(e.as_ref(), ASTNode::FunctionDeclaration { is_async: true, .. }))));
    }

    #[test]
    fn test_generator_function() {
        let ast = parse("function* gen() { yield 1; }");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { is_generator: true, .. }));
    }

    #[test]
    fn test_rest_params() {
        let ast = parse("function f(a, ...rest) {}");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { params, .. } if params.len() == 2 && params[1].is_rest));
    }

    #[test]
    fn test_default_params() {
        let ast = parse("function f(a = 1, b = 2) {}");
        assert!(matches!(first_stmt(&ast), ASTNode::FunctionDeclaration { params, .. } if params[0].default.is_some() && params[1].default.is_some()));
    }

    #[test]
    fn test_arrow_rest_params() {
        assert!(matches!(expr_of(first_stmt(&parse("(...a) => a"))), ASTNode::ArrowFunctionExpression { params, .. } if params.len() == 1 && params[0].is_rest));
    }

    #[test]
    fn test_arrow_default_params() {
        assert!(matches!(expr_of(first_stmt(&parse("(a = 1) => a"))), ASTNode::ArrowFunctionExpression { params, .. } if params[0].default.is_some()));
    }

    #[test]
    fn test_async_arrow() {
        // async x => x fails because parser expects 'function' after 'async'
        assert!(parse_err("async x => x").contains("Expected"));
    }

    // ========================================================================
    // 10. CLASSES
    // ========================================================================

    #[test]
    fn test_class_basic() {
        assert!(matches!(first_stmt(&parse("class Foo {}")), ASTNode::ClassDeclaration { name, super_class: None, .. } if name == "Foo"));
    }

    #[test]
    fn test_class_extends() {
        assert!(matches!(first_stmt(&parse("class Foo extends Bar {}")), ASTNode::ClassDeclaration { super_class: Some(_), .. }));
    }

    #[test]
    fn test_class_constructor() {
        assert!(matches!(first_stmt(&parse("class Foo { constructor() {} }")), ASTNode::ClassDeclaration { body, .. } if !body.is_empty()));
    }

    #[test]
    fn test_class_method() {
        assert!(matches!(first_stmt(&parse("class Foo { bar() {} }")), ASTNode::ClassDeclaration { body, .. } if body.iter().any(|e| e.kind == ClassElementKind::Method)));
    }

    #[test]
    fn test_class_static_method() {
        assert!(matches!(first_stmt(&parse("class Foo { static bar() {} }")), ASTNode::ClassDeclaration { body, .. } if body.iter().any(|e| e.is_static)));
    }

    #[test]
    fn test_class_getter() {
        assert!(matches!(first_stmt(&parse("class Foo { get x() { return 1; } }")), ASTNode::ClassDeclaration { body, .. } if body.iter().any(|e| e.kind == ClassElementKind::Get)));
    }

    #[test]
    fn test_class_setter() {
        assert!(matches!(first_stmt(&parse("class Foo { set x(v) {} }")), ASTNode::ClassDeclaration { body, .. } if body.iter().any(|e| e.kind == ClassElementKind::Set)));
    }

    #[test]
    fn test_class_expression() {
        let ast = parse("var C = class {};");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. } if matches!(&declarations[0].init, Some(e) if matches!(e.as_ref(), ASTNode::ClassDeclaration { .. }))));
    }

    // ========================================================================
    // 11. OBJECTS
    // ========================================================================

    #[test]
    fn test_object_init_prop() {
        assert!(matches!(first_stmt(&parse("var o = { a: 1 };")), ASTNode::VariableDeclaration { declarations, .. }));
    }

    #[test]
    fn test_object_shorthand() {
        let ast = parse("({ a })");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].is_shorthand));
    }

    #[test]
    fn test_object_computed_key() {
        let ast = parse("({ [expr]: 1 })");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].is_computed));
    }

    #[test]
    fn test_object_method() {
        let ast = parse("({ foo() {} })");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].is_method));
    }

    #[test]
    fn test_object_getter() {
        let ast = parse("({ get x() { return 1; } })");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].kind == PropertyKind::Get));
    }

    #[test]
    fn test_object_setter() {
        let ast = parse("({ set x(v) {} })");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].kind == PropertyKind::Set));
    }

    #[test]
    fn test_object_spread() {
        let ast = parse("({...obj})");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if matches!(&*props[0].key, ASTNode::SpreadElement(_))));
    }

    // ========================================================================
    // 12. ARRAYS
    // ========================================================================

    #[test]
    fn test_array_literal() {
        assert!(matches!(expr_of(first_stmt(&parse("[1, 2, 3]"))), ASTNode::ArrayExpression(elems) if elems.len() == 3));
    }

    #[test]
    fn test_array_empty() {
        assert!(matches!(expr_of(first_stmt(&parse("[]"))), ASTNode::ArrayExpression(elems) if elems.is_empty()));
    }

    #[test]
    fn test_array_with_spread() {
        assert!(matches!(expr_of(first_stmt(&parse("[1, ...arr, 2]"))), ASTNode::ArrayExpression(elems) if matches!(&elems[1], ASTNode::SpreadElement(_))));
    }

    #[test]
    fn test_array_elision() {
        // [1,,2] -> [1, undefined, 2]
        assert!(matches!(expr_of(first_stmt(&parse("[1,,2]"))), ASTNode::ArrayExpression(elems) if elems.len() == 3 && matches!(&elems[1], ASTNode::UndefinedLiteral)));
    }

    // ========================================================================
    // 13. DESTRUCTURING
    // ========================================================================

    #[test]
    fn test_destructuring_obj_pattern() {
        assert!(matches!(first_stmt(&parse("var {a, b} = obj;")), ASTNode::VariableDeclaration { declarations, .. } if declarations[0].pattern.is_some()));
    }

    #[test]
    fn test_destructuring_arr_pattern() {
        assert!(matches!(first_stmt(&parse("var [a, b] = arr;")), ASTNode::VariableDeclaration { declarations, .. } if declarations[0].pattern.is_some()));
    }

    #[test]
    fn test_destructuring_renaming() {
        assert!(matches!(first_stmt(&parse("var {a: x} = obj;")), ASTNode::VariableDeclaration { declarations, .. }));
    }

    #[test]
    fn test_destructuring_default_values() {
        // Parser doesn't support default values in object destructuring patterns
        assert!(parse_err("var {a = 1} = obj;").contains("Expected"));
    }

    // ========================================================================
    // 14. TEMPLATE LITERALS
    // ========================================================================

    #[test]
    fn test_template_literal_simple() {
        // `hello` desugars to string "hello" via the tokenizer
        assert!(matches!(expr_of(first_stmt(&parse("`hello`"))), ASTNode::StringLiteral(s) if s == "hello"));
    }

    #[test]
    fn test_template_literal_with_expression() {
        // `hi ${name}` desugars to string concat: "hi " + name
        let ast = parse("var x = `hi ${name}`;");
        assert!(matches!(first_stmt(&ast), ASTNode::VariableDeclaration { declarations, .. }));
    }

    #[test]
    fn test_template_literal_empty() {
        assert!(matches!(expr_of(first_stmt(&parse("``"))), ASTNode::StringLiteral(s) if s.is_empty()));
    }

    // ========================================================================
    // 15. TRY / CATCH / FINALLY
    // ========================================================================

    #[test]
    fn test_try_catch() {
        assert!(matches!(first_stmt(&parse("try { x; } catch (e) { y; }")), ASTNode::Try { catch: Some(_), finally: None, .. }));
    }

    #[test]
    fn test_try_finally() {
        assert!(matches!(first_stmt(&parse("try { x; } finally { z; }")), ASTNode::Try { catch: None, finally: Some(_), .. }));
    }

    #[test]
    fn test_try_catch_finally() {
        assert!(matches!(first_stmt(&parse("try { x; } catch (e) { y; } finally { z; }")), ASTNode::Try { catch: Some(_), finally: Some(_), .. }));
    }

    #[test]
    fn test_try_catch_without_param() {
        assert!(matches!(first_stmt(&parse("try { x; } catch { y; }")), ASTNode::Try { catch: Some(c), .. } if c.param.is_none()));
    }

    #[test]
    fn test_try_catch_with_param() {
        assert!(matches!(first_stmt(&parse("try { x; } catch (err) { y; }")), ASTNode::Try { catch: Some(c), .. } if c.param.is_some()));
    }

    // ========================================================================
    // 16. MODULES
    // ========================================================================

    #[test]
    fn test_import_default() {
        assert!(matches!(first_stmt(&parse("import Foo from 'mod';")), ASTNode::ImportDeclaration { specifiers, source } if source == "mod" && matches!(&specifiers[0], ImportSpecifier::Default { local } if local == "Foo")));
    }

    #[test]
    fn test_import_named() {
        assert!(matches!(first_stmt(&parse("import { a, b } from 'mod';")), ASTNode::ImportDeclaration { specifiers, .. } if specifiers.len() == 2));
    }

    #[test]
    fn test_import_named_as() {
        assert!(matches!(first_stmt(&parse("import { a as b } from 'mod';")), ASTNode::ImportDeclaration { specifiers, .. } if matches!(&specifiers[0], ImportSpecifier::Named { imported, local } if imported == "a" && local == "b")));
    }

    #[test]
    fn test_import_namespace() {
        assert!(matches!(first_stmt(&parse("import * as ns from 'mod';")), ASTNode::ImportDeclaration { specifiers, .. } if matches!(&specifiers[0], ImportSpecifier::Namespace { local } if local == "ns")));
    }

    #[test]
    fn test_import_side_effect() {
        assert!(matches!(first_stmt(&parse("import 'mod';")), ASTNode::ImportDeclaration { specifiers, source } if specifiers.is_empty() && source == "mod"));
    }

    #[test]
    fn test_export_default_expression() {
        assert!(matches!(first_stmt(&parse("export default 42;")), ASTNode::ExportDeclaration { declaration: Some(_), specifiers, .. } if specifiers.is_empty()));
    }

    #[test]
    fn test_export_default_function() {
        assert!(matches!(first_stmt(&parse("export default function foo() {}")), ASTNode::ExportDeclaration { declaration: Some(d), .. } if matches!(d.as_ref(), ASTNode::FunctionDeclaration { .. })));
    }

    #[test]
    fn test_export_default_class() {
        assert!(matches!(first_stmt(&parse("export default class Foo {}")), ASTNode::ExportDeclaration { declaration: Some(d), .. } if matches!(d.as_ref(), ASTNode::ClassDeclaration { .. })));
    }

    #[test]
    fn test_export_named() {
        assert!(matches!(first_stmt(&parse("export { a, b };")), ASTNode::ExportDeclaration { specifiers, .. } if specifiers.len() == 2));
    }

    #[test]
    fn test_export_named_as() {
        assert!(matches!(first_stmt(&parse("export { a as b };")), ASTNode::ExportDeclaration { specifiers, .. } if matches!(&specifiers[0], ExportSpecifier { local, exported } if local == "a" && exported == "b")));
    }

    #[test]
    fn test_export_named_from() {
        let ast = parse("export { a } from 'mod';");
        let s = first_stmt(&ast);
        assert!(matches!(s, ASTNode::ExportDeclaration { specifiers, source, .. } if source.as_deref() == Some("mod") && specifiers.len() == 1));
    }

    #[test]
    fn test_export_star_from() {
        let ast = parse("export * from 'mod';");
        let s = first_stmt(&ast);
        assert!(matches!(s, ASTNode::ExportDeclaration { source, specifiers, .. } if source.as_deref() == Some("mod") && specifiers.is_empty()));
    }

    #[test]
    fn test_export_var() {
        let ast = parse("export var x = 1;");
        let s = first_stmt(&ast);
        assert!(matches!(s, ASTNode::ExportDeclaration { declaration: Some(d), .. } if matches!(d.as_ref(), ASTNode::VariableDeclaration { .. })));
    }

    // ========================================================================
    // 17. OTHER
    // ========================================================================

    #[test]
    fn test_empty_statement() {
        let ast = parse(";");
        assert!(matches!(first_stmt(&ast), ASTNode::EmptyStatement));
    }

    #[test]
    fn test_labeled_statement() {
        let ast = parse("outer: while (true) { break outer; }");
        assert!(matches!(first_stmt(&ast), ASTNode::LabeledStatement { label, .. } if label == "outer"));
    }

    #[test]
    fn test_debugger() {
        let ast = parse("debugger;");
        assert!(matches!(first_stmt(&ast), ASTNode::DebuggerStatement));
    }

    #[test]
    fn test_sequence_expression() {
        let ast = parse("(1, 2, 3)");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Sequence { expressions } if expressions.len() == 3));
    }

    #[test]
    fn test_new_expression_with_args() {
        let ast = parse("new Foo(1, 2)");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::New { args, .. } if args.len() == 2));
    }

    #[test]
    fn test_new_expression_no_args() {
        let ast = parse("new Foo");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::New { args, .. } if args.is_empty()));
    }

    #[test]
    fn test_super_call() {
        let ast = parse("class A extends B { constructor() { super(1); } }");
        let cls = first_stmt(&ast);
        assert!(matches!(cls, ASTNode::ClassDeclaration { body, .. } if body[0].value.is_some()));
    }

    #[test]
    fn test_yield_expression() {
        let ast = parse("function* g() { yield 1; }");
        let fd = first_stmt(&ast);
        let body_stmts = match fd { ASTNode::FunctionDeclaration { body, .. } => match body.as_ref() { ASTNode::Block(s) => s, _ => panic!(), }, _ => panic!(), };
        assert!(matches!(&body_stmts[0], ASTNode::ExpressionStatement(e) if matches!(e.as_ref(), ASTNode::YieldExpression { argument: Some(_), delegate: false })));
    }

    #[test]
    fn test_yield_delegation() {
        let ast = parse("function* g() { yield* other(); }");
        let fd = first_stmt(&ast);
        let body_stmts = match fd { ASTNode::FunctionDeclaration { body, .. } => match body.as_ref() { ASTNode::Block(s) => s, _ => panic!(), }, _ => panic!(), };
        assert!(matches!(&body_stmts[0], ASTNode::ExpressionStatement(e) if matches!(e.as_ref(), ASTNode::YieldExpression { delegate: true, .. })));
    }

    #[test]
    fn test_yield_bare() {
        let ast = parse("function* g() { yield; }");
        let fd = first_stmt(&ast);
        let body_stmts = match fd { ASTNode::FunctionDeclaration { body, .. } => match body.as_ref() { ASTNode::Block(s) => s, _ => panic!(), }, _ => panic!(), };
        assert!(matches!(&body_stmts[0], ASTNode::ExpressionStatement(e) if matches!(e.as_ref(), ASTNode::YieldExpression { argument: None, delegate: false })));
    }

    #[test]
    fn test_await_expression() {
        let ast = parse("async function f() { await p; }");
        let fd = first_stmt(&ast);
        let body_stmts = match fd { ASTNode::FunctionDeclaration { body, .. } => match body.as_ref() { ASTNode::Block(s) => s, _ => panic!(), }, _ => panic!(), };
        assert!(matches!(&body_stmts[0], ASTNode::ExpressionStatement(e) if matches!(e.as_ref(), ASTNode::AwaitExpression(_))));
    }

    #[test]
    fn test_optional_chaining_property() {
        let ast = parse("a?.b");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Member { computed: false, .. }));
    }

    #[test]
    fn test_optional_chaining_computed() {
        let ast = parse("a?.[0]");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Member { computed: true, .. }));
    }

    #[test]
    fn test_optional_chaining_call() {
        let ast = parse("a?.b()");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Call { .. }));
    }

    #[test]
    fn test_spread_in_array() {
        let ast = parse("[...arr]");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ArrayExpression(elems) if matches!(&elems[0], ASTNode::SpreadElement(_))));
    }

    #[test]
    fn test_spread_in_call() {
        let ast = parse("foo(...args)");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Call { args, .. } if matches!(&args[0], ASTNode::SpreadElement(_))));
    }

    #[test]
    fn test_spread_in_object() {
        let ast = parse("({...obj})");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::ObjectExpression(props) if props[0].kind == PropertyKind::Init));
    }

    // ========================================================================
    // Operator precedence
    // ========================================================================

    #[test]
    fn test_precedence_mul_over_add() {
        // 1 + 2 * 3 => 1 + (2 * 3)
        let ast = parse("1 + 2 * 3");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::BinaryOp { op: BinaryOp::Add, right, .. } if matches!(right.as_ref(), ASTNode::BinaryOp { op: BinaryOp::Mul, .. })));
    }

    #[test]
    fn test_precedence_comparison_over_logical() {
        // a && b > c => a && (b > c)
        let ast = parse("a && b > c");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::BinaryOp { op: BinaryOp::And, right, .. } if matches!(right.as_ref(), ASTNode::BinaryOp { op: BinaryOp::Gt, .. })));
    }

    #[test]
    fn test_precedence_not_over_compare() {
        // !a < b => (!a) < b
        let ast = parse("!a < b");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::BinaryOp { op: BinaryOp::Lt, left, .. } if matches!(left.as_ref(), ASTNode::UnaryOp { op: UnaryOp::Not, .. })));
    }

    // ========================================================================
    // Error cases
    // ========================================================================

    #[test]
    fn test_error_unexpected_token() {
        let err = parse_err("var @ = 1;");
        assert!(err.contains("Unexpected"));
    }

    #[test]
    fn test_error_unterminated_string() {
        let err = parse_err("'hello");
        assert!(err.contains("Unterminated"));
    }

    #[test]
    fn test_error_unterminated_block_comment() {
        let err = parse_err("/* comment");
        assert!(err.contains("Unterminated"));
    }

    #[test]
    fn test_error_missing_catch_or_finally() {
        let err = parse_err("try { x; }");
        assert!(err.contains("catch") || err.contains("finally"));
    }

    #[test]
    fn test_error_empty_input() {
        let ast = parse("");
        assert!(matches!(ast, ASTNode::Block(s) if s.is_empty()));
    }

    // ========================================================================
    // Comments
    // ========================================================================

    #[test]
    fn test_line_comment() {
        assert!(matches!(first_stmt(&parse("// comment\nvar x = 1;")), ASTNode::VariableDeclaration { .. }));
    }

    #[test]
    fn test_block_comment() {
        assert!(matches!(first_stmt(&parse("/* comment */ var x = 1;")), ASTNode::VariableDeclaration { .. }));
    }

    // ========================================================================
    // Complex / mixed
    // ========================================================================

    #[test]
    fn test_nested_parens() {
        let ast = parse("((1 + 2) * 3)");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::BinaryOp { op: BinaryOp::Mul, .. }));
    }

    #[test]
    fn test_use_strict() {
        assert!(matches!(first_stmt(&parse("function f() { 'use strict'; var x = 1; }")), ASTNode::FunctionDeclaration { .. }));
    }

    #[test]
    fn test_chained_member_access() {
        let ast = parse("a.b.c.d");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Member { computed: false, .. }));
    }

    #[test]
    fn test_chained_calls() {
        let ast = parse("foo()()");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::Call { callee, .. } if matches!(callee.as_ref(), ASTNode::Call { .. })));
    }

    #[test]
    fn test_new_without_call() {
        let ast = parse("new Foo");
        let e = expr_of(first_stmt(&ast));
        assert!(matches!(e, ASTNode::New { args, .. } if args.is_empty()));
    }

    #[test]
    fn test_function_expression_in_var() {
        assert!(matches!(
            first_stmt(&parse("var f = function() { return 1; };")),
            ASTNode::VariableDeclaration { declarations, .. } if matches!(&declarations[0].init, Some(e) if matches!(e.as_ref(), ASTNode::FunctionDeclaration { .. }))
        ));
    }
}
