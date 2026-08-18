# lfunc.c — Prototypes, closures, upvalues, and to-be-closed variables

> **AI-Generated Documentation**

## Overview

`lfunc.c` is the machinery behind Lua functions and their captured state. It handles allocation and initialization of both C closures (`CClosure`) and Lua closures (`LClosure`), the full lifecycle of upvalues (open, closed, linked, unlinked, closed in bulk), the to-be-closed variable list, and the `Proto` structure used by the compiler and the debugger.

An upvalue in Eira is a two-state object. While a closure's captured variable lives on the stack, the `UpVal` is *open*: its `v.p` pointer references the stack slot directly, and the upvalue is threaded into a per-thread sorted linked list. When the stack slot goes out of scope, the upvalue is *closed*: its value is copied into its own `u.value` slot, `v.p` is redirected to point at `u.value`, and the upvalue is removed from the open list.

To-be-closed (`<close>`) variables use a separate linked list threaded through the `tbclist` field of `StackValue` entries. Each node stores a `delta` (unsigned short) giving the distance to the previous entry; when the gap exceeds `USHRT_MAX`, dummy nodes with `delta == 0` are inserted.

## Key Types / Macros

| Identifier | Purpose |
|---|---|
| `LClosure` | Lua closure: a `Proto *p` plus an array of `UpVal *upvals[]`. |
| `CClosure` | C closure: a `lua_CFunction f` plus an array of `TValue upvalue[]`. |
| `UpVal` | Upvalue object. When open, `v.p` points into the stack and `u.open` provides list links. When closed, `v.p` points to `u.value`. |
| `Proto` | Function prototype: bytecode, constants, child protos, upvalue descriptors, debug info, and flags (`PF_VAHID`, `PF_VATAB`, `PF_FIXED`). |
| `StackValue` | Union of `TValue` and a `tbclist` variant with a `delta` field for the to-be-closed list. |
| `upisopen(up)` | True when the upvalue's value lives on the stack (`v.p != &u.value`). |
| `uplevel(up)` | Casts the open upvalue's stack pointer to `StkId`. |
| `CLOSEKTOP` | Special status that preserves the stack top when closing upvalues. |

## Functions

### `luaF_newCclosure(lua_State *L, int nupvals)`

Allocates a `CClosure` with `nupvals` upvalue slots. The upvalue values are left uninitialized — the caller is responsible for filling them in. Returns the closure pointer.

### `luaF_newLclosure(lua_State *L, int nupvals)`

Allocates an `LClosure` with `nupvals` upvalue pointers, all initialized to `NULL`. The `Proto *p` is also set to `NULL`; the compiler fills it in later. Returns the closure pointer.

### `luaF_initupvals(lua_State *L, LClosure *cl)`

Creates fresh *closed* upvalues for every slot in the closure, each holding `nil`. Registers a write barrier for each new `UpVal` so the GC can see it.

### `newupval(lua_State *L, StkId level, UpVal **prev)` (static)

Creates an open `UpVal` whose value lives at stack slot `level`. Links it into the thread's open-upvalue list after `*prev`. If the thread isn't already in the global `twups` list, inserts it.

### `luaF_findupval(lua_State *L, StkId level)`

Searches the thread's open-upvalue list (sorted by decreasing stack address) for an upvalue at `level`. Reuses an existing one or calls `newupval` to create a new entry.

### `callclosemethod(lua_State *L, TValue *obj, TValue *err, int yy)` (static)

Calls the `__close` metamethod of `obj`, passing it as `self` and optionally the error object as a second argument. Uses `luaD_call` when `yy` is true (yieldable) or `luaD_callnoyield` otherwise.

### `checkclosemth(lua_State *L, StkId level)` (static)

Verifies that the value at `level` has a `__close` metamethod. If not, raises an error naming the offending local variable (looked up via `luaG_findlocal`).

### `prepcallclosemth(lua_State *L, StkId level, TStatus status, int yy)` (static)

Positions the to-be-closed value and (for error statuses) the error object on the stack, then calls `callclosemethod`. For `LUA_OK` / `CLOSEKTOP`, `errobj` is `NULL`.

### `luaF_newtbcupval(lua_State *L, StkId level)`

Registers `level` as a to-be-closed variable. False values are skipped (they don't need closing). Inserts dummy nodes (delta 0) when the gap from the previous entry exceeds `MAXDELTA` (`USHRT_MAX`). Calls `checkclosemth` to ensure the value is closable.

### `luaF_unlinkupval(UpVal *uv)`

Removes an open upvalue from its thread's doubly-linked open-upvalue list by patching its neighbors' links.

### `luaF_closeupval(lua_State *L, StkId level)`

Closes every open upvalue at or above `level`: copies the value from the stack into `uv->u.value`, redirects `v.p` to point there, unlinks from the open list, and marks the upvalue black (no gray phase for closed upvalues).

### `poptbclist(lua_State *L)` (static)

Removes the first to-be-closed variable from `tbclist`, also skipping any preceding dummy nodes (delta == 0). Backs up by `MAXDELTA` per dummy.

### `luaF_close(lua_State *L, StkId level, TStatus status, int yy)`

Closes all upvalues (via `luaF_closeupval`) and all to-be-closed variables down to `level`, invoking their `__close` metamethods via `prepcallclosemth`. Returns the (possibly shifted) stack level. The `yy` parameter controls whether metamethod calls are yieldable.

### `luaF_newproto(lua_State *L)`

Allocates an empty `Proto` with all arrays `NULL` and all sizes `0`, ready for the compiler to populate.

### `luaF_protosize(Proto *p)`

Computes the total memory footprint of a `Proto` (header + all owned arrays). Prototypes with `PF_FIXED` skip their code/line arrays, which live in fixed (non-GC-managed) memory.

### `luaF_freeproto(lua_State *L, Proto *f)`

Frees a `Proto` and all its dynamically allocated arrays. `PF_FIXED` protos skip their code and lineinfo arrays.

### `luaF_getlocalname(const Proto *f, int local_number, int pc)`

Returns the name of the `local_number`-th (1-based) local variable active at program point `pc`, or `NULL` if none exists. Iterates the `locvars` array, counting variables whose `startpc <= pc < endpc`.
