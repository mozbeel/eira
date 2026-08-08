# Design
This is a complete redesign of lua but still keeping the core functionality

## Globals
removed

## Variables
immutable by default:
```lua
local x = 5

```

mutabilty through adding a `!`:
```lua
local x! = 5

x! = x! + 1

x! += 1
x! -= 1
x! *= 1
x! /= 1
x! %= 1
```

## Functions
```lua
fn add(x, y) 
    return x + y
end
```

## Nil removal
Now you need to use the Option(t) type 


## Type annotations
By default Eira is type inferred but you can also statically type things
```lua
type Person = {
    age: Number = 2, -- defeaulted to 2
    name: String = "", -- defaulted to ""
}
```

To add methods:
```lua
type Dog = {
    name: String = "",
} with
    fn speak(self)

    end
end
```

Vararg types:

Since lua tables are just maps I decided to add vararg types in those tables
```lua
type List(a) = {
    ...[Number] = a
}

-- examples
list[1]
list[2]
list[3]

type Map(k, v) = {
    ...[k] = v
}

map["whatever"]
map["yo"]

type SomeOpenRecord(a) = {
    ...
}


```

Interfaces:
```lua
trait Speakable with
    fn speak(self)
end

type Dog = {
    name: String = "",
} with Speakable
    fn speak(self)
        print("woof")
    end
and Meta
    fn toString(self) {
        return self.name
    }

    fn eq(a, b) {
        return a.name == b.name
    }
end


trait GameObject with
    fn get_position() -> {Number, Number, Number}
end

type Player = {
    position: {Number, Number, Number} = {0, 0, 0}

} with GameObject
    fn get_position() -> {Number, Number, Number} 
        return self.position
    end
end
```

Unions:
```lua
type Shape = [
    Rectangle = {
        a: Number,
        b: Number,
    } 
    or 
    Circle = {
        radius: Number,
    }
] with 
    fn area(self) -> Number 
        match self with
            Rectangle(rect) => 
                return rect.a * rect.b
            or Circle(circle) => 
                return circle.radius
            end
        end
    end
end
```

## Type inference
Structural typing:
```lua
local list = { 1, 2, 3 } -- List(Number)

```

## Lists
```lua
local list = { 1, 2, 3 }

-- public type
type Vec3 = {
    x: Number, -- explicit private field, by default 
    y: Number,
    z: Number,

    return { x, y z }
} with 
    fn new(values: List(Number, 3)) -> self
        Vec3 {
            x = values[1]
            y = values[2]
            z = values[3] 
        }
    end
end

Vec3.new { 0, 0, 0 } -- call without parentheses

-- Actual type:
type List(a, size: Number) = {
    ...[0..size of Number] = a 
}
```


## Visibility
```lua
type Person = {
    pub alive: Bool, -- get and set
    pub(get) age: Number, -- get only
}
```

## Map
```lua
local players = {
    "mozbeel" => Vec3.new { 0, 0, 0 }
}

players["mozbeel"] -- Try(Vec3, _)
```

## Match
```lua
local list = {1, 2, 3}

match list[4] with
    Ok(num) =>
        ...
    or Err(err) =>
        ...
end
```

## Traits
fn to_string_caller(thing: a)
where a has {
    to_string: fn(a) -> String
}
    print(thing.to_string())
end