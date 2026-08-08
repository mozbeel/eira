local function tag(name)
    return function(list)
        return { name, list }
    end
end

local div = tag("div")
local span = tag("span")
local p = tag("p")
local h1 = tag("h1")
local h2 = tag("h2")
local ul = tag("ul")
local li = tag("li")
local body = tag("body")
local head = tag("head")
local title = tag("title")
local html = tag("html")

local function toString(node)
    local result = "<" .. node[1] .. ">"

    for _, child in ipairs(node[2]) do
        if type(child) == "table" then
            result = result .. toString(child)
        else
            result = result .. tostring(child)
        end
    end

    return result .. "</" .. node[1] .. ">"
end

local html = 
    html {
        head {
            title {
                "Hello World"
            }
        },

        body {
            h1 {
                "My Page"
            },

            p {
                "This is a paragraph."
            },

            div {
                span {
                    "Inside a span"
                }
            },

            h2 {
                "List"
            },

            ul {
                li { "First item" },
                li { "Second item" },
                li { "Third item" },
            }
        }
    }
    

print(toString(html))