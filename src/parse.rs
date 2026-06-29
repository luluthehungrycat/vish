//! Command-line parser — tokenization, quoting, pipes, redirections.
//!
//! Grammar (Phase 2):
//!   pipeline  := command ('|' command)*
//!   command   := word (redirect)* (word (redirect)*)*
//!   redirect  := ('>' | '>>' | '<' | '2>' | '2>>' | '2>&1') word
//!   word      := unquoted | single_quoted | double_quoted
//!
//! Single-quoted: literal, no escapes.
//! Double-quoted: escapes for \\\", \\$, \\`, \\\\, \\n, \\t, \\r.

use alloc::vec::Vec;
use alloc::string::String;

// ── Types ────────────────────────────────────────────────────────────────

/// Kind of I/O redirection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RedirectKind {
    /// `< file`
    Input,
    /// `> file`
    Output,
    /// `>> file`
    Append,
    /// `2> file`
    StderrOutput,
    /// `2>> file`
    StderrAppend,
    /// `2>&1`
    StderrMerge,
}

/// A single redirection directive.
#[derive(Debug, Clone)]
pub struct Redirect {
    pub kind: RedirectKind,
    pub target: String,
}

/// A single command with its arguments and redirections.
#[derive(Debug, Clone)]
pub struct SimpleCommand {
    pub argv: Vec<String>,
    pub redirects: Vec<Redirect>,
}

/// A pipeline of commands connected by pipes.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub commands: Vec<SimpleCommand>,
}

// ── Error type ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

// ── Tokenizer ────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Pipe,
    Redirect(RedirectKind),
    Newline,
    Eof,
}

struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    fn new(input: &str) -> Self {
        Tokenizer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        self.pos += 1;
        c
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Parse a word (unquoted, single-quoted, or double-quoted).
    fn read_word(&mut self) -> Result<String, ParseError> {
        let mut word = String::new();

        loop {
            match self.peek() {
                None | Some('|') | Some('>') | Some('<') | Some(' ') | Some('\t') => {
                    break;
                }
                Some(c) => match c {
                    '\'' => {
                        self.advance(); // consume opening '
                        loop {
                            match self.advance() {
                                None => {
                                    return Err(ParseError {
                                        message: "unterminated single quote".into(),
                                        offset: self.pos,
                                    });
                                }
                                Some('\'') => break,
                                Some(ch) => word.push(ch),
                            }
                        }
                    }
                    '"' => {
                        self.advance(); // consume opening "
                        loop {
                            match self.advance() {
                                None => {
                                    return Err(ParseError {
                                        message: "unterminated double quote".into(),
                                        offset: self.pos,
                                    });
                                }
                                Some('"') => break,
                                Some('\\') => {
                                    // Escape sequence inside double quotes
                                    match self.advance() {
                                        None => {
                                            return Err(ParseError {
                                                message:
                                                    "unterminated escape in double quote"
                                                        .into(),
                                                offset: self.pos,
                                            });
                                        }
                                        Some('"') => word.push('"'),
                                        Some('\\') => word.push('\\'),
                                        Some('$') => word.push('$'),
                                        Some('`') => word.push('`'),
                                        Some('n') => word.push('\n'),
                                        Some('t') => word.push('\t'),
                                        Some('r') => word.push('\r'),
                                        Some(ch) => {
                                            // POSIX: invalid escape → literal
                                            word.push('\\');
                                            word.push(ch);
                                        }
                                    }
                                }
                                Some(ch) => word.push(ch),
                            }
                        }
                    }
                    '\\' => {
                        // Outside quotes: backslash escapes next character
                        self.advance();
                        match self.advance() {
                            None => {
                                return Err(ParseError {
                                    message: "trailing backslash".into(),
                                    offset: self.pos,
                                });
                            }
                            Some(ch) => word.push(ch),
                        }
                    }
                    _ => {
                        self.advance();
                        word.push(c);
                    }
                },
            }
        }

        Ok(word)
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        match self.peek() {
            None => Ok(Token::Eof),
            Some(c) => match c {
                '|' => {
                    self.advance();
                    Ok(Token::Pipe)
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        Ok(Token::Redirect(RedirectKind::Append))
                    } else if self.peek() == Some('&') {
                        self.advance();
                        // Next token (the fd number, typically "1") is
                        // consumed by the parser as a word target.
                        Ok(Token::Redirect(RedirectKind::StderrMerge))
                    } else {
                        Ok(Token::Redirect(RedirectKind::Output))
                    }
                }
                '<' => {
                    self.advance();
                    Ok(Token::Redirect(RedirectKind::Input))
                }
                '2' => {
                    // Check for 2> or 2>>
                    if self.pos + 1 < self.chars.len() && self.chars[self.pos + 1] == '>' {
                        self.advance(); // consume '2'
                        self.advance(); // consume '>'
                        if self.peek() == Some('>') {
                            self.advance();
                            Ok(Token::Redirect(RedirectKind::StderrAppend))
                } else if self.peek() == Some('&') {
                        self.advance();
                        // Next token (fd number or filename) is
                        // consumed by the parser as a word target.
                        Ok(Token::Redirect(RedirectKind::StderrMerge))
                        } else {
                            Ok(Token::Redirect(RedirectKind::StderrOutput))
                        }
                    } else {
                        // Not a redirect, read as word
                        let word = self.read_word()?;
                        Ok(Token::Word(word))
                    }
                }
                '#' => {
                    // Comment — consume until end of line
                    while let Some(ch) = self.advance() {
                        if ch == '\n' {
                            break;
                        }
                    }
                    Ok(Token::Newline)
                }
                _ => {
                    let word = self.read_word()?;
                    Ok(Token::Word(word))
                }
            },
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────

/// Parse a command line into a pipeline of commands (with redirections).
pub fn parse_line(input: &str) -> Result<Pipeline, ParseError> {
    let mut tok = Tokenizer::new(input);
    let mut pipeline = Pipeline {
        commands: Vec::new(),
    };

    loop {
        let mut argv = Vec::new();
        let mut redirects = Vec::new();

        // Read tokens for one command. A command ends at pipe, newline, or EOF.
        let is_pipe = loop {
            match tok.next_token()? {
                Token::Word(w) => argv.push(w),
                Token::Redirect(kind) => {
                    let target = match tok.next_token()? {
                        Token::Word(t) => t,
                        _ => {
                            return Err(ParseError {
                                message: "expected filename after redirect".into(),
                                offset: tok.pos,
                            });
                        }
                    };
                    redirects.push(Redirect { kind, target });
                }
                Token::Pipe => {
                    if !argv.is_empty() || !redirects.is_empty() {
                        pipeline
                            .commands
                            .push(SimpleCommand { argv, redirects });
                    }
                    break true;
                }
                Token::Newline | Token::Eof => {
                    if !argv.is_empty() || !redirects.is_empty() {
                        pipeline
                            .commands
                            .push(SimpleCommand { argv, redirects });
                    }
                    break false;
                }
            }
        };

        // If this was a pipe, there might be more commands after.
        // If EOF/newline, we're done.
        if !is_pipe {
            break;
        }
    }

    // If nothing parsed, return an empty pipeline with one empty command
    if pipeline.commands.is_empty() {
        pipeline.commands.push(SimpleCommand {
            argv: Vec::new(),
            redirects: Vec::new(),
        });
    }

    Ok(pipeline)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let p = parse_line("").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert!(p.commands[0].argv.is_empty());
    }

    #[test]
    fn test_whitespace() {
        let p = parse_line("   ").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert!(p.commands[0].argv.is_empty());
    }

    #[test]
    fn test_simple() {
        let p = parse_line("echo hello world").unwrap();
        assert_eq!(p.commands.len(), 1);
        assert_eq!(p.commands[0].argv, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn test_single_quotes() {
        let p = parse_line("echo 'hello world'").unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_double_quotes() {
        let p = parse_line(r#"echo "hello world""#).unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_double_quote_escapes() {
        let p = parse_line(r#"echo "tab\there""#).unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "tab\there"]);
    }

    #[test]
    fn test_pipe() {
        let p = parse_line("echo hi | cat").unwrap();
        assert_eq!(p.commands.len(), 2);
        assert_eq!(p.commands[0].argv, vec!["echo", "hi"]);
        assert_eq!(p.commands[1].argv, vec!["cat"]);
    }

    #[test]
    fn test_output_redirect() {
        let p = parse_line("echo hi > out.txt").unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "hi"]);
        assert_eq!(p.commands[0].redirects.len(), 1);
        assert_eq!(p.commands[0].redirects[0].kind, RedirectKind::Output);
        assert_eq!(p.commands[0].redirects[0].target, "out.txt");
    }

    #[test]
    fn test_append_redirect() {
        let p = parse_line("echo hi >> out.txt").unwrap();
        assert_eq!(p.commands[0].redirects[0].kind, RedirectKind::Append);
    }

    #[test]
    fn test_input_redirect() {
        let p = parse_line("cat < in.txt").unwrap();
        assert_eq!(p.commands[0].redirects[0].kind, RedirectKind::Input);
        assert_eq!(p.commands[0].redirects[0].target, "in.txt");
    }

    #[test]
    fn test_stderr_redirect() {
        let p = parse_line("cmd 2> err.txt").unwrap();
        assert_eq!(p.commands[0].redirects[0].kind, RedirectKind::StderrOutput);
    }

    #[test]
    fn test_stderr_merge() {
        let p = parse_line("cmd 2>&1").unwrap();
        assert_eq!(p.commands[0].redirects[0].kind, RedirectKind::StderrMerge);
    }

    #[test]
    fn test_backslash_escape() {
        let p = parse_line(r"echo hello\ world").unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "hello world"]);
    }

    #[test]
    fn test_unterminated_single_quote() {
        let result = parse_line("echo 'unfinished");
        assert!(result.is_err());
    }

    #[test]
    fn test_unterminated_double_quote() {
        let result = parse_line(r#"echo "unfinished"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_comment() {
        let p = parse_line("echo hi # this is a comment").unwrap();
        assert_eq!(p.commands[0].argv, vec!["echo", "hi"]);
    }
}
