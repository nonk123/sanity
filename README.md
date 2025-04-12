# sanity

The only sane static site generator in existence. Here's what it does for you:

- Process SCSS to CSS using [grass](https://github.com/connorskees/grass).
- Render Jinja2 templates with [minijinja](https://github.com/mitsuhiko/minijinja).
- Run [Lua scripts](#scripting) with [mlua](https://github.com/mlua-rs/mlua).
- Leave other files alone and copy them as-is.

Directories are walked recursively depth-first, with files processed and directories read in an alphanumeric order.

Files prefixed with `_` are excluded from SCSS/Jinja2 rendering. (Useful for a "base" HTML template you don't want rendered, only extended.)

## Usage

Put your files inside the `www` folder in your project directory. Run the provided binary. You should get a fully processed site inside the `dist` folder.

Run with `--watch` to auto-rebuild your site on file changes.

Run with `--server` to run a development server. Implies `--watch`.

## Scripting

There isn't much to scripting besides the custom `render` function. Take a look at this static blog example:

```lua
local blog = {
    ["nice-day"] = {
        "date": "today",
        "contents": "I had a nice day today."
    }
}

for id, post in pairs(blog) do
    render("_article.html", "blog/" .. id .. ".html", {
        id = id,
        date = post.date,
        contents = post.contents,
    });
end
```

`render` takes a template name to render, where to write the output (relative to `dist`), and what context to supply to it. (You can use `date` and `contents` from above using the `{{ name }}` mustache syntax inside your template.)

You can also read JSON files inside `www` into Lua tables with e.g.:

```lua
local blog = json("blog/db.json");
-- the rest is the same as the example above...
```
