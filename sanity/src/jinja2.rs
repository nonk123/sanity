use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::RwLock,
};

use color_eyre::eyre::{self, eyre};
use minijinja::{Environment, Error, ErrorKind, context, value::merge_maps};

use crate::{fs::PathExt, minify};

pub struct JinjaEnvironment {
    templates: RwLock<HashMap<String, String>>,
}

impl JinjaEnvironment {
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
        }
    }

    fn make_env(templates: HashMap<String, String>) -> Environment<'static> {
        let mut base = Environment::new();

        base.add_filter("required", required_filter);

        base.set_loader(move |name| Ok(templates.get(name).map(String::to_string)));

        base
    }

    pub fn register(&self, path: &Path) -> eyre::Result<()> {
        let name = path.template_name()?;
        let source = fs::read_to_string(path)?;
        self.templates.write().unwrap().insert(name, source);
        Ok(())
    }

    pub fn all(&self) -> HashSet<String> {
        let templates = self.templates.read().unwrap();
        templates.keys().map(String::to_string).collect()
    }

    pub fn render(
        &self,
        name: &str,
        target: &Path,
        context: &minijinja::Value,
    ) -> eyre::Result<()> {
        let parent = target.parent().ok_or(eyre!("No parent directory???"))?;
        let _ = fs::create_dir_all(parent);

        let context = merge_maps([
            context! {
                __prod => crate::args().prod()
            },
            context.clone(),
        ]);

        let templates = self.templates.read().unwrap().to_owned();
        let base = Self::make_env(templates);

        let data = base.get_template(name)?.render(context)?;

        if let Some("html") = target.extension_str() {
            minify::write(&target, minify::Type::Html, data)?;
        } else {
            fs::write(target, data)?;
        }

        Ok(())
    }
}

fn required_filter(
    value: Option<minijinja::Value>,
    error_message: String,
) -> Result<minijinja::Value, Error> {
    if let Some(value) = value {
        Ok(value)
    } else {
        Err(Error::new(ErrorKind::InvalidOperation, error_message))
    }
}
