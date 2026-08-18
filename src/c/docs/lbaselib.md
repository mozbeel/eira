# lbaselib.c — Eira Basic Library

> **AI-Generated Documentation**

## Overview

This file implements the **basic library** for the Eira Lua 5.5 dialect. The basic library is automatically loaded into the global table and provides fundamental functions that are available in every Lua program without requiring any `require` call. It is opened via `luaopen_base`, which installs all functions into `_G` and sets the `_G` and `_VERSION` global variables.

The basic library covers a wide range of core language operations: value type inspection (`type`, `tostring`, `tonumber`), error handling (`error`, `assert`, `pcall`, `xpcall`), table iteration (`pairs`, `ipairs`, `next`), metatable access (`getmetatable`, `setmetatable`), raw table operations (`rawget`, `rawset`, `rawequal`, `rawlen`), code loading (`load`, `loadfile`, `dofile`), variable argument handling (`select`), garbage collector control (`collectgarbage`), printing (`print`), and diagnostic warnings (`warn`).

Several functions in this file use C continuation functions to support coroutine-safe calling conventions, particularly `pcall`, `xpcall`, `dofile`, and `pairs` when the `__pairs` metamethod is present. The `load` function supports both string input and a generic reader callback pattern, making it possible to compile code from arbitrary streaming sources.

## Functions

### `luaB_print(lua_State *L)`

Implements the global `print()` function. Converts each argument to a string using `luaL_tolstring` (which honors `__tostring` metamethods), writes them separated by tab characters, and appends a newline. Returns 0 values.

### `luaB_warn(lua_State *L)`

Implements the global `warn()` function. Validates that all arguments are strings first, then emits the warning in chunks — all but the last are marked as continuation pieces via `lua_warning(L, ..., 1)`, and the final chunk is marked as complete with `0`.

### `b_str2int(const char *s, unsigned base, lua_Integer *pn)`

Internal parser for `tonumber` with an explicit base. Skips leading whitespace, consumes an optional sign, then reads digits valid in the given `base` (2–36). Returns a pointer past the last valid character, or `NULL` if the input is not a valid numeral.

```c
static const char *b_str2int (const char *s, unsigned base, lua_Integer *pn) {
  lua_Unsigned n = 0;
  int neg = 0;
  s += strspn(s, SPACECHARS);
```

### `luaB_tonumber(lua_State *L)`

Implements the global `tonumber()`. With no base argument, converts strings to numbers (passing through values already numeric). With an explicit base (2–36), the entire string must be a valid numeral in that base. Returns the converted number or `nil` on failure.

### `luaB_error(lua_State *L)`

Implements the global `error()`. Raises a Lua error with the given message. When the message is a string and `level > 0`, it prefixes the message with source location information from the corresponding stack level using `luaL_where`.

### `luaB_getmetatable(lua_State *L)`

Implements the global `getmetatable()`. Returns the `__metatable` field if present (protecting the real table from external access), otherwise returns the raw metatable, or `nil` if none exists.

### `luaB_setmetatable(lua_State *L)`

Implements the global `setmetatable()`. Sets a new metatable on a table. Refuses the operation and raises an error if the existing metatable has a non-nil `__metatable` guard field.

### `luaB_rawequal(lua_State *L)`

Implements the global `rawequal()`. Performs an identity comparison between two values without invoking the `__eq` metamethod. Returns a boolean.

### `luaB_rawlen(lua_State *L)`

Implements the global `rawlen()`. Returns the raw length of a table or string, bypassing the `__len` metamethod.

### `luaB_rawget(lua_State *L)`

Implements the global `rawget()`. Retrieves a value from a table using raw access, bypassing `__index` metamethods.

### `luaB_rawset(lua_State *L)`

Implements the global `rawset()`. Assigns a value to a table using raw access, bypassing `__newindex` metamethods.

### `pushmode(lua_State *L, int oldmode)`

Helper for `collectgarbage`. Converts the integer GC mode returned by `lua_gc` into the string `"incremental"` or `"generational"`, or pushes `nil` when the call was invalid (returned `-1`).

### `luaB_collectgarbage(lua_State *L)`

Implements the global `collectgarbage()`. Maps the option string (e.g. `"stop"`, `"restart"`, `"collect"`, `"count"`, `"step"`, `"isrunning"`, `"generational"`, `"incremental"`, `"param"`) to the corresponding `lua_gc` operation. Returns vary by option. Calls that occur inside a finalizer are rejected with a fail result.

```c
static const char *const opts[] = {"stop", "restart", "collect",
  "count", "step", "isrunning", "generational", "incremental",
  "param", NULL};
```

