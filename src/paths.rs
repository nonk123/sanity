use std::path::PathBuf;

pub fn root() -> crate::Result<PathBuf> {
    Ok(std::env::current_dir()?.canonicalize()?)
}

pub fn www() -> crate::Result<PathBuf> {
    Ok(root()?.join("www"))
}

pub fn dist() -> crate::Result<PathBuf> {
    Ok(root()?.join("dist"))
}
