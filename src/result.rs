use std::fmt::{self, Write};
use std::rc::Rc;

use rnix::{parser, NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::ast::AstNode;
use thiserror::Error;

use crate::value::NixValueWrapped;
use crate::FileScope;

pub type NixResult<V = NixValueWrapped> = Result<V, NixError>;

#[derive(Clone, Debug)]
pub struct NixError {
    pub message: String,
    pub labels: Vec<NixLabel>,
    pub backtrace: Option<Rc<NixBacktrace>>,
}

#[derive(Clone, Debug)]
pub struct NixBacktrace(pub Rc<NixSpan>, pub Option<Rc<NixBacktrace>>);

#[derive(Clone, Debug)]
pub struct NixSpan {
    pub file: Rc<FileScope>,
    pub start: (usize, usize, usize),
    pub end: (usize, usize, usize),
}

#[derive(Clone, Debug)]
pub struct NixLabel {
    pub span: Rc<NixSpan>,
    pub label: NixLabelMessage,
    pub kind: NixLabelKind,
}

#[derive(Clone, Debug)]
pub enum NixLabelKind {
    Error,
    Help,
    Todo,
}

#[derive(Clone, Debug, Error)]
pub enum NixLabelMessage {
    #[error("Help: add '{0}' here")]
    AddHere(&'static str),

    #[error("Assertion failed")]
    AssertionFailed,

    #[error("Attribute missing")]
    AttributeMissing,

    #[error("{0}")]
    Custom(String),

    #[error("")]
    Empty,

    #[error("Unexpected token")]
    UnexpectedToken,

    #[error("Variable not found")]
    VariableNotFound,
}

impl NixLabelKind {
    pub fn color(&self) -> &'static str {
        match self {
            NixLabelKind::Error => "\x1b[1;91m",
            NixLabelKind::Help => "\x1b[1;96m",
            NixLabelKind::Todo => "\x1b[1;94m",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            NixLabelKind::Error => "^",
            NixLabelKind::Help => "-",
            NixLabelKind::Todo => "-",
        }
    }

    pub fn text(&self) -> &'static str {
        match self {
            NixLabelKind::Error => "error",
            NixLabelKind::Help => "help",
            NixLabelKind::Todo => "todo",
        }
    }
}

