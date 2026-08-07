--
-- BELOW THIS POINT IS AN AI GENERATED FILE
--

-- Lua syntax showcase

-- comments
-- single-line comment
--[[
  long comment
]]

-- variables
local x = 10
local y = 3.5
local name = "Lua"
local is_ready = true
local nothing = nil

-- multiple assignment
local a, b, c = 1, 2, 3
a, b = b, a

-- arithmetic
local sum = a + b
local diff = a - b
local product = a * b
local quotient = a / b
local int_div = a // b
local remainder = a % b
local power = a ^ b

-- comparisons and logic
local eq = (a == b)
local ne = (a ~= b)
local lt = (a < b)
local gt = (a > b)
local logic = true and false or true
local neg = not false

-- strings
local s1 = "double quoted"
local s2 = 'single quoted'
local s3 = [[long string]]

-- tables
local t = {
  1,
  2,
  3,
  key = "value",
  ["quoted-key"] = 42,
}

t[4] = 4
t.name = "table"

-- function declaration
local function add(m, n)
  return m + n
end

-- function with varargs
local function pack(...)
  return {...}
end

-- method syntax
local obj = {}

function obj:greet(who)
  return "Hello, " .. who
end

-- if / elseif / else
local score = 87
if score >= 90 then
  print("A")
elseif score >= 80 then
  print("B")
else
  print("C")
end

-- while loop
local i = 1
while i <= 3 do
  print("while:", i)
  i = i + 1
end

-- repeat / until loop
local j = 1
repeat
  print("repeat:", j)
  j = j + 1
until j > 3

-- numeric for loop
for n = 1, 5 do
  print("for:", n)
end

-- numeric for with step
for n = 10, 0, -2 do
  print("step for:", n)
end

-- generic for loop
for index, value in ipairs({ "a", "b", "c" }) do
  print(index, value)
end

for key, value in pairs(t) do
  print(key, value)
end

-- function calls
print(add(5, 7))
print(obj:greet("world"))

-- local function returning multiple values
local function divide(x1, x2)
  return x1 / x2, x1 % x2
end

local q, r = divide(10, 3)
print(q, r)

-- closures
local function counter()
  local count = 0
  return function()
    count = count + 1
    return count
  end
end

local c1 = counter()
print(c1())
print(c1())

-- labels and goto
local k = 1
::start::
print("goto:", k)
k = k + 1
if k <= 3 then
  goto start
end

-- block syntax
do
  local temp = "inside block"
  print(temp)
end

-- return values
local function stats(list)
  local total = 0
  for _, v in ipairs(list) do
    total = total + v
  end
  return total, #list
end

local total, count = stats({ 2, 4, 6, 8 })
print(total, count)

-- table iteration and nested table
local config = {
  title = "Example",
  options = {
    debug = true,
    verbose = false,
  }
}

print(config.title)
print(config.options.debug)

-- metatable example
local mt = {
  __index = function(_, key)
    return "missing: " .. tostring(key)
  end
}

setmetatable(config, mt)
print(config.unknown_key)