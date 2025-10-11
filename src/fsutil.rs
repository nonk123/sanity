use std::{ffi::OsStr, path::Path};

pub fn is_underscored(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(|x| x.starts_with("_"))
        .unwrap_or(false)
}
