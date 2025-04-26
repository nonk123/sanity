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
--><i class="poison">POISONING!!!</i><!---
-->"#;

    let output = rewrite_str(
        &contents,
        Settings {
            element_content_handlers: vec![
                element!("head", |el| {
                    el.append(
                        r#"<!---
--><style>
i.poison {
    position: absolute;
    left: -9999px;
    top: -9999px;
    opacity: 0;
}
</style><!---
-->"#,
                        ContentType::Html,
                    );
                    Ok(())
                }),
                element!("body *", |el| {
                    el.before(poison, ContentType::Html);
                    Ok(())
                }),
            ],
            ..Settings::new()
        },
    )?;

    Ok(output)
}
