# ltm.c — Tag methods (metamethods) and vararg handling

> **AI-Generated Documentation**

## Overview

`ltm.c` implements Eira's metamethod system — the mechanism that lets values customize operators, indexing, concatenation, ordering, length, equality, and garbage-collection behavior. It also handles the two vararg-access strategies used by the VM: a vararg table approach and a hidden-arguments approach.

On state initialization, `luaT_init` interns all 25 metamethod name strings (`__index`, `__newindex`, …, `__close`) into the global string table and fixes them so the GC never collects them. These interned names live in `G(L)->tmname[]` and are used for fast table lookups.

Metamethod lookup follows a two-tier strategy. `luaT_gettm` is the fast path: it does a short-string hash-table lookup on the metatable and caches misses as flag bits in `events->flags` so future lookups short-circuit. `luaT_gettmbyobj` handles the general case: it resolves the metatable for any value type (tables and userda have their own; other types use the per-type global metatable `G(L)->mt[type]`) and then looks up the method.

The file also provides the vararg machinery. When a vararg function uses `PF_VATAB`, extra arguments are bundled into a table with field `"n"`. When `PF_VAHID` is used, the extra arguments are hidden below `ci->func` on the stack and counted in `ci->u.l.nextraargs`.

## Key Types / Macros

| Identifier | Purpose |
|---|---|
| `TMS` | Enum of all tag-method events (`TM_INDEX` through `TM_CLOSE`, `TM_N` total). |
| `fasttm(l,mt,e)` | Macro: fast metamethod lookup checking the flag-bit cache first, then calling `luaT_gettm`. |
| `notm(tm)` | True when a metamethod lookup returned nil (no method). |
| `maskflags` | Bitmask for fast-access metamethod flags (bits 0 through `TM_EQ`). |
| `luaT_typenames_` | Array mapping type tags to printable names (`"nil"`, `"boolean"`, `"userdata"`, etc.). |

## Functions

### `luaT_init(lua_State *L)`

Interns all metamethod name strings into `G(L)->tmname[]` (indexed by the `TMS` enum) and fixes them with `luaC_fix` so the GC never collects them. Called once during state construction.

### `luaT_gettm(Table *events, TMS event, TString *ename)`

Fast metamethod lookup in a metatable. Does a raw short-string hash-table get. On a miss, sets the corresponding bit in `events->flags` to cache the absence, then returns `NULL`. Returns the metamethod `TValue` on hit.

### `luaT_gettmbyobj(lua_State *L, const TValue *o, TMS event)`

General metamethod lookup for any value. Tables and full userdata use their own metatable; all other types fall back to `G(L)->mt[ttype(o)]`. Returns the method or `G(L)->nilvalue` if no metatable or no method exists.

### `luaT_objtypename(lua_State *L, const TValue *o)`

Returns the type name for error messages. For tables and full userdata, checks for a `__name` string metafield and uses it if present. Falls back to the standard `ttypename(ttype(o))`.

### `luaT_callTM(lua_State *L, const TValue *f, const TValue *p1, const TValue *p2, const TValue *p3)`

Pushes metamethod `f` plus three arguments (`p1`, `p2`, `p3`) on the stack and calls it. Yields only when invoked from Lua code (`isLuacode`); uses `luaD_callnoyield` otherwise.

### `luaT_callTMres(lua_State *L, const TValue *f, const TValue *p1, const TValue *p2, StkId res)`

Calls a binary metamethod `f` with two operands, expects one result, and moves it to `res`. Returns the tag of the result. Saves and restores `res` across the call using `savestack` / `restorestack`.

### `callbinTM(lua_State *L, const TValue *p1, const TValue *p2, StkId res, TMS event)` (static)

Resolves a binary metamethod: tries `p1`'s metatable first, then `p2`'s. Returns the result tag on success or -1 if neither has the method.

### `luaT_trybinTM(lua_State *L, const TValue *p1, const TValue *p2, StkId res, TMS event)`

Calls `callbinTM` and raises a descriptive error if no metamethod is found. Bitwise operations get a specialized message (`"perform bitwise operation on"` / `"perform arithmetic on"`); if both operands are numbers, `luaG_tointerror` is used.

### `luaT_tryconcatTM(lua_State *L)`

Tries the `TM_CONCAT` metamethod on the two values at `L->top - 2` and `L->top - 1`. Raises `luaG_concaterror` if neither has the method.

### `luaT_trybinassocTM(lua_State *L, const TValue *p1, const TValue *p2, int flip, StkId res, TMS event)`

Binary metamethod with operand-flip support. When `flip` is set, `p2` and `p1` are swapped before the call — needed for non-commutative operations like subtraction and shifts.

### `luaT_trybiniTM(lua_State *L, const TValue *p1, lua_Integer i2, int flip, StkId res, TMS event)`

Convenience wrapper: boxes the integer `i2` into a `TValue` and delegates to `luaT_trybinassocTM`.

### `luaT_callorderTM(lua_State *L, const TValue *p1, const TValue *p2, TMS event)`

Calls an order metamethod (`__lt` or `__le`). Returns 1 if the result is truthy, 0 if falsy. Raises `luaG_ordererror` if no metamethod exists.

### `luaT_callorderiTM(lua_State *L, const TValue *p1, int v2, int flip, int isfloat, TMS event)`

Order comparison where the second operand is an integer (or float if `isfloat`). Handles flipped argument order, then delegates to `luaT_callorderTM`.

### `createvarargtab(lua_State *L, StkId f, int n)` (static)

Builds a vararg table at the top of the stack with `n` elements from `f`. Sets field `"n"` to the count and integer keys `1..n` to the arguments.

### `buildhiddenargs(lua_State *L, CallInfo *ci, const Proto *p, int totalargs, int nfixparams, int nextra)` (static)

Reorganizes a vararg frame without a table: copies the function above the extra arguments, moves fixed parameters after the copy, and adjusts `ci->func` to point past the hidden slots. Stores the extra-arg count in `ci->u.l.nextraargs`.

### `luaT_adjustvarargs(lua_State *L, CallInfo *ci, const Proto *p)`

Adjusts a vararg function's frame on entry. If `PF_VATAB` is set, creates a vararg table via `createvarargtab`. If `PF_VAHID`, hides the extra arguments via `buildhiddenargs` and nils the vararg parameter register.

### `luaT_getvararg(CallInfo *ci, StkId ra, TValue *rc)`

Resolves a single vararg access (`...[rc]`). An integer key indexes the hidden args; the string `"n"` returns the extra-arg count; anything else produces `nil`.

### `getnumargs(lua_State *L, CallInfo *ci, Table *h)` (static)

Counts the extra arguments of a vararg frame. If no vararg table exists, reads `ci->u.l.nextraargs`. Otherwise reads and validates the `"n"` field from the table.

### `luaT_getvarargs(lua_State *L, CallInfo *ci, StkId where, int wanted, int vatab)`

Copies `wanted` vararg values into `where` (all of them when `wanted < 0`). Reads from hidden stack slots (when `vatab < 0`) or the vararg table. Pads unfilled slots with `nil`.
