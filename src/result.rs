use std::fmt;

use rnix::parser;
use thiserror::Error;

use crate::value::NixValueWrapped;

pub type NixResult<V = NixValueWrapped> = Result<V, NixError>;

#[derive(Clone, Debug)]
pub struct NixError {
    data: NixErrorData,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Error)]
pub enum NixErrorData {
    #[error("Variable '{0}' not found.")]
    VariableNotFound(String),

    #[error("{0}")]
    ParseError(#[from] parser::ParseError),
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{desc}\n {line} | {ctx}\n {dots} | {arrow}^\n     at {line}:{column}",
            desc = self.data,
            line = self.line + 1,
            column = self.column + 1,
            ctx = "TODO: Context",
            dots = ".".repeat(self.line.to_string().len()),
            arrow = "-".repeat(self.column)
        ))
    }
}

impl std::error::Error for NixError {}

impl NixError {
    pub fn from_span(_span: (), data: NixErrorData) -> Self {
        Self {
            data,
            line: 2,
            column: 2,
        }
    }
}

impl From<parser::ParseError> for NixError {
    fn from(value: parser::ParseError) -> Self {
        Self::from_span((), value.into())
    }
}
