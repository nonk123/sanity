# sanity

The only sane static site generator in existence.

Here's what it does for you:

- Process [SCSS](https://sass-lang.com/documentation/syntax) to CSS using [grass](https://github.com/connorskees/grass).
- Render [Jinja2](https://jinja.palletsprojects.com/en/stable/templates) templates with [minijinja](https://github.com/mitsuhiko/minijinja).
- Run [Lua scripts](#scripting) with [mlua](https://github.com/mlua-rs/mlua) (uses [LuaJIT](https://luajit.org/) as the backend).
- Leave other files alone and copy them as-is.

Directories are walked recursively depth-first, with files processed and directories read in an alphanumeric order.

Files prefixed with `_` are excluded from SCSS/Jinja2/Lua processing and aren't copied. This is useful for a "base" HTML template you don't want a copy of rendered, or if you render the template programmatically.

## Usage

Put your files inside the `www` folder in your project directory. Run the provided binary. You should get a fully processed site inside the `dist` folder.

Run with `--watch` to auto-rebuild your site on file changes. Run with `--server` to run a development server (implies `--watch`).

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

`render` takes a template (path relative to `www`) to add to the render queue, its output path (relative to `dist`), and a context to supply to it (`id`, `date`, and `contents` above can be referenced within the template using the mustache syntax: `{{ contents }}`).

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
