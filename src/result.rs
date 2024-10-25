use std::fmt::{self, Write};
use std::ops::Range;

use rnix::{parser, SyntaxKind, SyntaxNode, SyntaxToken};

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
    pub line: usize,
    pub range: Range<usize>,
    pub context: String,
    pub label: String,
    pub kind: NixLabelKind,
}

#[derive(Clone, Debug)]
pub enum NixLabelKind {
    Error,
    Help,
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        assert!(!self.labels.is_empty());

        let first_label = self.labels.first().unwrap();

        f.write_str(&self.message)?;
        f.write_fmt(format_args!(
            "\n --> <anonymous>:{}:{}\n",
            first_label.line,
            first_label.range.start + 1,
        ))?;

        let mut labels = self.labels.clone();
        labels.sort_by_key(|v| v.line);

        let max_line_width = labels.last().unwrap().line.to_string().len();
        let line_padding = " ".repeat(max_line_width);
        let dots = ".".repeat(max_line_width);

        f.write_str(&line_padding)?;
        f.write_str(" | ")?;

        let mut last_line = labels.first().unwrap().line;

        let mut render_label = |label: &NixLabel| {
            if label.line.abs_diff(last_line) >= 2 {
                f.write_char('\n')?;
                f.write_str(&dots)?;
                f.write_str(" |")?;
            }

            last_line = label.line;

            let column_start = label.range.start;
            let range_len = label.range.len();

            let color = match label.kind {
                NixLabelKind::Error => "1;91m",
                NixLabelKind::Help => "1;96m",
            };

            let symbol = match label.kind {
                NixLabelKind::Error => "^",
                NixLabelKind::Help => "-",
            };

            f.write_fmt(format_args!(
                "\n{line:0>max_line_width$} | {ctx}\n{line_padding} | {spaces}\x1b[{color}{arrow} {label}\x1b[0m",
                label = label.label,
                line = label.line,
                ctx = label.context,
                spaces = " ".repeat(column_start),
                arrow = symbol.repeat(range_len)
            ))
        };

        for label in &labels {
            render_label(label)?;
        }

        f.write_char('\n')
    }
}

impl std::error::Error for NixError {}

impl NixError {
    pub fn from_message(label: impl Into<NixLabel>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
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

                    let expected_label = {
                        let mut range_start = range_start;

                        loop {
                            let last_newline = range_start
                                - file.content[..range_start]
                                    .chars()
                                    .rev()
                                    .skip(1)
                                    .position(|c| c == '\n')
                                    .unwrap_or(range_start);

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

                            let Some((line, column)) = (range_start - last_newline)
                                .checked_sub(1)
                                .map(|c| (line, c))
                            else {
                                range_start = last_newline.saturating_sub(1);
                                continue;
                            };

                            let context = file.content[last_newline..next_newline].to_owned();

                            break NixLabel {
                                line,
                                range: column..column + expected.len(),
                                context,
                                label: format!("help: add '{expected}' here"),
                                kind: NixLabelKind::Help,
                            };
                        }
                    };

                    let unexpected = syntax_kind_to_string(unexpected);

                    let unexpected_label = {
                        let last_newline = range_start
                            - file.content[..range_start]
                                .chars()
                                .rev()
                                .position(|c| c == '\n')
                                .unwrap_or(range_start);

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

                        let column = range_start - last_newline;

                        let context = file.content[last_newline..next_newline].to_owned();

                        NixLabel {
                            line,
                            range: column..column + usize::from(range.len()),
                            context,
                            label: format!("unexpected token"),
                            kind: NixLabelKind::Error,
                        }
                    };

                    (
                        format!("\x1b[1;91merror\x1b[0m: Unexpected token '{unexpected}'"),
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
}

impl From<&SyntaxNode> for NixLabel {
    fn from(_value: &SyntaxNode) -> Self {
        Self {
            line: 1,
            range: 0..1,
            context: String::new(),
            label: String::new(),
            kind: NixLabelKind::Help,
        }
    }
}

impl From<SyntaxToken> for NixLabel {
    fn from(_value: SyntaxToken) -> Self {
        Self {
            line: 1,
            range: 0..1,
            context: String::new(),
            label: String::new(),
            kind: NixLabelKind::Help,
        }
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
