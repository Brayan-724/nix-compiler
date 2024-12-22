use core::fmt;
use std::{rc::Rc, sync::LazyLock};

use rnix::ast;
use rowan::ast::AstNode;
use thiserror::Error;

use crate::FileScope;

use super::{print_labels, NixError, NixLabel, NixLabelKind, NixLabelMessage, NixSpan};

pub static BACKTRACE_ENV: LazyLock<BacktraceEnv> = LazyLock::new(|| {
    std::env::var("NIX_BACKTRACE")
        .map(|env| {
            env.starts_with("f")
                .then_some(BacktraceEnv::Full)
                .unwrap_or(BacktraceEnv::Enabled)
        })
        .unwrap_or(BacktraceEnv::Disabled)
});

#[derive(PartialEq, Eq)]
pub enum BacktraceEnv {
    Disabled,
    Enabled,
    Full,
}

impl BacktraceEnv {
    pub fn is_disabled(&self) -> bool {
        *self == Self::Disabled
    }
}

#[derive(Clone, Debug)]
pub struct NixBacktrace(
    pub Rc<NixSpan>,
    pub Rc<Option<NixBacktrace>>,
    pub NixBacktraceKind,
);

#[derive(Clone, Copy, Debug, Error)]
pub enum NixBacktraceKind {
    #[error("")]
    None,

    #[error("Apply")]
    Apply,
    #[error("Assert")]
    Assert,
    #[error("Error")]
    Error,
    #[error("IfElse")]
    IfElse,
    #[error("Select")]
    Select,
    #[error("Str")]
    Str,
    #[error("Path")]
    Path,
    #[error("Literal")]
    Literal,
    #[error("Lambda")]
    Lambda,
    #[error("LegacyLet")]
    LegacyLet,
    #[error("LetIn")]
    LetIn,
    #[error("List")]
    List,
    #[error("BinOp")]
    BinOp,
    #[error("AttrSet")]
    AttrSet,
    #[error("UnaryOp")]
    UnaryOp,
    #[error("Ident")]
    Ident,
    #[error("With")]
    With,
    #[error("HasAttr")]
    HasAttr,
}

impl NixBacktrace {
    pub fn new_none(span: Rc<NixSpan>, backtrace: impl Into<Rc<Option<NixBacktrace>>>) -> Self {
        Self(span, backtrace.into(), NixBacktraceKind::None)
    }

    pub fn change_span(&self, span: impl Into<NixSpan>) -> Self {
        Self(Rc::new(span.into()), self.1.clone(), self.2)
    }

    pub fn child(&self, file: &Rc<FileScope>, node: &impl AstNode, kind: NixBacktraceKind) -> Self {
        Self(
            Rc::new(NixSpan::from_ast_node(file, node)),
            Rc::new(Some(self.clone())),
            kind,
        )
    }

    pub fn child_none(&self, file: &Rc<FileScope>, node: &impl AstNode) -> Self {
        Self::child(&self, file, node, NixBacktraceKind::None)
    }

    pub fn to_error(
        &self,
        kind: NixLabelKind,
        label: NixLabelMessage,
        message: impl ToString,
    ) -> NixError {
        let NixBacktrace(span, backtrace, backtrace_kind) = self.clone();

        let label = label.or_else(|| NixLabelMessage::Custom(format!("in {backtrace_kind}")));

        let label = NixLabel::new(span, label, kind);

        NixError {
            message: message.to_string(),
            labels: vec![label],
            backtrace,
        }
    }

    pub fn to_labeled_error(&self, labels: Vec<NixLabel>, message: impl ToString) -> NixError {
        NixError {
            labels,
            message: message.to_string(),
            backtrace: Some(self.clone()).into(),
        }
    }

    pub fn visit(&self, file: &Rc<FileScope>, node: &ast::Expr) -> Self {
        match node {
            ast::Expr::Apply(node) => self.child(file, node, NixBacktraceKind::Apply),
            ast::Expr::Assert(node) => self.child(file, node, NixBacktraceKind::Assert),
            ast::Expr::AttrSet(_) => self.clone(),
            ast::Expr::BinOp(node) => self.child(file, node, NixBacktraceKind::BinOp),
            ast::Expr::Error(node) => self.child(file, node, NixBacktraceKind::Error),
            ast::Expr::HasAttr(node) => self.child(file, node, NixBacktraceKind::HasAttr),
            ast::Expr::Ident(node) => self.child(file, node, NixBacktraceKind::Ident),
            ast::Expr::IfElse(node) => self.child(file, node, NixBacktraceKind::IfElse),
            ast::Expr::Lambda(node) => self.child(file, node, NixBacktraceKind::Lambda),
            ast::Expr::LegacyLet(node) => self.child(file, node, NixBacktraceKind::LegacyLet),
            ast::Expr::LetIn(node) => self.child(file, node, NixBacktraceKind::LetIn),
            ast::Expr::List(_) => self.clone(),
            ast::Expr::Literal(node) => self.child(file, node, NixBacktraceKind::Literal),
            ast::Expr::Paren(_) => self.clone(),
            ast::Expr::Path(node) => self.child(file, node, NixBacktraceKind::Path),
            ast::Expr::Root(_) => self.clone(),
            ast::Expr::Select(node) => self.child(file, node, NixBacktraceKind::Select),
            ast::Expr::Str(node) => self.child(file, node, NixBacktraceKind::Str),
            ast::Expr::UnaryOp(node) => self.child(file, node, NixBacktraceKind::UnaryOp),
            ast::Expr::With(node) => self.child(file, node, NixBacktraceKind::With),
        }
    }
}

impl fmt::Display for NixBacktrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *BACKTRACE_ENV {
            BacktraceEnv::Disabled => Ok(()),
            BacktraceEnv::Enabled => {
                let file = self
                    .0
                    .file
                    .path
                    .strip_prefix(std::env::current_dir().unwrap())
                    .map(|p| format!("./{}", p.display()))
                    .unwrap_or(self.0.file.path.display().to_string());

                f.write_fmt(format_args!(
                    "    \x1b[34mat\x1b[36m {file}\x1b[0m {line}:{column}",
                    line = self.0.start.0,
                    column = self.0.start.1
                ))?;

                if let Some(backtrace) = &*self.1 {
                    f.write_fmt(format_args!("\n{backtrace}"))?;
                }

                Ok(())
            }
            BacktraceEnv::Full => print_labels(
                f,
                &[NixLabel::new(
                    self.0.clone(),
                    NixLabelMessage::Empty,
                    NixLabelKind::Help,
                )],
                None,
                self.1.clone(),
            ),
        }
    }
}
