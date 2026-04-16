use std::{fs, path::Path};

use color_eyre::eyre;
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_minifier::{Minifier, MinifierOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

pub enum Type {
    Html,
    Js,
}

pub fn write(target: &Path, file_type: Type, data: impl Into<Vec<u8>>) -> eyre::Result<()> {
    let orig_data = data.into();
    let data = orig_data.clone();
    let prod = crate::args().prod();

    let minified = match file_type {
        Type::Html if prod => html(data),
        Type::Js if prod => js(data),
        _ => Ok(data.into()),
    };

    match minified {
        Ok(data) => Ok(fs::write(target, data)?),
        Err(err) => {
            error!("minify {:?}: {:?}", target, err);
            warn!("Writing original file contents to destination for you to debug");
            let _ = fs::write(target, orig_data);
            Err(err)
        }
    }
}

fn html(mut data: Vec<u8>) -> eyre::Result<Vec<u8>> {
    let conf = minify_html_onepass::Cfg {
        minify_css: true,
        minify_js: true,
    };
    let end = minify_html_onepass::with_friendly_error(data.as_mut(), &conf)?;
    Ok(data[..end].to_vec())
}

fn js(data: Vec<u8>) -> eyre::Result<Vec<u8>> {
    let allocator = Allocator::default();

    let data = String::from_utf8(data)?;
    let mut parsed = Parser::new(&allocator, &data, SourceType::cjs()).parse();

    let options = MinifierOptions::default();
    let minifier = Minifier::new(options);
    minifier.minify(&allocator, &mut parsed.program);

    let generated = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: None,
            minify: true,
            comments: CommentOptions::disabled(),
            ..CodegenOptions::default()
        })
        .build(&parsed.program);
    Ok(generated.code.into_bytes())
}
