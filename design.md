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

## Types
By default Eira is type inferred but you can also statically type things
```lua
type Person = {
    age: number = 2 -- defeaulted to 2
    name: string = "" -- defaulted to ""
}
```

To add methods:
```lua
type Dog = {
    name: string = ""

    fn bark() {
        print("${self.name} BARK!!!")
    }
}
```

Interfaces:
```lua
trait Speakable = {
    fn speak(self)
}

local Dog := {
    name: String = "" 
} with Speakable {
    fn speak(self) {
        print("woof")
    }
} with Meta {
    fn toString(self) {
        return self.name
    }

    fn eq(a, b) {
        return a.name == b.name
    }
}
```