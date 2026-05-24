use std::{fmt::Display, path::Path};

type SeriousPath = color_eyre::eyre::Result<std::path::PathBuf>;

pub fn root() -> SeriousPath {
    Ok(std::env::current_dir()?.canonicalize()?)
}

pub fn www() -> SeriousPath {
    Ok(root()?.join("www"))
}

pub fn dist() -> SeriousPath {
    Ok(root()?.join("dist"))
}

pub trait PathExt {
    fn display_simple(&self) -> impl Display;
}

impl<T: AsRef<Path>> PathExt for T {
    fn display_simple(&self) -> impl Display {
        dunce::simplified(self.as_ref()).display()
    }
}
