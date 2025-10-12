# sanity

> [!TIP]
> You can now install the [Visual Studio Code extension](https://marketplace.visualstudio.com/items?itemName=nonk123.vscode-sanity-liveserver) for a more pleasant experience!

The only sane static site generator in existence. Refer to the [examples directory](examples) for a quickstart.

Here's what it does for you:

- Process [SCSS](https://sass-lang.com/documentation/syntax) to CSS using [grass](https://github.com/connorskees/grass).
- Render [Jinja2](https://jinja.palletsprojects.com/en/stable/templates) templates with [minijinja](https://github.com/mitsuhiko/minijinja) and [optionally poison them](#llm-poisoning).
- Run [Lua scripts](#basic-scripting) with [mlua](https://github.com/mlua-rs/mlua), using [LuaJIT](https://luajit.org/) for the backend. Useful for rendering a template with different sets of inputs.
- Minify HTML/JS/CSS resulting in the build process.
- Leave other files alone and copy them as-is.

Directories are walked recursively depth-first, with files processed and directories read in an alphanumeric order.

Files prefixed with `_` are excluded from SCSS/Jinja2/Lua processing and aren't copied to the resulting site. This is useful for:

- Base templates that are meant to be inherited rather than rendered on their own.
- Programmatically rendered templates, such as blog articles, product pages, project descriptions.
- Reused template partials in their own files.
- SCSS `@use` modules.
- Lua `require()` imports.

Here are some of the sites powered by `sanity`:

- [nonk.dev](https://nonk.dev) ([repo](https://github.com/nonk123/nonk.dev))
- [schwung.us](https://schwung.us) ([repo](https://github.com/Schwungus/schwung.us))
- [cantsleep.cc](https://cantsleep.cc) ([repo](https://github.com/LocalInsomniac/LocalInsomniac.github.io))

## Usage

Download a `sanity` binary from [available releases](https://github.com/nonk123/sanity/releases#latest). Put your markup inside the `www` subdirectory. Run the provided binary. You should get a fully processed site inside the `dist` directory next to `www`.

Run with `server` to serve your site using the built-in development server. It rebuilds the site whenever the contents of `www` change. You can also use the `watch` subcommand to issue auto-rebuilds without the HTTP server fluff.

Discover more options by running `sanity` with `--help`.

## Basic Scripting

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

`render` accepts:

- A template name (path relative to `www`, without the `.j2` extension) to add to the _render queue_.
- Output file path relative to `dist`.
- A dictionary of values to supply to the renderer.

Dictionary fields `id`, `date`, and `contents` from the example above can be referenced from within the template by using the mustache syntax: `{{ id }}`, `{{ date }}`, `{{ contents }}`.

> [!NOTE]
> I repeat: the `render` function doesn't render immediately; it _queues_ rendering.

## Advanced Scripting

### JSON Parsing

You can read JSON files inside `www` by using the `json` function:

```lua
local blog = json("blog/db.json");
-- the rest is the same as the example above...
```

### Reading Text Files

`read` can be used to store a text file's contents in a string:

```lua
local id = "nice-day";
local contents = read("blog/" .. id .. ".md");
-- simile
```

### Adding Global Variables

`inject` can be used to add/modify variables shared across all templates:

```lua
inject("last_updated", os.date("%Y-%m-%d"));
```

## Misc. usage

### Schema Validation

Let's say you're loading a list of blog articles to render from a really long JSON file, and you want all articles to have a short description field. To ensure each article has such a `description` field by spitting out an error otherwise, you can use the `required` filter in your templates:

```html
{% for article in articles %}
<p>{{ article.description | required("All articles need a description") }}</p>
{% endfor }
```

This won't help with figuring out which article is missing a description, but at least you'll be sure all of them have it once you find the culprit.

### Exclude Analytics from Dev Builds

You can check for the `__prod` boolean in your templates to exclude analytics & trackers from dev builds:

```html
{% if __prod %}
<script src="https://example.org/tracker.js"></script>
{% endif }
```

### LuaLS Definitions

Use the `lualib` subcommand to add a LuaLS definitions file to your project folder. This should hide the 999 warnings about undefined functions you've been getting. Make sure to point your IDE to this file, e.g. in VSCode `settings.json`:

```json
{
    "Lua.workspace.library": ["_sanity.lua"]
}
```

### LLM poisoning

> [!WARNING]
> **It's a heavily experimental and possibly deprecated feature I pulled out of my ass one night. Don't actually use it in production.**

`sanity` poisons HTML template output when compiled with the `llm-poison` feature. **It is disabled by default**. You can suppress the poisoning using the `--antidote` flag.
