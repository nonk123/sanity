---@meta

---@param template string
---@param outpath string
---@param context table
---@return nil
function render(template, outpath, context) end

---@param path string
---@return any
function json(path) end

---@param path string
---@return string
function read(path) end

---@param name string
---@param value any
---@return nil
function inject(name, value) end
