use std::fmt::{self, Write};
use std::ops::Range;
use std::path::PathBuf;

use rnix::{parser, NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken};
use thiserror::Error;

use crate::value::NixValueWrapped;
use crate::FileScope;

pub type NixResult<V = NixValueWrapped> = Result<V, NixError>;

#[derive(Clone, Debug)]
pub struct NixError {
    pub message: String,
    pub labels: Vec<NixLabel>,
}

#[derive(Clone, Debug)]
pub struct NixLabel {
    pub path: PathBuf,
    pub line: usize,
    pub range: Range<usize>,
    pub context: String,
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

    #[error("{0}")]
    Custom(String),

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
            first_label.path.display(),
            first_label.line,
            first_label.range.start + 1,
        ))?;

        let mut labels = self.labels.clone();
        labels.sort_by_key(|v| v.line);

        let max_line_width = labels.last().unwrap().line.to_string().len();
        let line_padding = " ".repeat(max_line_width);
        let dots = ".".repeat(max_line_width);

        f.write_str("\x1b[1;34m")?;
        f.write_str(&line_padding)?;
        f.write_str(" | \x1b[0m")?;

        let mut last_line = labels.first().unwrap().line;

        for label in &labels {
            if label.line.abs_diff(last_line) >= 2 {
                f.write_char('\n')?;
                f.write_str(&dots)?;
                f.write_str(" |")?;
            }

            last_line = label.line;

            f.write_fmt(format_args!(
                "\n\x1b[1;34m{line:0>max_line_width$} | \x1b[0m{context}\
                 \n\x1b[1;34m{line_padding} | \x1b[0m{spaces}{color}{arrow} {label}\x1b[0m",
                line = label.line,
                context = label.context,
                spaces = " ".repeat(label.range.start),
                color = label.kind.color(),
                arrow = label.kind.symbol().repeat(label.range.len()),
                label = label.label,
            ))?;
        }

        f.write_char('\n')
    }
}

impl std::error::Error for NixError {}

impl NixError {
    pub fn from_message(label: NixLabel, message: impl ToString) -> Self {
        Self {
            message: message.to_string(),
            labels: vec![label.into()],
        }
    }

    pub fn from_parse_error(file: &FileScope, error: parser::ParseError) -> Self {
        use parser::ParseError::*;
        let (message, labels) = match error {
            Unexpected(_) => todo!(),
            UnexpectedExtra(_) => todo!(),
            UnexpectedWanted(unexpected, range, expected) => {
                if expected.len() == 1 {
                    let range_start: usize = range.start().into();

                    let expected = expected.first().unwrap();
                    let expected = syntax_kind_to_string(*expected);

                    let expected_label = NixLabel::from_offset(
                        file,
                        range_start,
                        expected.len(),
                        NixLabelMessage::AddHere(expected),
                        NixLabelKind::Help,
                    );

                    let unexpected = syntax_kind_to_string(unexpected);

                    let unexpected_label = NixLabel::from_offset(
                        file,
                        range_start + 1,
                        range.len().into(),
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

        Self { message, labels }
    }

    pub fn todo(
        file: &FileScope,
        node: NodeOrToken<SyntaxNode, SyntaxToken>,
        message: impl ToString,
    ) -> Self {
        let message = message.to_string();
        let label = NixLabelMessage::Custom(message.clone());
        let kind = NixLabelKind::Todo;

        Self {
            message,
            labels: vec![match node {
                NodeOrToken::Node(node) => NixLabel::from_syntax_node(file, &node, label, kind),
                NodeOrToken::Token(token) => NixLabel::from_syntax_token(file, &token, label, kind),
            }],
        }
    }
}

impl NixLabel {
    pub fn from_offset(
        file: &FileScope,
        mut offset: usize,
        size: usize,
        label: NixLabelMessage,
        kind: NixLabelKind,
    ) -> Self {
        loop {
            let last_newline = offset
                - file.content[..offset]
                    .chars()
                    .rev()
                    .position(|c| c == '\n')
                    .unwrap_or(offset);

            let next_newline = file.content[last_newline..]
                .chars()
                .position(|c| c == '\n')
                .unwrap_or(file.content.len() - last_newline)
                + last_newline;

            let line = file.content[..=last_newline]
                .chars()
                .filter(|c| *c == '\n')
                .count()
                + 1;

            let Some((line, column)) = (offset - last_newline).checked_sub(1).map(|c| (line, c))
            else {
                offset = last_newline.saturating_sub(1);
                continue;
            };

            let size = size.min(next_newline - last_newline - column);

            let context = file.content[last_newline..next_newline].to_owned();

            break Self {
                path: file.path.clone(),
                line,
                range: column..column + size,
                context,
                label,
                kind,
            };
        }
    }

    pub fn from_syntax_node(
        file: &FileScope,
        node: &SyntaxNode,
        label: NixLabelMessage,
        kind: NixLabelKind,
    ) -> Self {
        NixLabel::from_offset(
            file,
            usize::from(node.text_range().start()) + 1,
            node.text_range().len().into(),
            label,
            kind,
        )
    }

    pub fn from_syntax_token(
        file: &FileScope,
        node: &SyntaxToken,
        label: NixLabelMessage,
        kind: NixLabelKind,
    ) -> Self {
        NixLabel::from_offset(
            file,
            usize::from(node.text_range().start()) + 1,
            node.text_range().len().into(),
            label,
            kind,
        )
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
