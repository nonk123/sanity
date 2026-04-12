use std::{
    ffi::OsStr,
    path::{Component, Path},
    time::SystemTime,
};

use color_eyre::eyre;

use crate::paths;

pub trait PathExt {
    fn extension_str(&self) -> Option<&str>;
    fn is_underscored(&self) -> bool;
    fn last_modified(&self) -> eyre::Result<SystemTime>;
    fn more_recent_than(&self, other: &Path) -> eyre::Result<bool>;
    fn template_name(&self) -> eyre::Result<String>;
}

impl<T: AsRef<Path>> PathExt for T {
    fn extension_str(&self) -> Option<&str> {
        self.as_ref().extension().and_then(OsStr::to_str)
    }

    fn is_underscored(&self) -> bool {
        self.as_ref()
            .file_name()
            .and_then(OsStr::to_str)
            .map(|x| x.starts_with("_"))
            .unwrap_or(false)
    }

    fn last_modified(&self) -> eyre::Result<SystemTime> {
        Ok(std::fs::metadata(self)?.modified()?)
    }

    fn more_recent_than(&self, other: &Path) -> eyre::Result<bool> {
        Ok(other.exists() && other.last_modified()? >= self.last_modified()?)
    }

    fn template_name(&self) -> eyre::Result<String> {
        let mut name = String::new();

        let base = self.as_ref().to_path_buf().with_extension("");
        let components = base.strip_prefix(paths::www()?)?.components();

        for comp in components {
            let Component::Normal(x) = comp else {
                continue;
            };
            if !name.is_empty() {
                name += "/";
            }
            let x = x.as_encoded_bytes().iter().cloned().collect();
            name += &String::from_utf8(x)?;
        }

        Ok(name)
    }
}
