# lapi.c — Public C API implementation for the Lua/Eira virtual machine

> **AI-Generated Documentation**

## Overview

`lapi.c` implements the entire public C API declared in `lua.h` on top of the Lua/Eira core internals. Every function exposed to embedding host code—stack manipulation, type queries, value push/pop, table access, function calls, garbage-collector control, and more—is defined here. The file is the sole bridge between the external world and the VM's internal representation (`TValue`, `StkId`, `CallInfo`, `global_State`).

The central design pattern is: translate an integer stack index into a `TValue*` or `StkId` via `index2value` / `index2stack`, perform the operation under `lua_lock` / `lua_unlock`, and return a result that fits the `lua.h` contract. Pseudo-indices (`LUA_REGISTRYINDEX` and upvalue slots) are handled as special branches inside `index2value`, keeping the public API clean of internal details.

API safety is enforced through `api_check`, `api_checkpop`, and `api_incr_top` macros (defined in `lapi.h`). These verify stack bounds, argument counts, and result-slot availability at every entry point. The file also contains the continuation-capable call/protected-call machinery (`lua_callk`, `lua_pcallk`), the `lua_load` chunk reader, and the garbage-collection dispatcher (`lua_gc`).

As an Eira 5.5-dialect project, this file preserves standard Lua 5.4+ conventions (to-be-closed variables, generational GC modes, `_ENV` upvalue bootstrapping) while remaining API-compatible with embedding code written against standard Lua.

## Key Types / Macros

| Name | Location | Purpose |
|---|---|---|
| `isvalid(L, o)` | `lapi.c:48` | Tests whether a `TValue*` resolves to a non-nil slot |
| `ispseudo(i)` | `lapi.c:52` | True when index `i` is a pseudo-index (≤ `LUA_REGISTRYINDEX`) |
| `isupvalue(i)` | `lapi.c:55` | True when index `i` refers to a closure upvalue |
| `api_incr_top(L)` | `lapi.h:26` | Increments `L->top.p` and asserts no stack overflow |
| `api_checkpop(L, n)` | `lapi.h:60` | Asserts the stack has `n` poppable elements above `tbclist` |
| `adjustresults(L, nres)` | `lapi.h:45` | Widens `ci->top.p` when a multi-return call overshoots |
| `checkresults(L, na, nr)` | `lapi.c:1160` | Validates the requested result count for a call |
| `LUA_API` | `lua.h` | Export macro for every public API function |
| `lua_lock` / `lua_unlock` | `lapi.h:33` | No-ops in single-thread builds; mutex wrappers in threads |
| `G(L)` | `lstate.h:375` | Shorthand for `L->l_G`, the shared `global_State` |
| `s2v(o)` | `lobject.h` | Converts `StkId` (stack slot pointer) to `TValue*` |

## Functions

### index2value(lua_State *L, int idx)
(`lapi.c:62`) Converts any acceptable stack index (positive, negative, or pseudo) to a `TValue*`. Positive indices index from `ci->func + 1`; negative indices index from `L->top`; `LUA_REGISTRYINDEX` returns `G(L)->l_registry`; upvalue pseudo-indices compute `LUA_REGISTRYINDEX - idx` and extract from the current C closure. Out-of-range slots return the sentinel `G(L)->nilvalue`.

### index2stack(lua_State *L, int idx)
(`lapi.c:97`) Converts a *real* (non-pseudo) stack index to an `StkId`. Used by operations that need to write directly into a stack slot (e.g., `lua_closeslot`). Positive and negative indices follow the same arithmetic as `index2value` but return a `StackValue*` pointer.

### lua_checkstack(lua_State *L, int n)
(`lapi.c:116`) Ensures the stack has room for `n` additional values. Delegates to `luaD_growstack` when the current `stack_last` is insufficient, then raises `ci->top.p` to expose the new slots. Returns 1 on success, 0 on failure.

### lua_xmove(lua_State *from, lua_State *to, int n)
(`lapi.c:136`) Moves `n` top values from `from` to `to`. Both states must share the same `global_State`. Pops the values from `from` and copies them with GC write-barrier-free assignment (same universe).

### lua_atpanic(lua_State *L, lua_CFunction panicf)
(`lapi.c:154`) Installs a panic function called on unprotected errors. Returns the previously installed function.

### lua_version(lua_State *L)
(`lapi.c:165`) Returns `LUA_VERSION_NUM`. The `L` parameter is unused.