### `luaB_type(lua_State *L)`

Implements the global `type()`. Returns the type name string of its argument. Raises an error if no argument is provided.

### `luaB_next(lua_State *L)`

Implements the global `next()`. Performs a single raw iteration step over a table, returning the next key-value pair. Returns `nil` when the table is exhausted. Used internally by `pairs`.

### `pairscont(lua_State *L, int status, lua_KContext k)`

Continuation function for `pairs` when the `__pairs` metamethod is invoked. Simply returns all 4 results that the metamethod already pushed.

### `luaB_pairs(lua_State *L)`

Implements the global `pairs()`. Honors the `__pairs` metamethod if present (returns its 4 results via a continuation-safe call). Otherwise returns `(next, table, nil, nil)` for raw iteration.

### `ipairsaux(lua_State *L)`

Step function for the `ipairs` iterator. Advances the numeric index and returns `(index, value)`, stopping at the first `nil` slot.

### `luaB_ipairs(lua_State *L)`

Implements the global `ipairs()`. Returns `(ipairsaux, object, 0)` as the iterator triplet. Iteration ends at the first nil-valued integer key.

### `load_aux(lua_State *L, int status, int envidx)`

Shared completion logic for `load` and `loadfile`. On success, optionally binds the first upvalue to the given environment. On failure, returns `(nil, error_message)`.

### `getMode(lua_State *L, int idx)`

Validates the `mode` argument of `load`/`loadfile`. Rejects `"B"` mode because this dialect cannot compile chunks into fixed buffers. Returns the mode string or `NULL`.

### `luaB_loadfile(lua_State *L)`

Implements the global `loadfile()`. Compiles the given file (or stdin when no filename is given) and optionally sets the environment. Uses `luaL_loadfilex` and delegates finalization to `load_aux`.

### `generic_reader(lua_State *L, void *ud, size_t *size)`

Reader callback for `load` when given a function argument. Calls the function to fetch the next chunk of source code; `nil` signals end of input. Non-string returns raise an error. Uses a `firstcall` flag passed via `ud` to manage stack state across successive calls.

```c
static const char *generic_reader (lua_State *L, void *ud, size_t *size) {
  int *firstcall = cast(int *, ud);
  luaL_checkstack(L, 2, "too many nested functions");
```

### `luaB_load(lua_State *L)`

Implements the global `load()`. When given a string, compiles it directly with `luaL_loadbufferx`. When given a function, uses `generic_reader` as a streaming reader. Supports optional chunk name, mode, and environment parameters.

### `dofilecont(lua_State *L, int d1, lua_KContext d2)`

Continuation for `dofile`. Returns every stack value except the chunk function itself (offset of 1).

### `luaB_dofile(lua_State *L)`

Implements the global `dofile()`. Loads and immediately executes a file, returning all values it produces. Errors propagate. Uses `lua_callk` with `dofilecont` to support yielding from within the executed chunk.

### `luaB_assert(lua_State *L)`

Implements the global `assert()`. Returns all arguments when the first argument is truthy; otherwise raises an error with the message `"assertion failed!"` or a user-supplied message.

### `luaB_select(lua_State *L)`

Implements the global `select()`. When the first argument is the string `"#"`, returns the count of remaining arguments. Otherwise returns all arguments starting from the given (possibly negative) index.

### `finishpcall(lua_State *L, int status, lua_KContext extra)`

Shared completion logic for `pcall` and `xpcall`. On error, returns `(false, error_message)`. On success, returns all results above the pre-pushed boolean marker, skipping `extra` stack values.

### `luaB_pcall(lua_State *L)`

Implements the global `pcall()`. Pushes a `true` marker under the function, performs a protected call via `lua_pcallk`, and strips the marker on success or returns `(false, message)` on error. Supports yielding across the protected call.

### `luaB_xpcall(lua_State *L)`

Implements the global `xpcall()`. Like `pcall` but with a dedicated message handler. The stack is rotated so the handler sits below the call arguments, and `finishpcall` skips 2 values when returning results.

```c
lua_rotate(L, 3, 2);  /* move them below function's arguments */
status = lua_pcallk(L, n - 2, LUA_MULTRET, 2, 2, finishpcall);
```

### `luaB_tostring(lua_State *L)`

Implements the global `tostring()`. Returns the string representation of its argument, honoring `__tostring` metamethods via `luaL_tolstring`.

### `luaopen_base(lua_State *L)`

Opens the base library. Registers all base functions into the global table, then sets `_G` (the global table itself) and `_VERSION` (the Lua version string) as globals.
