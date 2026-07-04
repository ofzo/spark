//! JavaScript lexer/tokenizer.
//!
//! Tokenizes JavaScript source code into a stream of tokens.
//! No unsafe code, no external dependencies.

use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

use crate::parser::{Keyword, TemplateToken, Token, TokenType};

/// Lexer error.
#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl LexerError {
    fn new(message: &str, line: usize, column: usize) -> Self {
        LexerError {
            message: message.to_string(),
            line,
            column,
        }
    }
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LexerError: {} (line {}, column {})",
            self.message, self.line, self.column
        )
    }
}

impl std::error::Error for LexerError {}

/// A JavaScript lexer/tokenizer.
pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    pos: usize,
    line: usize,
    column: usize,
    /// Lookahead token buffer
    peeked: Vec<Token>,
    /// Whether the previous token was a "value" (expression context).
    /// Used to disambiguate `/` as regex vs division.
    prev_is_value: bool,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source code.
    pub fn new(source: &'a str) -> Self {
        Lexer {
            chars: source.chars().peekable(),
            pos: 0,
            line: 1,
            column: 1,
            peeked: Vec::new(),
            prev_is_value: false,
        }
    }

    /// Peek at the next token without consuming it.
    pub fn peek_token(&mut self) -> Result<&Token, LexerError> {
        if self.peeked.is_empty() {
            let token = self.next_token()?;
            self.peeked.push(token);
        }
        Ok(&self.peeked[0])
    }

    /// Get the next token from the source.
    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        if let Some(token) = self.peeked.pop() {
            return Ok(token);
        }
        self.scan_token()
    }

    /// Scan the next token from source.
    fn scan_token(&mut self) -> Result<Token, LexerError> {
        self.skip_whitespace_and_comments();

        let start_line = self.line;
        let start_column = self.column;
        let start_pos = self.pos;

        let ch = match self.peek_char() {
            None => {
                return Ok(Token {
                    token_type: TokenType::Eof,
                    line: start_line,
                    column: start_column,
                    start: start_pos,
                    end: start_pos,
                });
            }
            Some(c) => c,
        };

        let token_type = match ch {
            // Single-character tokens
            '(' => { self.advance(); TokenType::LParen }
            ')' => { self.advance(); TokenType::RParen }
            '{' => { self.advance(); TokenType::LBrace }
            '}' => { self.advance(); TokenType::RBrace }
            '[' => { self.advance(); TokenType::LBracket }
            ']' => { self.advance(); TokenType::RBracket }
            ';' => { self.advance(); TokenType::Semicolon }
            ',' => { self.advance(); TokenType::Comma }
            ':' => { self.advance(); TokenType::Colon }
            '~' => { self.advance(); TokenType::BitNot }

            // Operators that can be doubled or combined
            '+' => self.scan_plus()?,
            '-' => self.scan_minus()?,
            '*' => self.scan_star()?,
            '%' => self.scan_percent()?,
            '&' => self.scan_ampersand()?,
            '|' => self.scan_pipe()?,
            '^' => self.scan_caret()?,
            '!' => self.scan_bang()?,
            '=' => self.scan_equal()?,
            '<' => self.scan_less()?,
            '>' => self.scan_greater()?,
            '?' => self.scan_question()?,

            '.' => {
                self.advance();
                if self.peek_char() == Some('.') {
                    self.advance();
                    if self.peek_char() == Some('.') {
                        self.advance();
                        TokenType::Ellipsis
                    } else {
                        // Two dots is an error in JS
                        return Err(LexerError::new("Unexpected '..'", start_line, start_column));
                    }
                } else if self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                    // Float starting with .
                    self.scan_number_after_dot()?
                } else {
                    TokenType::Dot
                }
            }

            // Strings
            '\'' | '"' => self.scan_string(ch)?,

            // Template literals
            '`' => self.scan_template_literal()?,

            // Numbers
            '0' => {
                self.advance();
                if self.peek_char() == Some('x') || self.peek_char() == Some('X') {
                    self.scan_hex_number()?
                } else if self.peek_char() == Some('o') || self.peek_char() == Some('O') {
                    self.scan_octal_number()?
                } else if self.peek_char() == Some('b') || self.peek_char() == Some('B') {
                    self.scan_binary_number()?
                } else {
                    self.scan_number_rest(0.0)?
                }
            }
            c if c.is_ascii_digit() => {
                self.scan_number()?
            }

            // Regex literal or division
            '/' => self.scan_regex_or_division()?,

            // Identifiers and keywords (ASCII)
            c if is_identifier_start(c) => self.scan_identifier_or_keyword()?,

            // Unicode identifier start
            c if !c.is_ascii() && is_unicode_identifier_start(c) => self.scan_identifier_or_keyword()?,

            _ => {
                self.advance();
                return Err(LexerError::new(
                    &format!("Unexpected character: '{}'", ch),
                    start_line,
                    start_column,
                ));
            }
        };

        // Update prev_is_value based on the token we just produced
        self.prev_is_value = is_value_token(&token_type);

        Ok(Token {
            token_type,
            line: start_line,
            column: start_column,
            start: start_pos,
            end: self.pos,
        })
    }

    // --- Character navigation ---

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn peek_char_at_offset(&mut self, offset: usize) -> Option<char> {
        self.chars.clone().nth(offset)
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn expect(&mut self, expected: char) -> Result<(), LexerError> {
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            _ => Err(LexerError::new(
                &format!("Expected '{}'", expected),
                self.line,
                self.column,
            )),
        }
    }

    // --- Whitespace and comments ---

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                None => break,
                Some(ch) if ch.is_whitespace() => {
                    self.advance();
                }
                Some('/') => {
                    // Check for comments
                    if self.peek_char_at_offset(1) == Some('/') {
                        // Line comment
                        self.advance(); // first /
                        self.advance(); // second /
                        while let Some(ch) = self.peek_char() {
                            if ch == '\n' {
                                break;
                            }
                            self.advance();
                        }
                    } else if self.peek_char_at_offset(1) == Some('*') {
                        // Block comment
                        self.advance(); // /
                        self.advance(); // *
                        loop {
                            match self.peek_char() {
                                None => break, // unterminated comment, just stop
                                Some('*') => {
                                    self.advance();
                                    if self.peek_char() == Some('/') {
                                        self.advance();
                                        break;
                                    }
                                }
                                Some(_) => {
                                    self.advance();
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
                // HTML comment markers <!-- -->
                Some('<') if self.peek_char_at_offset(1) == Some('!')
                    && self.peek_char_at_offset(2) == Some('-')
                    && self.peek_char_at_offset(3) == Some('-')
                => {
                    // Skip <!--
                    self.advance(); // <
                    self.advance(); // !
                    self.advance(); // -
                    self.advance(); // -
                    // Skip until -->
                    loop {
                        match self.peek_char() {
                            None => break,
                            Some('-') => {
                                self.advance();
                                if self.peek_char() == Some('-') {
                                    self.advance();
                                    if self.peek_char() == Some('>') {
                                        self.advance();
                                        break;
                                    }
                                }
                            }
                            Some(_) => {
                                self.advance();
                            }
                        }
                    }
                }
                // HTML comment markers -->
                Some('-') if self.peek_char_at_offset(1) == Some('-')
                    && self.peek_char_at_offset(2) == Some('>')
                => {
                    // Skip -->
                    self.advance();
                    self.advance();
                    self.advance();
                }
                _ => break,
            }
        }
    }

    // --- Operators ---

    fn scan_plus(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('+') => {
                self.advance();
                Ok(TokenType::Inc)
            }
            Some('=') => {
                self.advance();
                Ok(TokenType::PlusAssign)
            }
            _ => Ok(TokenType::Plus),
        }
    }

    fn scan_minus(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('-') => {
                self.advance();
                Ok(TokenType::Dec)
            }
            Some('=') => {
                self.advance();
                Ok(TokenType::MinusAssign)
            }
            _ => Ok(TokenType::Minus),
        }
    }

    fn scan_star(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                Ok(TokenType::StarAssign)
            }
            _ => Ok(TokenType::Star),
        }
    }

    fn scan_percent(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                Ok(TokenType::PercentAssign)
            }
            _ => Ok(TokenType::Percent),
        }
    }

    fn scan_ampersand(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('&') => {
                self.advance();
                Ok(TokenType::And)
            }
            Some('=') => {
                self.advance();
                Ok(TokenType::BitAndAssign)
            }
            _ => Ok(TokenType::BitAnd),
        }
    }

    fn scan_pipe(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('|') => {
                self.advance();
                Ok(TokenType::Or)
            }
            Some('=') => {
                self.advance();
                Ok(TokenType::BitOrAssign)
            }
            _ => Ok(TokenType::BitOr),
        }
    }

    fn scan_caret(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                Ok(TokenType::BitXorAssign)
            }
            _ => Ok(TokenType::BitXor),
        }
    }

    fn scan_bang(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                if self.peek_char() == Some('=') {
                    self.advance();
                    Ok(TokenType::StrictNe)
                } else {
                    Ok(TokenType::Ne)
                }
            }
            _ => Ok(TokenType::Not),
        }
    }

    fn scan_equal(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                if self.peek_char() == Some('=') {
                    self.advance();
                    Ok(TokenType::StrictEq)
                } else {
                    Ok(TokenType::Eq)
                }
            }
            Some('>') => {
                self.advance();
                Ok(TokenType::Arrow)
            }
            _ => Ok(TokenType::Assign),
        }
    }

    fn scan_less(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                Ok(TokenType::Le)
            }
            Some('<') => {
                self.advance();
                match self.peek_char() {
                    Some('=') => {
                        self.advance();
                        Ok(TokenType::ShlAssign)
                    }
                    _ => Ok(TokenType::Shl),
                }
            }
            _ => Ok(TokenType::Lt),
        }
    }

    fn scan_greater(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('=') => {
                self.advance();
                Ok(TokenType::Ge)
            }
            Some('>') => {
                self.advance();
                match self.peek_char() {
                    Some('=') => {
                        self.advance();
                        Ok(TokenType::UShrAssign)
                    }
                    Some('>') => {
                        self.advance();
                        Ok(TokenType::UShr)
                    }
                    _ => Ok(TokenType::Shr),
                }
            }
            _ => Ok(TokenType::Gt),
        }
    }

    fn scan_question(&mut self) -> Result<TokenType, LexerError> {
        self.advance();
        match self.peek_char() {
            Some('?') => {
                self.advance();
                Ok(TokenType::NullishCoalescing)
            }
            Some('.') => {
                // Check if next is digit (ternary with float) or not (optional chaining)
                match self.peek_char_at_offset(1) {
                    Some(c) if c.is_ascii_digit() => Ok(TokenType::Question),
                    _ => {
                        self.advance();
                        Ok(TokenType::OptionalChaining)
                    }
                }
            }
            _ => Ok(TokenType::Question),
        }
    }

    // --- Numbers ---

    fn scan_number(&mut self) -> Result<TokenType, LexerError> {
        let mut value = 0.0_f64;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                value = value * 10.0 + ch.to_digit(10).unwrap() as f64;
                self.advance();
            } else if ch == '_' {
                // Numeric separator, skip it
                self.advance();
            } else {
                break;
            }
        }

        // Check for floating point
        if self.peek_char() == Some('.') && self.peek_char_at_offset(1).map_or(false, |c| c.is_ascii_digit()) {
            self.advance(); // skip .
            let mut fraction = 0.1;
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    value += fraction * ch.to_digit(10).unwrap() as f64;
                    fraction *= 0.1;
                    self.advance();
                } else if ch == '_' {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.scan_number_exponent(value)
    }

    fn scan_number_after_dot(&mut self) -> Result<TokenType, LexerError> {
        let mut value = 0.0_f64;
        let mut fraction = 0.1;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                value += fraction * ch.to_digit(10).unwrap() as f64;
                fraction *= 0.1;
                self.advance();
            } else {
                break;
            }
        }
        self.scan_number_exponent(value)
    }

    fn scan_number_rest(&mut self, mut value: f64) -> Result<TokenType, LexerError> {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                value = value * 10.0 + ch.to_digit(10).unwrap() as f64;
                self.advance();
            } else if ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        // Floating point
        if self.peek_char() == Some('.') && self.peek_char_at_offset(1).map_or(false, |c| c.is_ascii_digit()) {
            self.advance();
            let mut fraction = 0.1;
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    value += fraction * ch.to_digit(10).unwrap() as f64;
                    fraction *= 0.1;
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.scan_number_exponent(value)
    }

    fn scan_number_exponent(&mut self, mut value: f64) -> Result<TokenType, LexerError> {
        if self.peek_char() == Some('e') || self.peek_char() == Some('E') {
            self.advance();
            let positive = match self.peek_char() {
                Some('+') => { self.advance(); true }
                Some('-') => { self.advance(); false }
                _ => true,
            };
            let mut exp = 0.0_f64;
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    exp = exp * 10.0 + ch.to_digit(10).unwrap() as f64;
                    self.advance();
                } else {
                    break;
                }
            }
            if positive {
                value *= 10.0_f64.powf(exp);
            } else {
                value /= 10.0_f64.powf(exp);
            }
        }
        Ok(TokenType::Number(value))
    }

    fn scan_hex_number(&mut self) -> Result<TokenType, LexerError> {
        self.advance(); // skip x/X
        let mut value = 0.0_f64;
        while let Some(ch) = self.peek_char() {
            match ch {
                '0'..='9' => {
                    value = value * 16.0 + ch.to_digit(10).unwrap() as f64;
                    self.advance();
                }
                'a'..='f' => {
                    value = value * 16.0 + 10.0 + (ch as u32 - 'a' as u32) as f64;
                    self.advance();
                }
                'A'..='F' => {
                    value = value * 16.0 + 10.0 + (ch as u32 - 'A' as u32) as f64;
                    self.advance();
                }
                '_' => {
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(TokenType::Number(value))
    }

    fn scan_octal_number(&mut self) -> Result<TokenType, LexerError> {
        self.advance(); // skip o/O
        let mut value = 0.0_f64;
        while let Some(ch) = self.peek_char() {
            match ch {
                '0'..='7' => {
                    value = value * 8.0 + ch.to_digit(8).unwrap() as f64;
                    self.advance();
                }
                '_' => {
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(TokenType::Number(value))
    }

    fn scan_binary_number(&mut self) -> Result<TokenType, LexerError> {
        self.advance(); // skip b/B
        let mut value = 0.0_f64;
        while let Some(ch) = self.peek_char() {
            match ch {
                '0' => {
                    value *= 2.0;
                    self.advance();
                }
                '1' => {
                    value = value * 2.0 + 1.0;
                    self.advance();
                }
                '_' => {
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(TokenType::Number(value))
    }

    // --- Strings ---

    fn scan_string(&mut self, quote: char) -> Result<TokenType, LexerError> {
        self.advance(); // skip opening quote
        let mut s = String::new();

        loop {
            match self.peek_char() {
                None => {
                    return Err(LexerError::new(
                        "Unterminated string literal",
                        self.line,
                        self.column,
                    ));
                }
                Some(ch) if ch == quote => {
                    self.advance();
                    return Ok(TokenType::String(s));
                }
                Some('\n') => {
                    return Err(LexerError::new(
                        "Unterminated string literal",
                        self.line,
                        self.column,
                    ));
                }
                Some('\\') => {
                    self.advance();
                    let escaped = self.scan_escape_sequence()?;
                    s.push(escaped);
                }
                Some(ch) => {
                    self.advance();
                    s.push(ch);
                }
            }
        }
    }

    fn scan_escape_sequence(&mut self) -> Result<char, LexerError> {
        let ch = self.peek_char().ok_or_else(|| LexerError::new(
            "Unexpected end of escape sequence",
            self.line,
            self.column,
        ))?;
        self.advance();
        match ch {
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            'b' => Ok('\x08'),
            'f' => Ok('\x0C'),
            'v' => Ok('\x0B'),
            '\\' => Ok('\\'),
            '\'' => Ok('\''),
            '"' => Ok('"'),
            '0' => Ok('\0'),
            'x' => self.scan_hex_escape(),
            'u' => self.scan_unicode_escape(),
            '\n' => Ok('\n'), // line continuation
            '\r' => Ok('\r'),
            '\u{2028}' => Ok('\u{2028}'),
            '\u{2029}' => Ok('\u{2029}'),
            c if c.is_ascii_digit() => self.scan_octal_escape(c),
            _ => Ok(ch), // identity escape (non-strict mode)
        }
    }

    fn scan_hex_escape(&mut self) -> Result<char, LexerError> {
        let mut hex = 0u32;
        for _ in 0..2 {
            let ch = self.peek_char().ok_or_else(|| LexerError::new(
                "Unexpected end of hex escape sequence",
                self.line,
                self.column,
            ))?;
            match ch {
                '0'..='9' => {
                    hex = hex * 16 + ch.to_digit(10).unwrap();
                    self.advance();
                }
                'a'..='f' => {
                    hex = hex * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                    self.advance();
                }
                'A'..='F' => {
                    hex = hex * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                    self.advance();
                }
                _ => return Err(LexerError::new(
                    "Invalid hex digit in escape sequence",
                    self.line,
                    self.column,
                )),
            }
        }
        char::from_u32(hex).ok_or_else(|| LexerError::new(
            "Invalid Unicode code point in hex escape",
            self.line,
            self.column,
        ))
    }

    fn scan_unicode_escape(&mut self) -> Result<char, LexerError> {
        if self.peek_char() == Some('{') {
            self.advance(); // skip {
            let mut code = 0u32;
            let mut count = 0;
            loop {
                let ch = self.peek_char().ok_or_else(|| LexerError::new(
                    "Unexpected end of unicode escape sequence",
                    self.line,
                    self.column,
                ))?;
                if ch == '}' {
                    self.advance();
                    break;
                }
                match ch {
                    '0'..='9' => {
                        code = code * 16 + ch.to_digit(10).unwrap();
                        self.advance();
                    }
                    'a'..='f' => {
                        code = code * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                        self.advance();
                    }
                    'A'..='F' => {
                        code = code * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                        self.advance();
                    }
                    _ => return Err(LexerError::new(
                        "Invalid digit in unicode escape sequence",
                        self.line,
                        self.column,
                    )),
                }
                count += 1;
                if count > 6 {
                    return Err(LexerError::new(
                        "Unicode escape sequence too long",
                        self.line,
                        self.column,
                    ));
                }
            }
            char::from_u32(code).ok_or_else(|| LexerError::new(
                "Invalid Unicode code point",
                self.line,
                self.column,
            ))
        } else {
            let mut hex = 0u32;
            for _ in 0..4 {
                let ch = self.peek_char().ok_or_else(|| LexerError::new(
                    "Unexpected end of unicode escape sequence",
                    self.line,
                    self.column,
                ))?;
                match ch {
                    '0'..='9' => {
                        hex = hex * 16 + ch.to_digit(10).unwrap();
                        self.advance();
                    }
                    'a'..='f' => {
                        hex = hex * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                        self.advance();
                    }
                    'A'..='F' => {
                        hex = hex * 16 + 10 + ch.to_digit(16).unwrap() - 10;
                        self.advance();
                    }
                    _ => return Err(LexerError::new(
                        "Invalid digit in unicode escape sequence",
                        self.line,
                        self.column,
                    )),
                }
            }
            char::from_u32(hex).ok_or_else(|| LexerError::new(
                "Invalid Unicode code point",
                self.line,
                self.column,
            ))
        }
    }

    fn scan_octal_escape(&mut self, first: char) -> Result<char, LexerError> {
        let mut code = first.to_digit(8).unwrap();
        // Lookahead for up to 2 more octal digits
        for _ in 0..2 {
            match self.peek_char() {
                Some(c @ '0'..='7') => {
                    let new_code = code * 8 + c.to_digit(8).unwrap();
                    if new_code > 255 {
                        break;
                    }
                    code = new_code;
                    self.advance();
                }
                _ => break,
            }
        }
        char::from_u32(code).ok_or_else(|| LexerError::new(
            "Invalid character in octal escape",
            self.line,
            self.column,
        ))
    }

    // --- Template literals ---

    fn scan_template_literal(&mut self) -> Result<TokenType, LexerError> {
        self.advance(); // skip opening `
        let mut parts = Vec::new();

        loop {
            let mut raw = String::new();
            let mut cooked = String::new();

            loop {
                match self.peek_char() {
                    None => {
                        return Err(LexerError::new(
                            "Unterminated template literal",
                            self.line,
                            self.column,
                        ));
                    }
                    Some('`') => {
                        self.advance();
                        parts.push(TemplateToken::StringPart { raw, cooked, tail: true });
                        return Ok(TokenType::TemplateLiteral(parts));
                    }
                    Some('$') => {
                        self.advance();
                        if self.peek_char() == Some('{') {
                            self.advance(); // skip {
                            parts.push(TemplateToken::StringPart { raw: raw.clone(), cooked: cooked.clone(), tail: false });
                            let expr_tokens = self.scan_template_expression()?;
                            parts.push(TemplateToken::Expression(expr_tokens));
                            // Reset raw/cooked for the next string part
                            raw.clear();
                            cooked.clear();
                        } else {
                            raw.push('$');
                            cooked.push('$');
                        }
                    }
                    Some('\\') => {
                        self.advance();
                        let escaped = self.scan_escape_sequence()?;
                        raw.push('\\');
                        cooked.push(escaped);
                    }
                    Some('\n') => {
                        self.advance();
                        raw.push('\n');
                        cooked.push('\n');
                    }
                    Some(ch) => {
                        self.advance();
                        raw.push(ch);
                        cooked.push(ch);
                    }
                }
            }
        }
    }

    fn scan_template_expression(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut depth = 1;
        let mut tokens = Vec::new();

        loop {
            let token = self.scan_token()?;
            match &token.token_type {
                TokenType::LBrace => depth += 1,
                TokenType::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                TokenType::Eof => {
                    return Err(LexerError::new(
                        "Unterminated template expression",
                        self.line,
                        self.column,
                    ));
                }
                _ => {}
            }
            tokens.push(token);
        }

        Ok(tokens)
    }

    // --- Regex ---

    fn scan_regex_or_division(&mut self) -> Result<TokenType, LexerError> {
        // Determine if / starts a regex or is a division operator.
        // Regex is expected after: operators, keywords, lparen, lbrace, lbracket,
        // semicolon, comma, colon, begin of file, etc.
        // Division is expected after: identifiers, numbers, strings, rparen,
        // rbracket, rbrace, ++, --, this, etc.
        if self.prev_is_value {
            // After a value-like token, / is division
            self.advance(); // skip /
            Ok(TokenType::Slash)
        } else {
            self.scan_regex()
        }
    }

    /// Scan a regex literal. Must be called when we know it's a regex.
    fn scan_regex(&mut self) -> Result<TokenType, LexerError> {
        self.advance(); // skip /
        let mut pattern = String::new();
        let mut in_char_class = false;

        loop {
            match self.peek_char() {
                None => {
                    return Err(LexerError::new(
                        "Unterminated regular expression literal",
                        self.line,
                        self.column,
                    ));
                }
                Some('\n') => {
                    return Err(LexerError::new(
                        "Unterminated regular expression literal",
                        self.line,
                        self.column,
                    ));
                }
                Some('\\') => {
                    self.advance();
                    pattern.push('\\');
                    if let Some(ch) = self.peek_char() {
                        self.advance();
                        pattern.push(ch);
                    }
                }
                Some('[') => {
                    in_char_class = true;
                    self.advance();
                    pattern.push('[');
                }
                Some(']') => {
                    in_char_class = false;
                    self.advance();
                    pattern.push(']');
                }
                Some('/') if !in_char_class => {
                    self.advance();
                    break;
                }
                Some(ch) => {
                    self.advance();
                    pattern.push(ch);
                }
            }
        }

        // Scan flags
        let mut flags = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphabetic() && !ch.is_whitespace() {
                flags.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        Ok(TokenType::RegExp { pattern, flags })
    }

    // --- Identifiers and keywords ---

    fn scan_identifier_or_keyword(&mut self) -> Result<TokenType, LexerError> {
        let mut ident = String::new();

        // First character (already validated as identifier start)
        if let Some(ch) = self.peek_char() {
            self.advance();
            ident.push(ch);
        }

        // Rest of identifier (ASCII fast path + Unicode fallback)
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii() {
                if is_identifier_part(ch) {
                    self.advance();
                    ident.push(ch);
                } else {
                    break;
                }
            } else if is_unicode_identifier_part(ch) {
                self.advance();
                ident.push(ch);
            } else {
                break;
            }
        }

        // Check if it's a keyword
        match ident.as_str() {
            "break" => Ok(TokenType::Keyword(Keyword::Break)),
            "case" => Ok(TokenType::Keyword(Keyword::Case)),
            "catch" => Ok(TokenType::Keyword(Keyword::Catch)),
            "class" => Ok(TokenType::Keyword(Keyword::Class)),
            "const" => Ok(TokenType::Keyword(Keyword::Const)),
            "continue" => Ok(TokenType::Keyword(Keyword::Continue)),
            "debugger" => Ok(TokenType::Keyword(Keyword::Debugger)),
            "default" => Ok(TokenType::Keyword(Keyword::Default)),
            "delete" => Ok(TokenType::Keyword(Keyword::Delete)),
            "do" => Ok(TokenType::Keyword(Keyword::Do)),
            "else" => Ok(TokenType::Keyword(Keyword::Else)),
            "export" => Ok(TokenType::Keyword(Keyword::Export)),
            "extends" => Ok(TokenType::Keyword(Keyword::Extends)),
            "finally" => Ok(TokenType::Keyword(Keyword::Finally)),
            "for" => Ok(TokenType::Keyword(Keyword::For)),
            "function" => Ok(TokenType::Keyword(Keyword::Function)),
            "if" => Ok(TokenType::Keyword(Keyword::If)),
            "import" => Ok(TokenType::Keyword(Keyword::Import)),
            "in" => Ok(TokenType::Keyword(Keyword::In)),
            "instanceof" => Ok(TokenType::Keyword(Keyword::Instanceof)),
            "let" => Ok(TokenType::Keyword(Keyword::Let)),
            "new" => Ok(TokenType::Keyword(Keyword::New)),
            "return" => Ok(TokenType::Keyword(Keyword::Return)),
            "super" => Ok(TokenType::Keyword(Keyword::Super)),
            "switch" => Ok(TokenType::Keyword(Keyword::Switch)),
            "this" => Ok(TokenType::Keyword(Keyword::This)),
            "throw" => Ok(TokenType::Keyword(Keyword::Throw)),
            "try" => Ok(TokenType::Keyword(Keyword::Try)),
            "typeof" => Ok(TokenType::Keyword(Keyword::Typeof)),
            "var" => Ok(TokenType::Keyword(Keyword::Var)),
            "void" => Ok(TokenType::Keyword(Keyword::Void)),
            "while" => Ok(TokenType::Keyword(Keyword::While)),
            "with" => Ok(TokenType::Keyword(Keyword::With)),
            "yield" => Ok(TokenType::Keyword(Keyword::Yield)),
            "async" => Ok(TokenType::Keyword(Keyword::Async)),
            "await" => Ok(TokenType::Keyword(Keyword::Await)),
            "of" => Ok(TokenType::Keyword(Keyword::Of)),
            "true" => Ok(TokenType::Bool(true)),
            "false" => Ok(TokenType::Bool(false)),
            "null" => Ok(TokenType::Null),
            "undefined" => Ok(TokenType::Undefined),
            _ => Ok(TokenType::Identifier(ident)),
        }
    }
}

// --- Character classification ---

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
}

fn is_identifier_part(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn is_unicode_identifier_start(ch: char) -> bool {
    // Unicode letter categories: Lu, Ll, Lt, Lm, Lo, Nl
    ch.is_alphabetic()
}

fn is_unicode_identifier_part(ch: char) -> bool {
    // Unicode letter, connector punctuation, or combining mark
    ch.is_alphanumeric() || ch == '_' || ch.is_alphabetic() || is_combining_mark(ch)
}

/// Check if a character is a Unicode combining mark.
/// Common ranges: U+0300-U+036F, U+1DC0-U+1DFF, U+20D0-U+20FF, U+FE20-U+FE2F
fn is_combining_mark(ch: char) -> bool {
    let cp = ch as u32;
    matches!(cp,
        0x0300..=0x036F |   // Combining Diacritical Marks
        0x1DC0..=0x1DFF |   // Combining Diacritical Marks Supplement
        0x20D0..=0x20FF |   // Combining Diacritical Marks for Symbols
        0xFE20..=0xFE2F     // Combining Half Marks
    )
}

/// Determine if a token type represents a "value" (right-hand side expression).
/// Used for regex vs division disambiguation.
fn is_value_token(tt: &TokenType) -> bool {
    match tt {
        // Literals are values
        TokenType::Number(_) | TokenType::String(_) | TokenType::Bool(_)
        | TokenType::Null | TokenType::Undefined | TokenType::RegExp { .. } => true,
        // Identifiers are values
        TokenType::Identifier(_) => true,
        // Template literals are values
        TokenType::TemplateLiteral(_) => true,
        // ) ] } are values (end of expression/grouping)
        TokenType::RParen | TokenType::RBracket | TokenType::RBrace => true,
        // ++ and -- are value-like (postfix)
        TokenType::Inc | TokenType::Dec => true,
        // this is a keyword but acts as a value
        TokenType::Keyword(Keyword::This) => true,
        // typeof, void, delete are unary operators that produce values,
        // but after them we expect an operand, so / would be regex.
        // Keywords like function, class, etc. are not values in this context.
        _ => false,
    }
}

// Allow unused on the unused TokenType variants
#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let token = lexer.next_token().unwrap();
            let is_eof = matches!(token.token_type, TokenType::Eof);
            if is_eof {
                break;
            }
            tokens.push(token);
        }
        tokens
    }

    fn first_token(source: &str) -> TokenType {
        let mut lexer = Lexer::new(source);
        lexer.next_token().unwrap().token_type
    }

    #[test]
    fn test_empty_source() {
        let tokens = tokenize("");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_single_line_comment() {
        let tokens = tokenize("// this is a comment");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_block_comment() {
        let tokens = tokenize("/* block comment */");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_single_line_comment_not_eating_next_line() {
        let tokens = tokenize("// comment\n42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Number(42.0));
    }

    #[test]
    fn test_block_comment_not_eating_next_token() {
        let tokens = tokenize("/* comment */42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Number(42.0));
    }

    #[test]
    fn test_integer_literal() {
        assert_eq!(first_token("42"), TokenType::Number(42.0));
        assert_eq!(first_token("0"), TokenType::Number(0.0));
        assert_eq!(first_token("1234567890"), TokenType::Number(1234567890.0));
    }

    #[test]
    fn test_float_literal() {
        assert_eq!(first_token("3.14"), TokenType::Number(3.14));
        assert_eq!(first_token(".5"), TokenType::Number(0.5));
        assert_eq!(first_token("1.0"), TokenType::Number(1.0));
    }

    #[test]
    fn test_hex_literal() {
        assert_eq!(first_token("0xFF"), TokenType::Number(255.0));
        assert_eq!(first_token("0X0"), TokenType::Number(0.0));
        assert_eq!(first_token("0x1a"), TokenType::Number(26.0));
    }

    #[test]
    fn test_octal_literal() {
        assert_eq!(first_token("0o10"), TokenType::Number(8.0));
        assert_eq!(first_token("0O77"), TokenType::Number(63.0));
    }

    #[test]
    fn test_binary_literal() {
        assert_eq!(first_token("0b1010"), TokenType::Number(10.0));
        assert_eq!(first_token("0B1"), TokenType::Number(1.0));
    }

    #[test]
    fn test_exponent_literal() {
        assert_eq!(first_token("1e2"), TokenType::Number(100.0));
        assert_eq!(first_token("1.5e2"), TokenType::Number(150.0));
        assert_eq!(first_token("1e-2"), TokenType::Number(0.01));
        assert_eq!(first_token("1.5e+2"), TokenType::Number(150.0));
    }

    #[test]
    fn test_string_double_quotes() {
        assert_eq!(
            first_token("\"hello world\""),
            TokenType::String("hello world".to_string())
        );
    }

    #[test]
    fn test_string_single_quotes() {
        assert_eq!(
            first_token("'hello world'"),
            TokenType::String("hello world".to_string())
        );
    }

    #[test]
    fn test_string_escape_sequences() {
        assert_eq!(
            first_token("\"hello\\nworld\""),
            TokenType::String("hello\nworld".to_string())
        );
        assert_eq!(
            first_token("'\\t\\r'"),
            TokenType::String("\t\r".to_string())
        );
        assert_eq!(
            first_token("'\\\\'"),
            TokenType::String("\\".to_string())
        );
    }

    #[test]
    fn test_string_hex_escape() {
        assert_eq!(
            first_token("\"\\x41\""),
            TokenType::String("A".to_string())
        );
    }

    #[test]
    fn test_string_unicode_escape() {
        assert_eq!(
            first_token("\"\\u0041\""),
            TokenType::String("A".to_string())
        );
        assert_eq!(
            first_token("\"\\u{1F600}\""),
            TokenType::String("\u{1F600}".to_string())
        );
    }

    #[test]
    fn test_string_unterminated() {
        let result = Lexer::new("\"hello").next_token();
        assert!(result.is_err());
    }

    #[test]
    fn test_string_newline_terminates() {
        let result = Lexer::new("\"hello\nworld\"").next_token();
        assert!(result.is_err());
    }

    #[test]
    fn test_bool_true() {
        assert_eq!(first_token("true"), TokenType::Bool(true));
    }

    #[test]
    fn test_bool_false() {
        assert_eq!(first_token("false"), TokenType::Bool(false));
    }

    #[test]
    fn test_null() {
        assert_eq!(first_token("null"), TokenType::Null);
    }

    #[test]
    fn test_undefined() {
        assert_eq!(first_token("undefined"), TokenType::Undefined);
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(first_token("foo"), TokenType::Identifier("foo".to_string()));
        assert_eq!(first_token("_bar"), TokenType::Identifier("_bar".to_string()));
        assert_eq!(first_token("$baz"), TokenType::Identifier("$baz".to_string()));
        assert_eq!(first_token("foo123"), TokenType::Identifier("foo123".to_string()));
    }

    #[test]
    fn test_keywords() {
        assert_eq!(first_token("if"), TokenType::Keyword(Keyword::If));
        assert_eq!(first_token("else"), TokenType::Keyword(Keyword::Else));
        assert_eq!(first_token("for"), TokenType::Keyword(Keyword::For));
        assert_eq!(first_token("while"), TokenType::Keyword(Keyword::While));
        assert_eq!(first_token("function"), TokenType::Keyword(Keyword::Function));
        assert_eq!(first_token("var"), TokenType::Keyword(Keyword::Var));
        assert_eq!(first_token("let"), TokenType::Keyword(Keyword::Let));
        assert_eq!(first_token("const"), TokenType::Keyword(Keyword::Const));
        assert_eq!(first_token("return"), TokenType::Keyword(Keyword::Return));
        assert_eq!(first_token("new"), TokenType::Keyword(Keyword::New));
        assert_eq!(first_token("this"), TokenType::Keyword(Keyword::This));
        assert_eq!(first_token("class"), TokenType::Keyword(Keyword::Class));
        assert_eq!(first_token("extends"), TokenType::Keyword(Keyword::Extends));
        assert_eq!(first_token("super"), TokenType::Keyword(Keyword::Super));
        assert_eq!(first_token("import"), TokenType::Keyword(Keyword::Import));
        assert_eq!(first_token("export"), TokenType::Keyword(Keyword::Export));
        assert_eq!(first_token("default"), TokenType::Keyword(Keyword::Default));
        assert_eq!(first_token("try"), TokenType::Keyword(Keyword::Try));
        assert_eq!(first_token("catch"), TokenType::Keyword(Keyword::Catch));
        assert_eq!(first_token("finally"), TokenType::Keyword(Keyword::Finally));
        assert_eq!(first_token("throw"), TokenType::Keyword(Keyword::Throw));
        assert_eq!(first_token("typeof"), TokenType::Keyword(Keyword::Typeof));
        assert_eq!(first_token("void"), TokenType::Keyword(Keyword::Void));
        assert_eq!(first_token("delete"), TokenType::Keyword(Keyword::Delete));
        assert_eq!(first_token("instanceof"), TokenType::Keyword(Keyword::Instanceof));
        assert_eq!(first_token("in"), TokenType::Keyword(Keyword::In));
        assert_eq!(first_token("do"), TokenType::Keyword(Keyword::Do));
        assert_eq!(first_token("switch"), TokenType::Keyword(Keyword::Switch));
        assert_eq!(first_token("case"), TokenType::Keyword(Keyword::Case));
        assert_eq!(first_token("break"), TokenType::Keyword(Keyword::Break));
        assert_eq!(first_token("continue"), TokenType::Keyword(Keyword::Continue));
        assert_eq!(first_token("with"), TokenType::Keyword(Keyword::With));
        assert_eq!(first_token("debugger"), TokenType::Keyword(Keyword::Debugger));
        assert_eq!(first_token("yield"), TokenType::Keyword(Keyword::Yield));
        assert_eq!(first_token("async"), TokenType::Keyword(Keyword::Async));
        assert_eq!(first_token("await"), TokenType::Keyword(Keyword::Await));
        assert_eq!(first_token("of"), TokenType::Keyword(Keyword::Of));
    }

    #[test]
    fn test_single_char_operators() {
        assert_eq!(first_token("+"), TokenType::Plus);
        assert_eq!(first_token("-"), TokenType::Minus);
        assert_eq!(first_token("*"), TokenType::Star);
        // / at start of source is regex, not division (needs context to disambiguate)
        assert_eq!(first_token("%"), TokenType::Percent);
        assert_eq!(first_token("="), TokenType::Assign);
        assert_eq!(first_token("!"), TokenType::Not);
        assert_eq!(first_token("~"), TokenType::BitNot);
        assert_eq!(first_token("&"), TokenType::BitAnd);
        assert_eq!(first_token("|"), TokenType::BitOr);
        assert_eq!(first_token("^"), TokenType::BitXor);
    }

    #[test]
    fn test_compound_operators() {
        assert_eq!(first_token("++"), TokenType::Inc);
        assert_eq!(first_token("--"), TokenType::Dec);
        assert_eq!(first_token("+="), TokenType::PlusAssign);
        assert_eq!(first_token("-="), TokenType::MinusAssign);
        assert_eq!(first_token("*="), TokenType::StarAssign);
        assert_eq!(first_token("=="), TokenType::Eq);
        assert_eq!(first_token("!="), TokenType::Ne);
        assert_eq!(first_token("==="), TokenType::StrictEq);
        assert_eq!(first_token("!=="), TokenType::StrictNe);
        assert_eq!(first_token("<"), TokenType::Lt);
        assert_eq!(first_token(">"), TokenType::Gt);
        assert_eq!(first_token("<="), TokenType::Le);
        assert_eq!(first_token(">="), TokenType::Ge);
        assert_eq!(first_token("&&"), TokenType::And);
        assert_eq!(first_token("||"), TokenType::Or);
        assert_eq!(first_token("<<"), TokenType::Shl);
        assert_eq!(first_token(">>"), TokenType::Shr);
        assert_eq!(first_token(">>>"), TokenType::UShr);
        assert_eq!(first_token("=>"), TokenType::Arrow);
        assert_eq!(first_token("??"), TokenType::NullishCoalescing);
        assert_eq!(first_token("..."), TokenType::Ellipsis);
    }

    #[test]
    fn test_question_dot_operator() {
        // ?. should be optional chaining
        assert_eq!(first_token("?."), TokenType::OptionalChaining);
    }

    #[test]
    fn test_punctuation() {
        assert_eq!(first_token("("), TokenType::LParen);
        assert_eq!(first_token(")"), TokenType::RParen);
        assert_eq!(first_token("{"), TokenType::LBrace);
        assert_eq!(first_token("}"), TokenType::RBrace);
        assert_eq!(first_token("["), TokenType::LBracket);
        assert_eq!(first_token("]"), TokenType::RBracket);
        assert_eq!(first_token(";"), TokenType::Semicolon);
        assert_eq!(first_token(","), TokenType::Comma);
        assert_eq!(first_token("."), TokenType::Dot);
        assert_eq!(first_token(":"), TokenType::Colon);
        assert_eq!(first_token("?"), TokenType::Question);
    }

    #[test]
    fn test_line_tracking() {
        let mut lexer = Lexer::new("42\n7\n3");
        let t1 = lexer.next_token().unwrap();
        assert_eq!(t1.line, 1);
        assert_eq!(t1.column, 1);
        let t2 = lexer.next_token().unwrap();
        assert_eq!(t2.line, 2);
        assert_eq!(t2.column, 1);
        let t3 = lexer.next_token().unwrap();
        assert_eq!(t3.line, 3);
        assert_eq!(t3.column, 1);
    }

    #[test]
    fn test_column_tracking() {
        let mut lexer = Lexer::new("  42");
        let t = lexer.next_token().unwrap();
        assert_eq!(t.line, 1);
        assert_eq!(t.column, 3);
    }

    #[test]
    fn test_multiple_tokens() {
        let tokens = tokenize("var x = 42;");
        assert_eq!(tokens.len(), 5); // var, x, =, 42, ;
        assert_eq!(tokens[0].token_type, TokenType::Keyword(Keyword::Var));
        assert_eq!(tokens[1].token_type, TokenType::Identifier("x".to_string()));
        assert_eq!(tokens[2].token_type, TokenType::Assign);
        assert_eq!(tokens[3].token_type, TokenType::Number(42.0));
        assert_eq!(tokens[4].token_type, TokenType::Semicolon);
    }

    #[test]
    fn test_function_declaration() {
        let tokens = tokenize("function foo(a, b) { return a + b; }");
        assert_eq!(tokens[0].token_type, TokenType::Keyword(Keyword::Function));
        assert_eq!(tokens[1].token_type, TokenType::Identifier("foo".to_string()));
        assert_eq!(tokens[2].token_type, TokenType::LParen);
        assert_eq!(tokens[3].token_type, TokenType::Identifier("a".to_string()));
        assert_eq!(tokens[4].token_type, TokenType::Comma);
        assert_eq!(tokens[5].token_type, TokenType::Identifier("b".to_string()));
        assert_eq!(tokens[6].token_type, TokenType::RParen);
        assert_eq!(tokens[7].token_type, TokenType::LBrace);
        assert_eq!(tokens[8].token_type, TokenType::Keyword(Keyword::Return));
        assert_eq!(tokens[9].token_type, TokenType::Identifier("a".to_string()));
        assert_eq!(tokens[10].token_type, TokenType::Plus);
        assert_eq!(tokens[11].token_type, TokenType::Identifier("b".to_string()));
        assert_eq!(tokens[12].token_type, TokenType::Semicolon);
        assert_eq!(tokens[13].token_type, TokenType::RBrace);
    }

    #[test]
    fn test_nested_comments() {
        let tokens = tokenize("/* outer /* inner */ 42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Number(42.0));
    }

    #[test]
    fn test_unterminated_block_comment() {
        let tokens = tokenize("/* unterminated comment");
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_unexpected_character() {
        let result = Lexer::new("@").next_token();
        assert!(result.is_err());
    }

    #[test]
    fn test_regex_literal() {
        let tokens = tokenize("/pattern/gi");
        assert_eq!(
            tokens[0].token_type,
            TokenType::RegExp { pattern: "pattern".to_string(), flags: "gi".to_string() }
        );
    }

    #[test]
    fn test_regex_with_escape() {
        let tokens = tokenize("/foo\\/bar/");
        assert_eq!(
            tokens[0].token_type,
            TokenType::RegExp { pattern: "foo\\/bar".to_string(), flags: String::new() }
        );
    }

    #[test]
    fn test_regex_with_char_class() {
        let tokens = tokenize("/[a-z]+/g");
        assert_eq!(
            tokens[0].token_type,
            TokenType::RegExp { pattern: "[a-z]+".to_string(), flags: "g".to_string() }
        );
    }

    #[test]
    fn test_number_with_underscores() {
        assert_eq!(first_token("1_000"), TokenType::Number(1000.0));
        assert_eq!(first_token("0xFF_FF"), TokenType::Number(65535.0));
    }

    #[test]
    fn test_unicode_identifier() {
        // Unicode letters should be valid identifier starts
        assert_eq!(
            first_token("cafe\u{0301}"),
            TokenType::Identifier("cafe\u{0301}".to_string())
        );
    }

    #[test]
    fn test_template_literal_simple() {
        let tokens = tokenize("`hello`");
        assert!(matches!(
            tokens[0].token_type,
            TokenType::TemplateLiteral(_)
        ));
    }

    #[test]
    fn test_template_literal_with_expression() {
        let tokens = tokenize("`hello ${name}`");
        // Should have template literal tokens and an expression
        assert!(!tokens.is_empty());
        // First token should be template literal (with open expression)
        assert!(matches!(
            tokens[0].token_type,
            TokenType::TemplateLiteral(_)
        ));
    }

    #[test]
    fn test_invalid_escape_string() {
        // Unknown escape sequences in non-strict mode are identity
        let tokens = tokenize("\"\\a\"");
        assert_eq!(tokens[0].token_type, TokenType::String("a".to_string()));
    }

    #[test]
    fn test_char_after_division_is_regex() {
        // x/abc/g should be x / abc / g (all division, since x is a value token)
        let tokens = tokenize("x/abc/g");
        assert_eq!(tokens.len(), 5); // x, /, abc, /, g
    }

    #[test]
    fn test_offset_tracking() {
        let mut lexer = Lexer::new("  hello");
        let t = lexer.next_token().unwrap();
        assert_eq!(t.start, 2);
        assert_eq!(t.end, 7);
    }

    #[test]
    fn test_semicolon_as_regex_stop() {
        // /a/; should work
        let tokens = tokenize("/a/; x");
        assert_eq!(
            tokens[0].token_type,
            TokenType::RegExp { pattern: "a".to_string(), flags: String::new() }
        );
        assert_eq!(tokens[1].token_type, TokenType::Semicolon);
        assert_eq!(tokens[2].token_type, TokenType::Identifier("x".to_string()));
    }

    #[test]
    fn test_peek_token() {
        let mut lexer = Lexer::new("1 2");
        let peeked = lexer.peek_token().unwrap().clone();
        assert_eq!(peeked.token_type, TokenType::Number(1.0));
        // Peek again should return the same
        let peeked2 = lexer.peek_token().unwrap().clone();
        assert_eq!(peeked2.token_type, TokenType::Number(1.0));
        // Next should also return 1 (peek doesn't consume)
        let next = lexer.next_token().unwrap();
        assert_eq!(next.token_type, TokenType::Number(1.0));
        // Now the next should be 2
        let next2 = lexer.next_token().unwrap();
        assert_eq!(next2.token_type, TokenType::Number(2.0));
    }

    #[test]
    fn test_all_operators_concatenated() {
        // Note: / is excluded because it's ambiguous (regex vs division)
        // without parser context. Testing it separately.
        let tokens = tokenize("+-*%===!==<><=>=&&||!&|^~<<>>>=++--=><>...");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_numeric_separator_in_float() {
        assert_eq!(first_token("1_000.5_0"), TokenType::Number(1000.5));
    }

    #[test]
    fn test_exponent_with_separator() {
        assert_eq!(first_token("1_0e2"), TokenType::Number(1000.0));
    }

    #[test]
    fn test_string_with_line_continuation() {
        assert_eq!(
            first_token("\"hello\\\nworld\""),
            TokenType::String("hello\nworld".to_string())
        );
    }

    #[test]
    fn test_octal_escape_in_string() {
        assert_eq!(
            first_token("'\\101'"),
            TokenType::String("A".to_string())
        );
    }

    #[test]
    fn test_error_display() {
        let err = LexerError::new("test error", 10, 20);
        assert_eq!(format!("{}", err), "LexerError: test error (line 10, column 20)");
    }

    #[test]
    fn test_nested_block_comments() {
        // JS block comments don't nest, but our lexer should handle them gracefully
        let tokens = tokenize("/* /* */ 42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Number(42.0));
    }

    #[test]
    fn test_complex_expression() {
        let tokens = tokenize("if (x >= 10 && y !== null) { return true; }");
        assert_eq!(tokens[0].token_type, TokenType::Keyword(Keyword::If));
        assert_eq!(tokens[1].token_type, TokenType::LParen);
        assert_eq!(tokens[2].token_type, TokenType::Identifier("x".to_string()));
        assert_eq!(tokens[3].token_type, TokenType::Ge);
        assert_eq!(tokens[4].token_type, TokenType::Number(10.0));
        assert_eq!(tokens[5].token_type, TokenType::And);
        assert_eq!(tokens[6].token_type, TokenType::Identifier("y".to_string()));
        assert_eq!(tokens[7].token_type, TokenType::StrictNe);
        assert_eq!(tokens[8].token_type, TokenType::Null);
        assert_eq!(tokens[9].token_type, TokenType::RParen);
        assert_eq!(tokens[10].token_type, TokenType::LBrace);
        assert_eq!(tokens[11].token_type, TokenType::Keyword(Keyword::Return));
        assert_eq!(tokens[12].token_type, TokenType::Bool(true));
        assert_eq!(tokens[13].token_type, TokenType::Semicolon);
        assert_eq!(tokens[14].token_type, TokenType::RBrace);
    }
}
