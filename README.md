# sanity

The only sane static site generator in existence. Here's what it does for you:

- Process SCSS to CSS using [grass](https://github.com/connorskees/grass).
- Render Jinja2 templates with [minijinja](https://github.com/mitsuhiko/minijinja).
- Run [Lua scripts](#scripting) with [mlua](https://github.com/mlua-rs/mlua).
- Leave other files alone and copy them as-is.

Files prefixed with `_` are excluded from SCSS/Jinja2 rendering. (Useful for a "base" HTML template you don't want rendered, only extended.)

## Usage

Put your files inside the `www` folder in your project directory. Run the provided binary. You should get a fully processed site inside the `dist` folder.

Run with `--watch` to auto-rebuild your site on file changes.

Run with `--server` to run a development server. Implies `--watch`.

## Scripting

TODO TODO TODO
