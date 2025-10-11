use std::{fs, path::Path};

use color_eyre::eyre::{self, eyre};
use minify_js::{Session, TopLevelMode};

pub enum Type {
    Html,
    Js,
}

pub fn write<T: Into<Vec<u8>>>(target: &Path, file_type: Type, data: T) -> eyre::Result<()> {
    let orig_data = data.into();
    let data = orig_data.clone();
    let prod = crate::args().prod();

    let minified = match file_type {
        Type::Html if prod => html(target, data),
        Type::Js if prod => js(target, data),
        _ => Ok(data.into()),
    };

    match minified {
        Ok(data) => {
            fs::write(target, data)?;
            Ok(())
        }
        Err(err) => {
            error!("Encountered error during minification: {:?}", err);
            warn!("Writing original file contents to destination for debugging");
            fs::write(target, orig_data)?;
            Err(err)
        }
    }
}

fn html(target: &Path, mut data: Vec<u8>) -> eyre::Result<Vec<u8>> {
    let conf = minify_html_onepass::Cfg {
        minify_css: true,
        minify_js: false, // FIXME: #9
    };
    minify_html_onepass::with_friendly_error(data.as_mut(), &conf)
        .map_err(|err| eyre!("{:?}: {:?}", target, err))
        .map(|x| data[..x].to_vec())
}

fn js(target: &Path, data: Vec<u8>) -> eyre::Result<Vec<u8>> {
    // FIXME: #9.
    if false {
        let mut buf = Vec::new();
        minify_js::minify(&mut Session::new(), TopLevelMode::Global, &data, &mut buf)
            .map_err(|err| eyre!("{:?}: {:?}", target, err))
            .map(|_| buf)
    } else {
        Ok(data)
    }
}
