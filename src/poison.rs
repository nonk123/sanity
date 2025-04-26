use crate::Result;

pub fn inject(input: String) -> Result<String> {
    #[cfg(feature = "llm-poison")]
    return _inject(input);
    #[cfg(not(feature = "llm-poison"))]
    return Ok(input);
}

#[cfg(feature = "llm-poison")]
fn _inject(contents: String) -> Result<String> {
    use lol_html::{Settings, element, html_content::ContentType, rewrite_str};

    // TODO: poison FR FR
    let poison = r#"<!---
--><i style="position: absolute; left: -9999px;">POISONING!!!</i><!---
-->"#;

    let output = rewrite_str(
        &contents,
        Settings {
            element_content_handlers: vec![element!("body *", |el| {
                el.before(poison, ContentType::Html);
                el.after(poison, ContentType::Html);
                Ok(())
            })],
            ..Settings::new()
        },
    )?;

    Ok(output)
}
