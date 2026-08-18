# ldblib.c — Eira Debug Library

> **AI-Generated Documentation**

## Overview

This file implements the **debug library** for the Eira Lua 5.5 dialect, exposed as the `debug` table. The debug library provides introspection and manipulation capabilities for the Lua virtual machine: inspecting stack frames, reading and writing locals and upvalues, installing hooks, accessing metatables without metamethod protection, and retrieving registry information. It is the Lua-side interface to the C debug API (`lua_debug`, `lua_getinfo`, `lua_getlocal`, etc.).

The library contains 16 functions covering three main categories: **stack and variable inspection** (`debug.getinfo`, `debug.getlocal`, `debug.setlocal`, `debug.getupvalue`, `debug.setupvalue`, `debug.upvalueid`, `debug.upvaluejoin`), **metatable and userdata access** (`debug.getmetatable`, `debug.setmetatable`, `debug.getuservalue`, `debug.setuservalue`, `debug.getregistry`), and **hook management** (`debug.sethook`, `debug.gethook`). Additionally, `debug.debug` provides an interactive debugger prompt, and `debug.traceback` builds a formatted error traceback.

A key design pattern in this file is the optional first thread argument: most functions accept an optional `thread` parameter as their first argument. When present, the function operates on that thread; when absent, it operates on the current thread. The `getthread` helper manages this convention, and `checkstack` ensures cross-thread stack safety when pushing values onto a thread that may be in any state.

The hook system uses a weak-keyed table stored at `registry[HOOKKEY]` (where `HOOKKEY` is the static string `"_HOOKKEY"`) to map threads to their Lua hook functions. A single C hook function (`hookf`) is installed via `lua_sethook`; it looks up the Lua callback in this registry table and invokes it with the event name and current line.

## Functions

### `checkstack(lua_State *L, lua_State *L1, int n)`

Ensures thread `L1` has room for `n` additional stack values. Only performs the check when `L1` differs from `L` (the calling thread), since `L`'s stack is guaranteed to be safe. Raises a Lua error on `L` if the check fails.

### `db_getregistry(lua_State *L)`

Implements `debug.getregistry()`. Pushes the registry table onto the stack. This gives Lua code direct access to the internal registry, which is otherwise inaccessible.

### `db_getmetatable(lua_State *L)`

Implements `debug.getmetatable(obj)`. Returns the metatable of any value, **ignoring** the `__metatable` guard. Unlike `getmetatable()` from the base library, this never returns the protected `__metatable` field — it always returns the actual metatable or `nil`.

### `db_setmetatable(lua_State *L)`

Implements `debug.setmetatable(obj, table)`. Sets a metatable directly on a value, bypassing the `__metatable` protection. The second argument must be `nil` or a table. Returns the first argument.

### `db_getuservalue(lua_State *L)`

Implements `debug.getuservalue(uv, [n])`. Returns the `n`-th associated value of a userdata (defaulting to 1). Returns the value along with a boolean `true` on success, or `nil` when the argument is not a userdata or the index is out of range.

### `db_setuservalue(lua_State *L)`

Implements `debug.setuservalue(uv, value, [n])`. Writes the `n`-th associated value of a userdata (defaulting to 1). Returns the old value on success, or `nil` if the index is invalid.

### `getthread(lua_State *L, int *arg)`

Helper for functions with an optional first thread argument. If the first stack value is a thread, sets `*arg` to 1 and returns that thread. Otherwise sets `*arg` to 0 and returns `L` itself. This allows callers to uniformly skip the thread argument when accessing their other parameters.

### `settabss(lua_State *L, const char *k, const char *v)`

Utility: pushes a string value `v` and sets it as field `k` on the table at stack top. Used by `db_getinfo` to populate the result table.

### `settabsi(lua_State *L, const char *k, int v)`

Utility: pushes an integer value `v` and sets it as field `k` on the table at stack top. Used by `db_getinfo`.

### `settabsb(lua_State *L, const char *k, int v)`

Utility: pushes a boolean value `v` and sets it as field `k` on the table at stack top. Used by `db_getinfo`.

### `treatstackoption(lua_State *L, lua_State *L1, const char *fname)`

Moves an extra result produced by `lua_getinfo` (either the function object or the active-lines table) from the target thread's stack onto the result table. Handles the case where `L` and `L1` are the same thread (uses `lua_rotate`) or different threads (uses `lua_xmove`).

### `db_getinfo(lua_State *L)`

Implements `debug.getinfo([thread,] level_or_func, [options])`. Dispatches on the option string (default `"flnSrtu"`), calling `lua_getinfo` and collecting the requested fields into a new table. Options include: `S` (source info), `l` (current line), `u` (upvalue/param info), `n` (name info), `r` (transfer info), `t` (tail call info), `L` (active lines), `f` (function object). Passing `>` in the options string causes the function to be pushed on the target stack for `lua_getinfo` to inspect.

