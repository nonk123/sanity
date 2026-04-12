-- Lua scripts are used in `sanity` to queue template rendering with custom contexts.
--
-- In this example, we're rendering a personalized wedding invitation for each of our guests.

local guests = { "Bob", "Fred", "Mars", "Luca", "Miguel" };
render("_index.html", "index.html", { default = guests[1] });

for _, name in pairs(guests) do
    render("_page.html", name .. ".html", {
        name = name,
        guests = guests,
    });
end
