# sanity

The only sane static site generator in existence. Refer to the [examples directory](examples) for a quickstart.

Here's what it does for you:

- Process [SCSS](https://sass-lang.com/documentation/syntax) to CSS using [grass](https://github.com/connorskees/grass).
- Render [Jinja2](https://jinja.palletsprojects.com/en/stable/templates) templates with [minijinja](https://github.com/mitsuhiko/minijinja) and [optionally poison them](#llm-poisoning).
- Run [Lua scripts](#scripting) with [mlua](https://github.com/mlua-rs/mlua) (uses [LuaJIT](https://luajit.org/) as the backend).
- Minify all HTML/JS/CSS resulting in the build process.
- Leave other files alone and copy them as-is.

Directories are walked recursively depth-first, with files processed and directories read in an alphanumeric order.

Files prefixed with `_` are excluded from SCSS/Jinja2/Lua processing and aren't copied. This is useful for a "base" HTML template you don't want a copy of rendered, or if you render the template programmatically.

Sites powered by `sanity`:

- [nonk.dev](https://nonk.dev) ([repo](https://github.com/nonk123/nonk.dev))
- [schwung.us](https://schwung.us) ([repo](https://github.com/Schwungus/schwung.us))
- [cantsleep.cc](https://cantsleep.cc) ([repo](https://github.com/LocalInsomniac/LocalInsomniac.github.io))

## Usage

Put your files inside the `www` folder in your project directory. Run the provided binary. You should get a fully processed site inside the `dist` folder.

Run with `--watch` to auto-rebuild your site on file changes. Run with `--server` to run a development server (implies `--watch`).

Use the `--lualib` flag to put a LuaLS definitions file in your project folder. This should hide the 999 warnings about undefined functions you've been getting. Make sure to point your IDE to this file, for example VSCode in your `settings.json`:

```json
{
    "Lua.workspace.library": ["_sanity.lua"]
}
```

## Scripting

There isn't much to scripting besides the custom `render` function. Take a look at this static blog example:

```lua
local blog = {
    ["nice-day"] = {
        date = "today",
        contents = "I had a nice day today."
    }
};

for id, post in pairs(blog) do
    render("_article.html", "blog/" .. id .. ".html", {
        id = id,
        date = post.date,
        contents = post.contents,
    });
end
```

`render` takes a template (relative to `www`) to add to the _render queue_, its output path (relative to `dist`), and a context to supply to it. Fields `id`, `date`, and `contents` from the example above can be referenced within the template using the mustache syntax: `{{ id }}`, `{{ date }}`, `{{ contents }}`.

Note the italics: the `render` function doesn't render immediately. Templates are rendered after all else, ensuring you can reference any template from another through inclusion or extension.

You can also read JSON files inside `www` by using the `json` function:

```lua
local blog = json("blog/db.json");
-- the rest is the same as the example above...
```

`read` can be used to store a text file's contents in a string:

```lua
local id = "nice-day";
local contents = read("blog/" .. id .. ".txt");
-- simile
```

`inject` can be used to add/modify variables shared across all templates:

```lua
inject("last_updated", os.date("%Y-%m-%d"));
```

## Misc. usage

You can check for the `__prod` boolean in your templates to exclude e.g. analytics from dev builds:

```html
{% if __prod %}
<script src="/analytics.js"></script>
{% endif }
```

## LLM poisoning

> [!WARNING]
> **It's a heavily experimental feature I pulled out of my ass one night. Don't actually use it in production.**

`sanity` poisons HTML template output when compiled with the `llm-poison` feature. **It is disabled by default**. You can suppress the poisoning using the `--antidote` flag.