impl fmt::Display for NixBacktrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self
            .0
            .file
            .path
            .strip_prefix(std::env::current_dir().unwrap())
            .map(|p| format!("./{}", p.display()))
            .unwrap_or(self.0.file.path.display().to_string());

        f.write_fmt(format_args!(
            "at {file} {line}:{column}",
            line = self.0.start.0,
            column = self.0.start.1
        ))?;

        if let Some(backtrace) = &self.1 {
            f.write_fmt(format_args!("\n{backtrace}"))?;
        }

        Ok(())
    }
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        assert!(!self.labels.is_empty());

        let first_label = self.labels.first().unwrap();

        f.write_str(first_label.kind.color())?;
        f.write_str(first_label.kind.text())?;
        f.write_str(":\x1b[0m ")?;
        f.write_str(&self.message)?;
        f.write_fmt(format_args!(
            "\n \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
            first_label.span.file.path.display(),
            first_label.span.start.0,
            first_label.span.start.1 + 1,
        ))?;

        let mut labels = self.labels.clone();
        labels.sort_by_key(|v| v.span.start.0);

        let max_line_width = labels.last().unwrap().span.end.0.to_string().len();
        let line_padding = " ".repeat(max_line_width);
        let dots = ".".repeat(max_line_width);

        f.write_str("\x1b[1;34m")?;
        f.write_str(&line_padding)?;
        f.write_str(" | \x1b[0m")?;

        let mut last_line = usize::MAX;

        for label in &labels {
            if last_line != usize::MAX && label.span.start.0.abs_diff(last_line) >= 2 {
                f.write_char('\n')?;
                f.write_str(&dots)?;
                f.write_str(" |")?;
            }

            let is_singleline = label.span.start.0 == label.span.end.0;

            if label.span.start.0 != last_line {
                let start_line = label.span.start.0;
                let offset_line = label.span.start.2;

                if is_singleline {
                    let next_newline = label.span.file.content[offset_line..]
                        .chars()
                        .skip(1)
                        .position(|c| c == '\n')
                        .unwrap_or_else(|| label.span.file.content.len() - offset_line)
                        + offset_line
                        + 1;

                    f.write_fmt(format_args!(
                        "\n\x1b[1;34m{line:0>max_line_width$} | \x1b[0m{context}",
                        line = start_line,
                        context = &label.span.file.content[offset_line..next_newline]
                    ))?;
                } else {
                    let next_newline = {
                        let mut line = start_line;
                        label.span.file.content[offset_line..]
                            .chars()
                            .skip(1)
                            .position(|c| match c {
                                '\n' if line >= label.span.end.0 => true,
                                '\n' => {
                                    line += 1;
                                    false
                                }
                                _ => false,
                            })
                            .unwrap_or_else(|| label.span.file.content.len() - offset_line)
                            + offset_line
                            + 1
                    };

                    let mut line = start_line;
                    f.write_fmt(format_args!(
                        "\n\x1b[1;34m{line:0>max_line_width$} {color}/ \x1b[0m",
                        color = label.kind.color()
                    ))?;
                    for c in label.span.file.content[offset_line..next_newline].chars() {
                        if c == '\n' {
                            line += 1;
                            f.write_fmt(format_args!(
                                "\n\x1b[1;34m{line:0>max_line_width$} {color}| \x1b[0m",
                                color = label.kind.color()
                            ))?;
                            continue;
                        }

                        f.write_char(c)?;
                    }
                }

                last_line = label.span.end.0;
            }

            if is_singleline {
                f.write_fmt(format_args!(
                    "\n\x1b[1;34m{line_padding} | \x1b[0m{spaces}{color}{arrow} {label}\x1b[0m",
                    spaces = " ".repeat(label.span.start.1),
                    color = label.kind.color(),
                    arrow = label
                        .kind
                        .symbol()
                        .repeat(label.span.start.1.abs_diff(label.span.end.1) + 1),
                    label = label.label,
                ))?;
            } else {
                f.write_fmt(format_args!(
                    "\n\x1b[1;34m{line_padding} {color}\\ {arrow} {label}\x1b[0m",
                    color = label.kind.color(),
                    arrow = label
                        .kind
                        .symbol()
                        .repeat(label.span.end.1.max(label.span.start.1) + 1),
                    label = label.label,
                ))?;
            }
        }

        f.write_char('\n')?;

        if let Some(backtrace) = &self.backtrace {
            f.write_fmt(format_args!("{backtrace}"));
        }

        Ok(())
    }
}

impl std::error::Error for NixError {}

impl NixError {
    pub fn from_message(label: NixLabel, message: impl ToString) -> Self {
        Self {
            message: message.to_string(),
            labels: vec![label.into()],
            backtrace: None,
        }
    }

    pub fn from_parse_error(file: &Rc<FileScope>, error: parser::ParseError) -> Self {
        use parser::ParseError::*;
        let (message, labels) = match error {
            Unexpected(_) => todo!(),
            UnexpectedExtra(_) => todo!(),
            UnexpectedWanted(unexpected, range, expected) => {
                if expected.len() == 1 {
                    let range_start: usize = range.start().into();

                    let expected = expected.first().unwrap();
                    let expected = syntax_kind_to_string(*expected);

                    let expected_label = NixLabel::new(
                        NixSpan::from_offset(file, range_start, range_start).into(),
                        NixLabelMessage::AddHere(expected),
                        NixLabelKind::Help,
                    );

                    let unexpected = syntax_kind_to_string(unexpected);

                    let unexpected_label = NixLabel::new(
                        NixSpan::from_offset(
                            file,
                            range_start + 1,
                            range_start + usize::from(range.len()),
                        )
                        .into(),
                        NixLabelMessage::UnexpectedToken,
                        NixLabelKind::Error,
                    );

                    (
                        format!("Unexpected token '{unexpected}'"),
                        vec![unexpected_label, expected_label],
                    )
                } else {
                    todo!()
                }
            }
            UnexpectedDoubleBind(_) => todo!(),
            UnexpectedEOF => todo!(),
            UnexpectedEOFWanted(_) => todo!(),
            DuplicatedArgs(_, _) => todo!(),
            RecursionLimitExceeded => todo!(),
            _ => unreachable!(),
        };

        Self {
            message,
            labels,
            backtrace: None,
        }
    }