### lua_absindex(lua_State *L, int idx)
(`lapi.c:180`) Converts an acceptable index to an absolute (positive) index. Positive and pseudo-indices are returned as-is; negative indices are converted using the distance from `func + 1` to `top`.

### lua_gettop(lua_State *L)
(`lapi.c:189`) Returns the number of values in the current function's stack frame: `L->top - (L->ci->func + 1)`.

### lua_settop(lua_State *L, int idx)
(`lapi.c:197`) Sets the stack top to absolute index `idx`. Positive indices pad new slots with nil; negative indices shrink. When shrinking past a to-be-closed slot, its `__close` metamethod runs first via `luaF_close`.

### lua_closeslot(lua_State *L, int idx)
(`lapi.c:228`) Explicitly runs the `__close` metamethod on the to-be-closed variable at `idx`. The slot must be the topmost TBC slot; it is then set to nil.

### reverse(lua_State *L, StkId from, StkId to)
(`lapi.c:246`) Reverses the stack segment `[from, to]` in-place. Helper for `lua_rotate`.

### lua_rotate(lua_State *L, int idx, int n)
(`lapi.c:260`) Rotates the stack segment starting at `idx` by `n` positions. Positive `n` rotates left; negative rotates right. Uses three calls to `reverse`. Refuses to move to-be-closed slots.

### lua_copy(lua_State *L, int fromidx, int toidx)
(`lapi.c:277`) Copies the value at `fromidx` to `toidx`. Emits a GC write barrier when the destination is an upvalue.

### lua_pushvalue(lua_State *L, int idx)
(`lapi.c:293`) Pushes a copy of the value at `idx` onto the stack top.

### lua_type(lua_State *L, int idx)
(`lapi.c:309`) Returns the type tag of the value at `idx`, or `LUA_TNONE` for invalid indices.

### lua_typename(lua_State *L, int t)
(`lapi.c:316`) Returns the printable name of type tag `t` (e.g., `"number"`).

### lua_iscfunction(lua_State *L, int idx)
(`lapi.c:324`) Returns true if the value at `idx` is a light C function or a C closure.

### lua_isinteger(lua_State *L, int idx)
(`lapi.c:331`) Returns true if the value is an integer (no coercion attempted).

### lua_isnumber(lua_State *L, int idx)
(`lapi.c:339`) Returns true if the value is a number or a string convertible to number. No stack modification.

### lua_isstring(lua_State *L, int idx)
(`lapi.c:347`) Returns true if the value is a string or convertible to one (numbers are convertible).

### lua_isuserdata(lua_State *L, int idx)
(`lapi.c:354`) Returns true if the value is full or light userdata.

### lua_rawequal(lua_State *L, int index1, int index2)
(`lapi.c:362`) Raw (no metamethods) equality test between two stack values. Returns 0 if either index is invalid.

### lua_arith(lua_State *L, int op)
(`lapi.c:371`) Performs an arithmetic or bitwise operation on the top 1–2 stack values. Unary ops (`LUA_OPUNM`, `LUA_OPBNOT`) duplicate the top value as a fake second operand. Result replaces the operands at `top - 2`.

### lua_compare(lua_State *L, int index1, int index2, int op)
(`lapi.c:390`) Compares values at two indices using `LUA_OPEQ`, `LUA_OPLT`, or `LUA_OPLE`, honoring metamethods. Returns 0 for invalid indices or non-matching comparison.

### lua_numbertocstring(lua_State *L, int idx, char *buff)
(`lapi.c:412`) Writes the string form of the number at `idx` into `buff` and returns its length. Returns 0 if the value is not a number.

### lua_stringtonumber(lua_State *L, const char *s)
(`lapi.c:426`) Parses `s` as a number and pushes the result. Returns the number of characters consumed (0 on failure).

### lua_tonumberx(lua_State *L, int idx, int *pisnum)
(`lapi.c:436`) Returns the float value at `idx`, writing conversion success to `*pisnum`. Returns 0 on failure without raising an error.

### lua_tointegerx(lua_State *L, int idx, int *pisnum)
(`lapi.c:448`) Returns the integer value at `idx`, writing conversion success to `*pisnum`.

### lua_toboolean(lua_State *L, int idx)
(`lapi.c:460`) Returns the boolean truth of the value (only `nil` and `false` are false).

