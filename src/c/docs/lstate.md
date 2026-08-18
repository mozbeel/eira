# lstate.c — Global state and thread lifecycle

> **AI-Generated Documentation**

## Overview

`lstate.c` manages the complete lifecycle of an Eira Lua state: creation (`lua_newstate`), initialization of the main thread, allocation and destruction of coroutines (`lua_newthread` / `luaE_freethread`), and teardown (`lua_close`). It is the only file that constructs the `global_State` structure and wires it to the main `lua_State` (embedded as `global_State.mainth`).

The file also maintains the `CallInfo` doubly-linked list that tracks the active call stack. CallInfos are allocated on demand via `luaE_extendCI` and pruned either fully (`freeCI`) or by half (`luaE_shrinkCI`) to avoid unbounded memory growth from deep call chains.

Garbage-collection debt management is handled by `luaE_setdebt`, which clamps `GCtotalbytes` so it never overflows `MAX_LMEM`. C-stack recursion safety is enforced by `luaE_checkcstack` / `luaE_incCstack`, which raise errors when `nCcalls` exceeds `LUAI_MAXCCALLS`.

Warning and error-reporting helpers (`luaE_warning`, `luaE_warnerror`) round out the file, forwarding messages through the state's configurable warning function.

## Key Types / Macros

| Identifier | Purpose |
|---|---|
| `global_State` | The shared state for all threads: allocator, GC lists/parameters, string table, registry, metatables, tag-method names, and the main thread. |
| `lua_State` | Per-thread state: stack, `CallInfo` chain, open upvalues, hooks, error recovery, `nCcalls`. |
| `LX` | Wrapper block: `lua_State` plus `LUA_EXTRASPACE` bytes of extra storage at the front. |
| `CallInfo` | Doubly-linked call-frame record: function slot, top, saved PC (Lua) or continuation (C), and status flags. |
| `fromstate(L)` | Macro that recovers the `LX*` from a `lua_State*` by subtracting `offsetof(LX, l)`. |
| `completestate(g)` | True when `g->nilvalue` is nil, indicating the state was fully initialized. |

## Functions

### `luaE_setdebt(global_State *g, l_mem debt)`

Sets the GC debt while keeping the total byte count (`GCtotalbytes = real_bytes + debt`) below `MAX_LMEM`. If the requested debt would cause overflow, it is clamped so that `GCtotalbytes == MAX_LMEM`.

### `luaE_extendCI(lua_State *L, int err)`

Allocates a new `CallInfo` via `luaM_reallocvector` and splices it into the doubly-linked `ci` list right after `L->ci`. If `err` is set and allocation fails, raises `LUA_ERRMEM`; otherwise returns `NULL`. Initializes `u.l.trap = 0` and increments `L->nci`.

### `freeCI(lua_State *L)` (static)

Frees every `CallInfo` after `L->ci`, keeping only the active one. Decrements `L->nci` for each freed node.

### `luaE_shrinkCI(lua_State *L)`

Frees **half** of the free `CallInfo` chain (keeping the first free entry), so deep call stacks shrink geometrically rather than all at once.

### `luaE_checkcstack(lua_State *L)`

Guards against C-stack overflow: if `getCcalls(L) == LUAI_MAXCCALLS` raises `"C stack overflow"`; if the count is significantly higher (already handling an overflow), calls `luaD_errerr` instead.

### `luaE_incCstack(lua_State *L)`

Increments `L->nCcalls` and calls `luaE_checkcstack` when the limit is reached.

### `resetCI(lua_State *L)` (static)

Resets a thread to its base C frame: `ci` points to `base_ci`, the function slot is nil, `top` is `func + 1 + LUA_MINSTACK`, callstatus is `CIST_C`, and `status` is `LUA_OK`.

### `stack_init(lua_State *L1, lua_State *L)` (static)

Allocates `BASIC_STACK_SIZE + EXTRA_STACK` slots (all nil), sets `stack_last`, calls `resetCI`, and sets `top` to `stack + 1`.

### `freestack(lua_State *L)` (static)

Frees the `CallInfo` list (via `freeCI`) and the stack array. No-ops if `stack.p` is `NULL`.

### `init_registry(lua_State *L, global_State *g)` (static)

Creates the registry table with three predefined entries:
- Index `1` → `false`
- `LUA_RIDX_MAINTHREAD` → the main thread `L`
- `LUA_RIDX_GLOBALS` → a new empty table (the globals)

### `f_luaopen(lua_State *L, void *ud)` (static)

Protected frame that completes state construction: initializes the stack, registry, strings (`luaS_init`), type metamethods (`luaT_init`), and the lexer (`luaX_init`). Enables GC and sets `g->nilvalue` to nil to mark the state as complete.

### `preinit_thread(lua_State *L, global_State *g)` (static)

Zero-alloc thread setup: sets the back-pointer to `global_State`, NULLs out stack/ci/hook fields, sets `twups = L` (self-loop = no upvalues), `status = LUA_OK`, and resets the base `CallInfo`.

### `luaE_threadsize(lua_State *L)`

Returns the total memory footprint of a thread: `sizeof(LX)` + CallInfo count × `sizeof(CallInfo)` + stack slots × `sizeof(StackValue)`. Used by the GC to account for thread memory.

### `close_state(lua_State *L)` (static)

Tears down a state: for a complete state, resets the base frame, closes upvalues, empties the stack (for finalizers), collects all objects, frees the string table, stack, and the `global_State` block. For a partial state, just frees objects.

### `lua_newthread(lua_State *L)` (LUA_API)

Creates a new coroutine: allocates an `LX` via GC, anchors it on the creator's stack, copies hooks and extra space from the main thread, and initializes the stack. Triggers a GC check first.

### `luaE_freethread(lua_State *L, lua_State *L1)`

Releases a thread: closes open upvalues, frees the stack and CallInfos, then frees the `LX` block.

### `luaE_resetthread(lua_State *L, TStatus status)`

Restores a thread to a clean resumable state: resets the base frame, closes to-be-closed variables (propagating `status`), sets the error object if needed, and shrinks the stack to minimal size.

### `lua_closethread(lua_State *L, lua_State *from)` (LUA_API)

API entry point that resets thread `L`. When `L` is closing itself, it also unwinds to the base level via `luaD_throwbaselevel`. Returns the API-compatible status.

### `lua_newstate(lua_Alloc f, void *ud, unsigned seed)` (LUA_API)

Creates a whole Lua state: allocates the `global_State` via the user-supplied allocator `f`, wires the main thread, initializes all GC fields (lists, parameters, debt), and runs `f_luaopen` under protection. Frees the partial state on failure.

### `lua_close(lua_State *L)` (LUA_API)

Closes a Lua state. Only the main thread may be closed; delegates to `close_state`.

### `luaE_warning(lua_State *L, const char *msg, int tocont)`

Forwards a warning message to the state's warning function (`G(L)->warnf`). The `tocont` flag marks multi-part messages.

### `luaE_warnerror(lua_State *L, const char *where)`

Emits an `"error in <where> (<msg>)"` warning sequence. Extracts the error message from the top of the stack (or `"error object is not a string"` if it isn't).
