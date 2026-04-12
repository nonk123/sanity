---@meta

---Queue a template to be rendered to outpath.
---
---@param template string
---@param outpath string
---@param context table
---@return nil
function render(template, outpath, context) end

---Load a JSON file from `www` as a Lua table.
---
---@param path string
---@return any
function json(path) end

---Read a text file from `www` and return its contents as a string.
---
---@param path string
---@return string
function read(path) end

---Add a global variable that can be referenced from templates.
---
---@param name string
---@param value any
---@return nil
function inject(name, value) end

---Return a file's last-modified date as an ISO timestamp string.
---
---Useful for embedding in a `sitemap.xml`.
---
---@param path string
---@return string
function lastmod(path) end