### lua_tolstring(lua_State *L, int idx, size_t *len)
(`lapi.c:469`) Returns a pointer to the string at `idx`, converting numbers in-place. The number→string conversion may trigger GC and reallocate the stack, so `idx` is re-resolved afterward. Returns NULL for non-convertible types.

### lua_rawlen(lua_State *L, int idx)
(`lapi.c:495`) Returns the raw length of the value (strings, tables, userdata) without invoking `__len`. For tables, delegates to `luaH_getn`.

### lua_tocfunction(lua_State *L, int idx)
(`lapi.c:515`) Returns the C function pointer of a light C function or C closure. Returns NULL for other types.

### lua_touserdata(lua_State *L, int idx)
(`lapi.c:537`) Returns the memory pointer of a full or light userdata, or NULL.

### lua_tothread(lua_State *L, int idx)
(`lapi.c:544`) Returns the `lua_State*` of a thread value, or NULL.

### lua_topointer(lua_State *L, int idx)
(`lapi.c:557`) Returns an opaque pointer identifying the internal object (userdata block, function address, or GC object pointer). ISO C workaround: light C function pointers are cast through `size_t`.

### lua_pushnil(lua_State *L)
(`lapi.c:580`) Pushes `nil`.

### lua_pushnumber(lua_State *L, lua_Number n)
(`lapi.c:589`) Pushes a float `n`.

### lua_pushinteger(lua_State *L, lua_Integer n)
(`lapi.c:598`) Pushes an integer `n`.

### lua_pushlstring(lua_State *L, const char *s, size_t len)
(`lapi.c:611`) Pushes a string of `len` bytes. When `len == 0`, `s` may be NULL; an empty string is created via `luaS_new`. Returns the internal copy's pointer. Triggers a GC check.

### lua_pushexternalstring(lua_State *L, const char *s, size_t len, lua_Alloc falloc, void *ud)
(`lapi.c:626`) Pushes a string whose memory is externally owned. Lua takes ownership and frees via `falloc`/`ud` when collected. The string must end with `'\0'`. An Eira extension not present in standard Lua.

### lua_pushstring(lua_State *L, const char *s)
(`lapi.c:643`) Pushes a null-terminated C string (or `nil` if `s` is NULL). Returns the pointer to Lua's internal copy.

### lua_pushvfstring(lua_State *L, const char *fmt, va_list argp)
(`lapi.c:662`) Formats `fmt` with `argp` and pushes the result string.

### lua_pushfstring(lua_State *L, const char *fmt, ...)
(`lapi.c:675`) Varargs wrapper around `lua_pushvfstring`.

### lua_pushcclosure(lua_State *L, lua_CFunction fn, int n)
(`lapi.c:689`) Pops `n` upvalues and pushes a C closure. When `n == 0`, pushes a plain light C function. The new closure is white (GC-clean), so no write barrier is needed for the upvalue copies.

### lua_pushboolean(lua_State *L, int b)
(`lapi.c:717`) Pushes boolean `b` (0 → false, non-zero → true).

### lua_pushlightuserdata(lua_State *L, void *p)
(`lapi.c:729`) Pushes a light userdata wrapping pointer `p`. No allocation or `__gc`.

### lua_pushthread(lua_State *L)
(`lapi.c:739`) Pushes the state's own thread value. Returns 1 if `L` is the main thread.

### auxgetstr(lua_State *L, const TValue *t, const char *k)
(`lapi.c:757`) Core of `lua_getfield`/`lua_getglobal`: performs a fast table lookup via `luaH_getstr`, falling back to the full `__index` metamethod chain on miss.

### getGlobalTable(lua_State *L, TValue *gt)
(`lapi.c:778`) Resolves the global table from `LUA_RIDX_GLOBALS` in the registry. Asserts it exists and is a table.

### lua_getglobal(lua_State *L, const char *name)
(`lapi.c:788`) Pushes `_G[name]` and returns its type.

### lua_gettable(lua_State *L, int idx)
(`lapi.c:798`) Pops the key and pushes `t[key]`, honoring `__index`. Returns the result type.

### lua_getfield(lua_State *L, int idx, const char *k)
(`lapi.c:814`) Pushes `t[k]` for the table at `idx` (string key, `__index` honored).

### lua_geti(lua_State *L, int idx, lua_Integer n)
(`lapi.c:822`) Pushes `t[n]` for the table at `idx` (integer key, `__index` honored).

