use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::{Arc, Mutex, Weak},
};

use color_eyre::eyre::{self, eyre};
use minijinja::{Environment, Error, ErrorKind, context, value::merge_maps};

use crate::{fs::PathExt, minify, poison};

pub struct JinjaEnvironment {
    templates: Arc<Mutex<HashMap<String, String>>>,
    base: Environment<'static>,
}

impl JinjaEnvironment {
    pub fn new() -> Self {
        let templates = Arc::new(Mutex::new(HashMap::new()));
        let base = Self::make_env(Arc::downgrade(&templates));
        Self { templates, base }
    }

    fn make_env(templates: Weak<Mutex<HashMap<String, String>>>) -> Environment<'static> {
        let mut base = Environment::new();
        base.add_filter("required", required_filter);
        base.set_loader(move |name| {
            if let Some(templates) = templates.upgrade() {
                Ok(templates.lock().unwrap().get(name).map(String::to_string))
            } else {
                Ok(None)
            }
        });
        base
    }

    pub fn register(&mut self, path: &Path) -> eyre::Result<()> {
        let name = path.template_name()?;
        let source = fs::read_to_string(path)?;
        self.templates.lock().unwrap().insert(name, source);
        Ok(())
    }

    pub fn all(&self) -> HashSet<String> {
        let templates = self.templates.lock().unwrap();
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

        let mut data = self.base.get_template(name)?.render(context)?;
        if !crate::args().antidote {
            data = poison::inject(data)?;
        }

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