```c
if (lua_isfunction(L, arg + 1)) {
  options = lua_pushfstring(L, ">%s", options);
  lua_pushvalue(L, arg + 1);
  lua_xmove(L, L1, 1);
}
```

### `db_getlocal(lua_State *L)`

Implements `debug.getlocal([thread,] level, index)`. When given a stack level, returns the name and value of the `index`-th local at that level. When given a function, returns only the name of the `index`-th local (there is no live value to return for a function prototype).

### `db_setlocal(lua_State *L)`

Implements `debug.setlocal([thread,] level, index, value)`. Assigns `value` to the `index`-th local at the given stack level. Returns the local's name, or `nil` if the index is out of range.

### `auxupvalue(lua_State *L, int get)`

Shared implementation for `getupvalue` and `setupvalue`. When `get` is true, reads the `n`-th upvalue and returns `(name, value)`. When `get` is false, writes the value from the stack and returns `(name)`. Returns 0 (no results) when the upvalue index is invalid.

### `db_getupvalue(lua_State *L)`

Implements `debug.getupvalue(func, index)`. Returns the name and current value of the `index`-th upvalue of the given closure.

### `db_setupvalue(lua_State *L)`

Implements `debug.setupvalue(func, index, value)`. Assigns `value` to the `index`-th upvalue of the given closure and returns the upvalue's name.

### `checkupval(lua_State *L, int argf, int argnup, int *pnup)`

Validates that the upvalue at index `argnup` of the closure at `argf` exists. Returns the upvalue's identity pointer (a stable address shared across closures over the same variable). When `pnup` is non-NULL, stores the validated index there; the pointer is `NULL` for invalid indices.

### `db_upvalueid(lua_State *L)`

Implements `debug.upvalueid(func, index)`. Returns a light userdata that uniquely identifies the given upvalue. Two closures that share the same upvalue (i.e., were created in the same scope) return the same identifier.

### `db_upvaluejoin(lua_State *L)`

Implements `debug.upvaluejoin(f1, n1, f2, n2)`. Makes closure `f1`'s upvalue `n1` share the same storage cell as closure `f2`'s upvalue `n2`. Both closures must be Lua functions (not C functions). After the join, both closures see the same value for the joined variable.

```c
luaL_argcheck(L, !lua_iscfunction(L, 1), 1, "Lua function expected");
luaL_argcheck(L, !lua_iscfunction(L, 3), 3, "Lua function expected");
lua_upvaluejoin(L, 1, n1, 3, n2);
```

### `hookf(lua_State *L, lua_Debug *ar)`

The C hook function installed by `debug.sethook`. On each hook event, it looks up the thread in the registry's hook table to find the corresponding Lua callback, pushes the event name (`"call"`, `"return"`, `"line"`, `"count"`, `"tail call"`) and the current line, and calls the Lua function with two arguments.

### `makemask(const char *smask, int count)`

Converts a hook mask string (characters `c`, `r`, `l`) plus a count into the `LUA_MASK*` bitmask expected by `lua_sethook`. If `count > 0`, the `LUA_MASKCOUNT` bit is also set.

### `unmakemask(int mask, char *smask)`

Inverse of `makemask`: converts a `LUA_MASK*` bitmask back into a string containing `c`, `r`, and/or `l` characters for reporting by `debug.gethook`.

### `db_sethook(lua_State *L)`

Implements `debug.sethook([thread,] hook, mask, [count])`. Stores the Lua hook function in a weak-keyed registry table keyed by thread, installs the C wrapper `hookf` as the actual hook via `lua_sethook`. Passing `nil` as the hook removes the hook. The hook table is created on first use with `__mode = "k"` (weak keys) and is self-referencing (its own metatable).

### `db_gethook(lua_State *L)`

Implements `debug.gethook([thread])`. Returns up to 3 values: the hook function (or `nil` if none, or the string `"external hook"` if a non-Lua hook is installed), the mask string (e.g. `"crl"`), and the count.

### `db_debug(lua_State *L)`

Implements `debug.debug()`. Enters an interactive loop reading commands from stdin (prompt `lua_debug>`), compiling and executing each line in the current environment. Errors are printed to stderr. Typing `cont` or EOF exits the loop.

```c
for (;;) {
  char buffer[250];
  lua_writestringerror("%s", "lua_debug> ");
  if (fgets(buffer, sizeof(buffer), stdin) == NULL ||
      strcmp(buffer, "cont\n") == 0)
    return 0;
```

### `db_traceback(lua_State *L)`

Implements `debug.traceback([thread,] [message,] [level])`. Builds a formatted traceback string. Non-string messages are returned unchanged. Delegates to `luaL_traceback` for the actual formatting.

### `luaopen_debug(lua_State *L)`

Opens the debug library. Creates and returns the `debug` table containing all 16 library functions.