### finishrawget(lua_State *L, lu_byte tag)
(`lapi.c:841`) Common tail for raw-get functions: replaces empty results with nil, bumps top, returns type tag.

### gettable(lua_State *L, int idx)
(`lapi.c:851`) Resolves the table at `idx`, raising `"table expected"` if it is not one.

### lua_rawget(lua_State *L, int idx)
(`lapi.c:860`) Raw table lookup (no metamethods). Pops the key, pushes the result.

### lua_rawgeti(lua_State *L, int idx, lua_Integer n)
(`lapi.c:874`) Raw integer-keyed table lookup. Uses `luaH_fastgeti`.

### lua_rawgetp(lua_State *L, int idx, const void *p)
(`lapi.c:886`) Raw pointer-keyed table lookup (pointer treated as light userdata).

### lua_createtable(lua_State *L, int narray, int nrec)
(`lapi.c:898`) Creates an empty table pre-sized for `narray` array entries and `nrec` hash entries.

### lua_getmetatable(lua_State *L, int objindex)
(`lapi.c:914`) Pushes the metatable of the value at `objindex`. For tables/userdata, reads the object's `metatable` field; for other types, reads `G(L)->mt[type]`. Returns 1 if a metatable exists, 0 otherwise.

### lua_getiuservalue(lua_State *L, int idx, int n)
(`lapi.c:943`) Pushes the `n`-th (1-based) associated userdata value of the full userdata at `idx`. Returns `LUA_TNONE` when `n` is out of range.

### auxsetstr(lua_State *L, const TValue *t, const char *k)
(`lapi.c:973`) Core of `lua_setfield`/`lua_setglobal`: performs a fast table set via `luaH_psetstr`, falling back to the full `__newindex` chain. Unlocks `L` before returning (callers are responsible for locking).

### lua_setglobal(lua_State *L, const char *name)
(`lapi.c:993`) Pops the top value and assigns `_G[name] = v`.

### lua_settable(lua_State *L, int idx)
(`lapi.c:1003`) Pops key and value, performs `t[key] = value` with `__newindex` support.

### lua_setfield(lua_State *L, int idx, const char *k)
(`lapi.c:1020`) Pops the top value and assigns `t[name] = v`.

### lua_seti(lua_State *L, int idx, lua_Integer n)
(`lapi.c:1027`) Pops the top value and assigns `t[n] = v`.

### aux_rawset(lua_State *L, int idx, TValue *key, int n)
(`lapi.c:1049`) Common tail for raw-set functions: performs a raw `luaH_set`, invalidates the TM cache, emits a GC barrier, and pops `n` values.

### lua_rawset(lua_State *L, int idx)
(`lapi.c:1063`) Raw assignment `t[key] = value`, popping key and value.

### lua_rawsetp(lua_State *L, int idx, const void *p)
(`lapi.c:1070`) Raw pointer-keyed assignment into table at `idx`.

### lua_rawseti(lua_State *L, int idx, lua_Integer n)
(`lapi.c:1078`) Raw integer-keyed assignment `t[n] = value`.

### lua_setmetatable(lua_State *L, int objindex)
(`lapi.c:1093`) Pops a table (or nil) and sets it as the metatable of the value at `objindex`. For tables/userdata, emits a barrier and registers for finalization. For basic types, sets `G(L)->mt[type]`. Always returns 1.

### lua_setiuservalue(lua_State *L, int idx, int n)
(`lapi.c:1135`) Pops a value and stores it as the `n`-th associated userdata value. Returns 0 if `n` is out of range, 1 on success. Emits a GC barrier.

### lua_callk(lua_State *L, int nargs, int nresults, lua_KContext ctx, lua_KFunction k)
(`lapi.c:1171`) Calls the function `nargs` slots below the top, leaving `nresults` results. When a continuation `k` is given and the thread is yieldable, the continuation is saved in `ci->u.c` so that yielding later resumes through `k`. Not protected—errors propagate via longjmp.

### f_call(lua_State *L, void *ud)
(`lapi.c:1208`) Protected-call adapter: performs `luaD_callnoyield` from inside `luaD_pcall` so errors are caught.