    pub fn todo(
        span: Rc<NixSpan>,
        message: impl ToString,
        backtrace: Option<Rc<NixBacktrace>>,
    ) -> Self {
        let message = message.to_string();
        let label = NixLabelMessage::Custom(message.clone());
        let kind = NixLabelKind::Todo;

        let label = NixLabel::new(span, label, kind);

        Self {
            message,
            labels: vec![label],
            backtrace,
        }
    }
}

impl NixSpan {
    fn get_line_column(file: &FileScope, mut offset: usize) -> (usize, usize, usize) {
        loop {
            let last_newline = offset
                - file.content[..offset]
                    .chars()
                    .rev()
                    .position(|c| c == '\n')
                    .unwrap_or(offset);

            // let next_newline = file.content[last_newline..]
            //     .chars()
            //     .position(|c| c == '\n')
            //     .unwrap_or(file.content.len() - last_newline)
            //     + last_newline;

            let line = file.content[..=last_newline.min(file.content.len() - 1)]
                .chars()
                .filter(|c| *c == '\n')
                .count()
                + 1;

            let Some(column) = (offset - last_newline).checked_sub(1) else {
                offset = last_newline.saturating_sub(1);
                continue;
            };

            break (line, column, last_newline);
        }
    }

    pub fn from_offset(file: &Rc<FileScope>, start: usize, end: usize) -> Self {
        let start = Self::get_line_column(file, start);
        let end = Self::get_line_column(file, end);

        Self {
            file: file.clone(),
            start,
            end,
        }
    }

    pub fn from_syntax_element(file: &Rc<FileScope>, node: &SyntaxElement) -> Self {
        match node {
            NodeOrToken::Node(node) => Self::from_syntax_node(file, node),
            NodeOrToken::Token(node) => Self::from_syntax_token(file, node),
        }
    }

    pub fn from_syntax_node(file: &Rc<FileScope>, node: &SyntaxNode) -> Self {
        Self::from_offset(
            file,
            usize::from(node.text_range().start()) + 1,
            usize::from(node.text_range().end()),
        )
    }

    pub fn from_syntax_token(file: &Rc<FileScope>, node: &SyntaxToken) -> Self {
        Self::from_offset(
            file,
            usize::from(node.text_range().start()) + 1,
            usize::from(node.text_range().end()),
        )
    }

    pub fn from_ast_node(file: &Rc<FileScope>, node: &impl AstNode) -> Self {
        Self::from_offset(
            file,
            usize::from(node.syntax().text_range().start()) + 1,
            usize::from(node.syntax().text_range().end()),
        )
    }
}

impl NixLabel {
    pub fn new(span: Rc<NixSpan>, label: NixLabelMessage, kind: NixLabelKind) -> Self {
        Self { span, label, kind }
    }
}

