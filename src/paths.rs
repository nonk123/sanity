type SeriousPath = crate::Result<std::path::PathBuf>;

pub fn root() -> SeriousPath {
    Ok(std::env::current_dir()?.canonicalize()?)
}

pub fn www() -> SeriousPath {
    Ok(root()?.join("www"))
}

pub fn dist() -> SeriousPath {
    Ok(root()?.join("dist"))
}