### lua_pcallk(lua_State *L, int nargs, int nresults, int errfunc, lua_KContext ctx, lua_KFunction k)
(`lapi.c:1219`) Protected version of `lua_callk`. On error, runs the handler at `errfunc` and returns an `LUA_ERR*` code. When a continuation is given, protection is deferred to the resuming thread via `CIST_YPCALL`.

### lua_load(lua_State *L, lua_Reader reader, void *data, const char *chunkname, const char *mode)
(`lapi.c:1269`) Parses a chunk fed by `reader`/`data` and pushes the compiled function. The first upvalue (typically `_ENV`) is initialized with the global table from the registry.

### lua_dump(lua_State *L, lua_Writer writer, void *data, int strip)
(`lapi.c:1298`) Dumps the Lua function at the top of the stack via `writer`. Restores the stack top to its pre-call value.

### lua_status(lua_State *L)
(`lapi.c:1314`) Returns the thread status (`LUA_OK`, `LUA_YIELD`, or an `LUA_ERR*` code).

### lua_gc(lua_State *L, int what, ...)
(`lapi.c:1325`) Garbage-collection dispatcher. Handles stop/restart/step/collect/count/generative/incremental mode switching and parameter tuning. Returns -1 when the collector is internally stopped or for invalid options. The step amount `what` (for `LUA_GCSTEP`) is interpreted in Kbytes.

### lua_error(lua_State *L)
(`lapi.c:1417`) Raises the value at the stack top as an error. Special-cases the shared memory-error message string to raise a proper memory error via `luaM_error`. Never returns.

### lua_next(lua_State *L, int idx)
(`lapi.c:1435`) Continues a raw table traversal. The top holds the previous key. Pushes the next key-value pair (returns 1) or pops the key (returns 0).

### lua_toclose(lua_State *L, int idx)
(`lapi.c:1454`) Marks the variable at `idx` as to-be-closed. Creates a TBC upvalue and sets the `CIST_TBC` flag on the current frame.

### lua_concat(lua_State *L, int n)
(`lapi.c:1467`) Concatenates the `n` top values into one string. When `n ≤ 0`, pushes an empty string.

### lua_len(lua_State *L, int idx)
(`lapi.c:1483`) Pushes the length of the value at `idx`, honoring `__len`.

### lua_getallocf(lua_State *L, void **ud)
(`lapi.c:1495`) Returns the current memory-allocation function and stores its user data in `*ud`.

### lua_setallocf(lua_State *L, lua_Alloc f, void *ud)
(`lapi.c:1506`) Installs a new memory allocator with user data.

### lua_setwarnf(lua_State *L, lua_WarnFunction f, void *ud)
(`lapi.c:1515`) Installs the warning function for the state.

### lua_warning(lua_State *L, const char *msg, int tocont)
(`lapi.c:1525`) Emits a warning through the installed handler. `tocont != 0` continues a multi-part message.

### lua_newuserdatauv(lua_State *L, size_t size, int nuvalue)
(`lapi.c:1535`) Allocates a full userdata with `size` bytes and `nuvalue` associated values, pushes it, and returns a pointer to its memory block. `nuvalue` must be < `SHRT_MAX`.

### aux_upvalue(TValue *fi, int n, TValue **val, GCObject **owner)
(`lapi.c:1552`) Resolves upvalue `n` of a closure, returning its value pointer, owner GC object, and name. Returns NULL for out-of-range or non-closure types.

### lua_getupvalue(lua_State *L, int funcindex, int n)
(`lapi.c:1581`) Pushes upvalue `n` of the closure at `funcindex` and returns its name (or NULL).

### lua_setupvalue(lua_State *L, int funcindex, int n)
(`lapi.c:1597`) Pops the top value and stores it as upvalue `n`, emitting a write barrier. Returns the name.

### getupvalref(lua_State *L, int fidx, int n, LClosure **pf)
(`lapi.c:1619`) Returns the `UpVal**` slot for upvalue `n` of a Lua closure, or a pointer to a static NULL for invalid indices.

### lua_upvalueid(lua_State *L, int fidx, int n)
(`lapi.c:1636`) Returns a stable, unique pointer identifying upvalue `n` (an `UpVal*` for Lua closures, a slot address for C closures). NULL for light functions.

### lua_upvaluejoin(lua_State *L, int fidx1, int n1, int fidx2, int n2)
(`lapi.c:1660`) Makes upvalue `(fidx1, n1)` share the same `UpVal` as `(fidx2, n2)`, so both closures read/write the same cell.
