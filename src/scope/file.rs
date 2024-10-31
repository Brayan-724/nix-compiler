use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fmt, fs};

use crate::{NixError, NixResult};

use super::Scope;

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
    pub fn from_path(path: impl AsRef<Path>) -> Rc<Self> {
        let mut path = path.as_ref().to_path_buf();

        if path.is_dir() {
            path.push("default.nix")
        }

        let path = path.canonicalize().expect("File path is already found");

        Rc::new(FileScope {
            content: fs::read_to_string(&path).unwrap(),
            path,
        })
    }

    pub fn evaluate(self: Rc<Self>) -> NixResult {
        let root = rnix::Root::parse(&self.content)
            .ok()
            .map_err(|error| NixError::from_parse_error(&self, error))?;

        let scope = Scope::new_with_builtins(self);

        scope.visit_root(root)
    }
}
