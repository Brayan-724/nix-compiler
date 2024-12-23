use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fmt, fs};

use crate::{
    LazyNixValue, NixBacktrace, NixBacktraceKind, NixError, NixResult, NixSpan, NixValueWrapped,
    NixVar,
};

use super::Scope;

thread_local! {
    static FILE_CACHE: RefCell<HashMap<PathBuf, (Rc<NixSpan>, NixVar)>> = HashMap::new().into();
}

#[derive(PartialEq, Eq)]
pub struct FileScope {
    pub path: PathBuf,
    pub content: String,
}

impl fmt::Debug for FileScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileScope")
            .field("path", &self.path)
            .finish()
    }
}

impl FileScope {
    fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
        let mut path = path.as_ref().to_path_buf();

        if path.is_dir() {
            path.push("default.nix")
        }

        path.canonicalize().unwrap()
    }

    pub fn get_file(
        backtrace: impl Into<Rc<Option<NixBacktrace>>>,
        path: impl AsRef<Path>,
    ) -> NixResult<(NixBacktrace, NixValueWrapped)> {
        FILE_CACHE.with(|file_cache| {
            let path = Self::normalize_path(path);

            let (backtrace, out) = {
                let backtrace = backtrace.into();

                let mut file_cache = file_cache.borrow_mut();

                let entry = file_cache.entry(path);

                match entry {
                    Entry::Occupied(e) => {
                        let (span, value) = e.get().clone();
                        let backtrace = NixBacktrace(span, backtrace, NixBacktraceKind::File);
                        (backtrace, value)
                    }
                    Entry::Vacant(e) => {
                        let path = e.key();
                        let path = path.clone();

                        let (backtrace, span, out) = Rc::new(FileScope {
                            content: fs::read_to_string(&path).unwrap(),
                            path,
                        })
                        .raw_evaluate(backtrace)?;

                        e.insert((span, out.clone()));

                        (backtrace, out)
                    }
                }
            };

            let out = out.resolve(&backtrace)?;

            Ok((backtrace, out))
        })
    }

    pub fn repl_file(path: PathBuf, content: String) -> NixResult<(NixBacktrace, NixValueWrapped)> {
        Rc::new(FileScope { path, content })
            .raw_evaluate(None.into())
            .and_then(|r| Ok((r.0.clone(), r.2.resolve(&r.0)?)))
    }

    fn raw_evaluate(
        self: Rc<Self>,
        backtrace: Rc<Option<NixBacktrace>>,
    ) -> NixResult<(NixBacktrace, Rc<NixSpan>, NixVar)> {
        let root = rnix::Root::parse(&self.content)
            .ok()
            .map_err(|error| NixError::from_parse_error(&self, error))?;

        let span = Rc::new(NixSpan::from_ast_node(&self, &root));
        let backtrace = NixBacktrace(span.clone(), backtrace, NixBacktraceKind::File);

        let scope = Scope::new_with_builtins(self);

        let out =
            LazyNixValue::Pending(backtrace.clone(), scope, rnix::ast::Expr::Root(root)).wrap_var();

        Ok((backtrace, span, out))
    }
}