impl From<String> for NixLabelMessage {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

fn syntax_kind_to_string(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::TOKEN_COMMENT => "<comment>",
        SyntaxKind::TOKEN_ERROR => "<error>",
        SyntaxKind::TOKEN_WHITESPACE => "<whitespace>",

        // Keywords
        SyntaxKind::TOKEN_ASSERT => "assert",
        SyntaxKind::TOKEN_ELSE => todo!(),
        SyntaxKind::TOKEN_IF => todo!(),
        SyntaxKind::TOKEN_IN => todo!(),
        SyntaxKind::TOKEN_INHERIT => todo!(),
        SyntaxKind::TOKEN_LET => todo!(),
        SyntaxKind::TOKEN_OR => todo!(),
        SyntaxKind::TOKEN_REC => todo!(),
        SyntaxKind::TOKEN_THEN => todo!(),
        SyntaxKind::TOKEN_WITH => todo!(),

        // Literals
        SyntaxKind::TOKEN_FLOAT => todo!(),
        SyntaxKind::TOKEN_IDENT => todo!(),
        SyntaxKind::TOKEN_INTEGER => todo!(),
        SyntaxKind::TOKEN_INTERPOL_END => todo!(),
        SyntaxKind::TOKEN_INTERPOL_START => todo!(),
        SyntaxKind::TOKEN_PATH => todo!(),
        SyntaxKind::TOKEN_URI => todo!(),
        SyntaxKind::TOKEN_STRING_CONTENT => todo!(),
        SyntaxKind::TOKEN_STRING_END => todo!(),
        SyntaxKind::TOKEN_STRING_START => todo!(),

        // Punctuation
        SyntaxKind::TOKEN_ELLIPSIS => "...",
        SyntaxKind::TOKEN_L_BRACE => "{",
        SyntaxKind::TOKEN_R_BRACE => "}",
        SyntaxKind::TOKEN_L_BRACK => "[",
        SyntaxKind::TOKEN_R_BRACK => "]",
        SyntaxKind::TOKEN_L_PAREN => "(",
        SyntaxKind::TOKEN_R_PAREN => ")",
        SyntaxKind::TOKEN_SEMICOLON => ";",

        // Operators
        SyntaxKind::TOKEN_ASSIGN => todo!(),
        SyntaxKind::TOKEN_AT => todo!(),
        SyntaxKind::TOKEN_COLON => todo!(),
        SyntaxKind::TOKEN_COMMA => todo!(),
        SyntaxKind::TOKEN_DOT => todo!(),
        SyntaxKind::TOKEN_QUESTION => todo!(),
        SyntaxKind::TOKEN_CONCAT => todo!(),
        SyntaxKind::TOKEN_INVERT => todo!(),
        SyntaxKind::TOKEN_UPDATE => todo!(),
        SyntaxKind::TOKEN_ADD => todo!(),
        SyntaxKind::TOKEN_SUB => todo!(),
        SyntaxKind::TOKEN_MUL => todo!(),
        SyntaxKind::TOKEN_DIV => todo!(),
        SyntaxKind::TOKEN_AND_AND => todo!(),
        SyntaxKind::TOKEN_EQUAL => todo!(),
        SyntaxKind::TOKEN_IMPLICATION => todo!(),
        SyntaxKind::TOKEN_LESS => todo!(),
        SyntaxKind::TOKEN_LESS_OR_EQ => todo!(),
        SyntaxKind::TOKEN_MORE => todo!(),
        SyntaxKind::TOKEN_MORE_OR_EQ => todo!(),
        SyntaxKind::TOKEN_NOT_EQUAL => todo!(),
        SyntaxKind::TOKEN_OR_OR => todo!(),

        SyntaxKind::NODE_APPLY => todo!(),
        SyntaxKind::NODE_ASSERT => todo!(),
        SyntaxKind::NODE_ATTRPATH => todo!(),
        SyntaxKind::NODE_DYNAMIC => todo!(),
        SyntaxKind::NODE_ERROR => todo!(),
        SyntaxKind::NODE_IDENT => todo!(),
        SyntaxKind::NODE_IF_ELSE => todo!(),
        SyntaxKind::NODE_SELECT => todo!(),
        SyntaxKind::NODE_INHERIT => todo!(),
        SyntaxKind::NODE_INHERIT_FROM => todo!(),
        SyntaxKind::NODE_STRING => todo!(),
        SyntaxKind::NODE_INTERPOL => todo!(),
        SyntaxKind::NODE_LAMBDA => todo!(),
        SyntaxKind::NODE_IDENT_PARAM => todo!(),
        SyntaxKind::NODE_LEGACY_LET => todo!(),
        SyntaxKind::NODE_LET_IN => todo!(),
        SyntaxKind::NODE_LIST => todo!(),
        SyntaxKind::NODE_BIN_OP => todo!(),
        SyntaxKind::NODE_PAREN => todo!(),
        SyntaxKind::NODE_PATTERN => todo!(),
        SyntaxKind::NODE_PAT_BIND => todo!(),
        SyntaxKind::NODE_PAT_ENTRY => todo!(),
        SyntaxKind::NODE_ROOT => todo!(),
        SyntaxKind::NODE_ATTR_SET => todo!(),
        SyntaxKind::NODE_ATTRPATH_VALUE => todo!(),
        SyntaxKind::NODE_UNARY_OP => todo!(),
        SyntaxKind::NODE_LITERAL => todo!(),
        SyntaxKind::NODE_WITH => todo!(),
        SyntaxKind::NODE_PATH => todo!(),
        SyntaxKind::NODE_HAS_ATTR => todo!(),
        _ => todo!(),
    }
}
