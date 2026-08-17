fn add(x, y)
    return  x + y
end

print(add(1, 2))

fn map(list, f)
    local result = {}

    for key, value in pairs(list) do
        result[key] = f(value)
    end

    return result
end

local squared = map({1, 2, 3}, fn(x)
    return x * x
end)

print(squared)