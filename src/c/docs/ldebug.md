# ldebug.c — Debug interface: hooks, stack introspection, symbolic execution, and error reporting

> **AI-Generated Documentation**

## Overview

`ldebug.c` implements Lua's debug interface — the machinery behind `debug.getinfo`, `debug.getlocal`, `debug.sethook`, and the error-reporting functions called throughout the VM (`luaG_typeerror`, `luaG_runerror`, etc.). It bridges the raw bytecode representation with human-readable information: source lines, variable names, and function descriptions. The file is approximately 1085 lines.

The file has three major sections. First, **hook management** (`lua_sethook`, `luaG_tracecall`, `luaG_traceexec`) controls the debug hook system, which fires call/return/line/count events. When hooks are installed, a `trap` flag is set on every active Lua frame; the interpreter polls this flag on every instruction fetch via `vmfetch()`, calling `luaG_traceexec` when it is set. The line hook uses `L->oldpc` and `changedline` to detect source-line transitions efficiently, avoiding expensive `luaG_getfuncline` calls for nearby instructions.

Second, **stack introspection** (`lua_getstack`, `lua_getinfo`, `lua_getlocal`, `lua_setlocal`, `luaG_findlocal`) walks the CallInfo linked list to report function metadata, local variable names, and upvalue information. This supports the public debug API and also provides the variable names used in error messages (e.g., `"attempt to index local 'x' (a nil value)"`).

Third, **symbolic execution** (`findsetreg`, `getobjname`, `basicgetobjname`) performs backward instruction analysis to determine which register was last written by which instruction, enabling the debug system to attach meaningful names to values in error messages. The analysis tracks jump targets to avoid trusting conditional code paths — only unconditional writes are considered reliable.

Error-reporting functions (`luaG_typeerror`, `luaG_callerror`, `luaG_runerror`, `luaG_errormsg`, etc.) use the introspection and symbolic execution infrastructure to produce rich error messages with source locations, variable names, and type information. The `varinfo` function ties it all together by searching upvalues and registers for the value that caused the error, producing suffixes like `" (local 'x')"` or `" (upvalue 'env')"`.

## Key Types / Macros

| Name | Purpose |
|------|---------|
| `ABSLINEINFO` (-0x80) | Marker in `lineinfo` array indicating the next entry is an absolute line in `abslineinfo` |
| `MAXIWTHABS` (128) | Maximum instructions between absolute line info entries; power of 2 for fast division |
| `pcRel(pc, p)` | Convert absolute PC pointer to relative instruction index: `(pc - p->code) - 1` |
| `ci_func(ci)` | Extract the `LClosure*` from a CallInfo: `clLvalue(s2v(ci->func.p))` |
| `resethookcount(L)` | Reset `L->hookcount` to `L->basehookcount` |
| `LuaClosure(f)` | Test whether closure `f` is a Lua closure (has `tt == LUA_VLCL`) |
| `strlocal` / `strupval` | Static string constants `"local"` / `"upvalue"` used as name-kind markers in symbolic execution |

## Functions

### `currentpc(CallInfo *ci)`

Return the instruction index (0-based) that `ci` is currently executing. Asserts that `ci` is a Lua frame. Uses `pcRel(ci->u.l.savedpc, ci_func(ci)->p)` — the `-1` in `pcRel` accounts for the fact that `savedpc` always points to the *next* instruction to execute (it was incremented during the previous fetch in `vmfetch()`).

---

### `getbaseline(const Proto *f, int pc, int *basepc)`

Find the absolute line entry at or before instruction `pc`. Line info is compressed: absolute entries are stored at most `MAXIWTHABS` instructions apart in `abslineinfo`, with signed-byte relative deltas in `lineinfo` between them. Estimates the base index via `pc / MAXIWTHABS - 1`, then walks forward through `abslineinfo` if the estimate is too low.

**Returns:** the base line number. Stores the base pc in `*basepc` (-1 when starting from `linedefined`).

---

### `luaG_getfuncline(const Proto *f, int pc)`

Compute the source line for instruction `pc` of prototype `f`. Calls `getbaseline` to find the starting point, then sums relative `lineinfo[basepc]` deltas until reaching `pc`. Returns -1 if `f->lineinfo == NULL` (no debug information available — typical for code compiled without `-g`).

---

### `getcurrentline(CallInfo *ci)`

Return the source line currently being executed by the active Lua call `ci`. Thin wrapper: `luaG_getfuncline(ci_func(ci)->p, currentpc(ci))`.

---

### `settraps(CallInfo *ci)`

Walk all active CallInfo frames and set `ci->u.l.trap = 1` on every Lua frame. Called from `lua_sethook` when hooks are turned on, ensuring the interpreter stops at the next instruction to re-check. Safe to call from signal handlers under reasonable assumptions about pointer atomicity.

---

### `lua_sethook(lua_State *L, lua_Hook func, int mask, int count)`

Install or clear the debug hook. When `func == NULL` or `mask == 0`, clears everything. Otherwise sets `L->hook`, `L->basehookcount`, resets `L->hookcount` via `resethookcount`, and sets `L->hookmask`. When turning hooks on (`mask != 0`), calls `settraps(L->ci)` to ensure the interpreter re-checks on the next instruction.

This function is safe to call from signal handlers.

---

### `lua_gethook(lua_State *L)` / `lua_gethookmask(lua_State *L)` / `lua_gethookcount(lua_State *L)`

Simple accessors returning the current hook function, event mask, and count interval from the thread state. All lock/unlock the state for thread safety.

---

### `lua_getstack(lua_State *L, int level, lua_Debug *ar)`

Walk the CallInfo list from `L->ci` toward the base to find the `level`-th active function (0 = innermost). Stores the CallInfo pointer in `ar->i_ci`. Returns 1 if found, 0 if the level doesn't exist or is negative.

---

### `upvalname(const Proto *p, int uv)`

Return the debug name of upvalue `uv` from prototype `p` as a C string, or `"?"` if no name is stored (`p->upvalues[uv].name == NULL`). Bounds-checked via `check_exp`.

---

### `findvararg(CallInfo *ci, int n, StkId *pos)`

Locate the `-n`-th extra argument of a vararg call (negative index since locals are numbered from 1). Checks the `PF_VAHID` flag to confirm the function is vararg, then checks that `n` is within `nextraargs`. Computes the position as `ci->func - nextraargs - (n + 1)`.

**Returns:** the generic name `"(vararg)"` and stores the position in `*pos`, or NULL if out of range.

---

### `luaG_findlocal(lua_State *L, CallInfo *ci, int n, StkId *pos)`

Find the name of the `n`-th local (or `-n`-th vararg) of call `ci` at the current PC. For Lua frames: negative indices delegate to `findvararg`; positive indices first check `luaF_getlocalname` for debug names (from the `locvars` debug structure), falling back to `"(temporary)"` for valid slots. For C frames: valid slots get `"(C temporary)"`. If `pos` is non-NULL, stores `base + (n - 1)`.

**Returns:** the local's name, or NULL when `n` is out of range.

---

### `lua_getlocal(lua_State *L, const lua_Debug *ar, int n)`

API: return the name of local `n` and push its value onto the stack. With `ar == NULL`, inspects parameters of a non-active Lua function on the top of the stack (line 0 — as if the function just started). Otherwise uses `luaG_findlocal` via `ar->i_ci`. Increments `L->top.p` when a value is pushed.

---

### `lua_setlocal(lua_State *L, const lua_Debug *ar, int n)`

API: assign the value on top of the stack to local `n` of the function described by `ar`, pop it, and return the local's name. Uses `luaG_findlocal` to locate the target slot. Returns NULL if `n` is out of range.

---

### `funcinfo(lua_Debug *ar, Closure *cl)`

Fill the `S` (source, what, linedefined, lastlinedefined) debug fields. C closures get `source = "=[C]"`, `what = "C"`. Lua closures get their prototype's source string and definition lines; `what` is `"main"` when `linedefined == 0` (chunk entry point), otherwise `"Lua"`. Also calls `luaO_chunkid` to compute `short_src` (a shortened version of the source path for display).

---

### `nextline(const Proto *p, int currentline, int pc)`

Compute the line for instruction `pc` given the running `currentline`. If `p->lineinfo[pc]` is `ABSLINEINFO`, falls back to `luaG_getfuncline` for an absolute lookup (this happens at most once per `MAXIWTHABS` instructions). Otherwise adds the relative delta to `currentline`.

---

### `collectvalidlines(lua_State *L, Closure *f)`

Build and push a table mapping every executable line in function `f` to boolean `true` (the `'L'` option for `lua_getinfo`). Non-Lua closures get nil. Vararg functions skip `OP_VARARGPREP` (instruction 0) to avoid counting the preparation line. Walks all instructions, accumulating `currentline` via `nextline`, and inserts each into the table via `luaH_setint`. Used by debug tools to show which lines are "breakable."

---

### `getfuncname(lua_State *L, CallInfo *ci, const char **name)`

Try to name the function being called by `ci` from its caller's code. If the caller is a Lua function and not a tail call, delegates to `funcnamefromcall(ci->previous)`. Returns NULL for tail calls or C callers (they have no meaningful calling instruction to inspect).

---

### `auxgetinfo(lua_State *L, const char *what, lua_Debug *ar, Closure *f, CallInfo *ci)`

Fill `lua_Debug` fields according to the `what` string, character by character:
- `'S'`: calls `funcinfo` for source/line info.
- `'l'`: sets `currentline` from `getcurrentline` (or -1 for C functions).
- `'u'`: sets `nups`, `isvararg`, `nparams` from the closure/prototype.
- `'t'`: sets `istailcall` and `extraargs` (the packed `__call` metamethod count).
- `'n'`: sets `name` and `namewhat` via `getfuncname`.
- `'r'`: sets `ftransfer`/`ntransfer` from hook transfer info.
- `'f'`/`'L'`: handled by the caller (`lua_getinfo` pushes the function or valid-lines table).

**Returns:** 1 on success, 0 if an unknown option is encountered.

---

### `lua_getinfo(lua_State *L, const char *what, lua_Debug *ar)`

API entry point for gathering debug information. With `>` prefix, inspects the function on top of the stack (popped). Otherwise inspects the function in `ar->i_ci` (from a previous `lua_getstack` call). Calls `auxgetinfo` for the `what` fields, then additionally pushes the function object for `'f'` and a valid-lines table for `'L'`.

---

### `filterpc(int pc, int jmptarget)`

Symbolic execution helper: if `pc < jmptarget`, the instruction was inside a conditional branch (after a jump target) and may not have executed, so returns -1 (unknown writer). Otherwise returns `pc` as a trustworthy write point.

---

### `findsetreg(const Proto *p, int lastpc, int reg)`

Backward scan through instructions to find the last one before `lastpc` that may have written register `reg`. This is the core of the symbolic execution engine.

Tracks `jmptarget` to detect conditional code — any instruction before the furthest jump target is considered conditional. Handles special opcodes:
- `LOADNIL`: sets registers `A` through `A+B`.
- `TFORCALL`: sets all registers at or above `A+2` (iterator results).
- `CALL`/`TAILCALL`: sets all registers at or above `A` (function results).
- `JMP`: updates `jmptarget` to the farthest forward destination.
- MM-mode opcodes: the previous instruction was not actually executed.
- All other `A`-mode ops: sets register `A` if it matches `reg`.

**Returns:** the pc of the last unconditional writer, or -1 if only conditional writes were found.

---

### `kname(const Proto *p, int index, const char **name)`

If constant `index` is a string, use its contents as a name and return `"constant"`. Otherwise set `*name = "?"` and return NULL. Used by `getobjname` to extract meaningful key names from constant operands.

---

### `basicgetobjname(const Proto *p, int *ppc, int reg, const char **name)`

Name the value in register `reg` via two strategies: (1) check `luaF_getlocalname` for a debug local name at the current pc — if found, returns `strlocal`; (2) follow the instruction that wrote it via `findsetreg`, then: `OP_MOVE` recurses on the source register (if `B < A`, meaning it's a copy from a lower register), `OP_GETUPVAL` returns the upvalue name via `upvalname` and `strupval`, `OP_LOADK`/`OP_LOADKX` return the constant name via `kname` and `"constant"`.

**Returns:** the name kind (`strlocal`, `strupval`, `"constant"`) or NULL.

---

### `rname(const Proto *p, int pc, int c, const char **name)`

Name for register `c` used as a table key operand. Calls `basicgetobjname`; keeps the result only when it was found as a constant (the kind string is `"constant"`). Otherwise sets `*name = "?"` (the key's name is not a known constant).

---

### `isEnv(const Proto *p, int pc, Instruction i, int isup)`

Check whether the table being indexed by instruction `i` is the `_ENV` environment. If the table operand is an upvalue (`isup`), checks `upvalname`; if a register, uses `basicgetobjname` to find its name. Returns `"global"` if the name is exactly `LUA_ENV` (`"_ENV"`), otherwise `"field"`. This is how the debug system distinguishes global variable accesses from table field accesses.

---

### `getobjname(const Proto *p, int lastpc, int reg, const char **name)`

Extend `basicgetobjname` with table access instructions. After the basic lookup fails, examines the instruction at `lastpc`: `GETTABUP`/`GETTABLE`/`GETVARG`/`GETFIELD` provide the key via `kname`/`rname` and classify as `"global"` or `"field"` (via `isEnv`). `GETI` returns `"integer index"` / `"field"`. `GETVARG` uses `rname` for the key. `SELF` returns the key name / `"method"`. These categories appear in error messages like `"attempt to index a global 'x' (a nil value)"`.

---

### `funcnamefromcode(lua_State *L, const Proto *p, int pc, const char **name)`

Name a called function from the instruction at `pc` that invoked it. `OP_CALL`/`OP_TAILCALL` look up the function register via `getobjname`. `OP_TFORCALL` returns `"for iterator"`. Metamethod-triggering opcodes return `"metamethod"` with the tag method name from `G(L)->tmname[tm]` (stripped of the leading two characters). Covers `OP_SELF`, `OP_GET*`, `OP_SET*`, `OP_MMBIN*`, `OP_UNM`, `OP_BNOT`, `OP_LEN`, `OP_CONCAT`, `OP_EQ`, `OP_LT`, `OP_LE`, `OP_CLOSE`, `OP_RETURN`.

---

### `funcnamefromcall(lua_State *L, CallInfo *ci, const char **name)`

Name a function from how its frame was created: `"hook"` with name `"?"` for hook-invoked functions (`CIST_HOOKED`), `"metamethod"` with name `"__gc"` for finalizers (`CIST_FIN`), or delegates to `funcnamefromcode` for Lua callers. Returns NULL for C callers.

---

### `instack(CallInfo *ci, const TValue *o)`

Check whether value `o` lives inside the stack frame of `ci`. Walks slots from `base` to `ci->top` one by one, comparing pointers. Returns the register index (0-based) or -1. Avoids comparing `o` against frame boundaries (which would be undefined behavior for out-of-frame pointers in ISO C).

---

### `getupvalname(CallInfo *ci, const TValue *o, const char **name)`

Check whether `o` is the current value of any upvalue in the current Lua closure. Iterates `c->upvals[0..nupvalues-1]`, comparing `upvals[i]->v.p` against `o`. This only works for open upvalues (which point directly to the stack) — closed upvalues point to a heap copy.

**Returns:** the upvalue's debug name and `strupval`, or NULL.

---

### `formatvarinfo(lua_State *L, const char *kind, const char *name)`

Build the `" (kind 'name')"` suffix for error messages. Returns `""` (empty string) when `kind == NULL` (no information available). Otherwise uses `luaO_pushfstring` to format like `" (local 'x')"` or `" (upvalue 'env')"`.

---

### `varinfo(lua_State *L, const TValue *o)`

Build a description of where value `o` came from, for use in error messages. Checks upvalues first via `getupvalname`, then checks whether `o` is a register via `instack` + `getobjname`. Returns a formatted suffix that helps the user understand which variable caused the error. Returns `""` when no name information is available.

---

### `typeerror(lua_State *L, const TValue *o, const char *op, const char *extra)`

Internal helper: raise `"attempt to <op> a <type> value<extra>"`. Gets the type name via `luaT_objtypename`. This is the lowest-level type error function; all others eventually call this.

---

### `luaG_typeerror(lua_State *L, const TValue *o, const char *op)`

Raise a type error with automatic variable-name information from `varinfo`. This is the most commonly called error function — virtually every type-mismatch in the VM calls it.

---

### `luaG_callerror(lua_State *L, const TValue *o)`

Raise an error for calling a non-callable value. Tries to name the object from the call site (`funcnamefromcall`) first, producing messages like `"attempt to call a nil value (global 'f')"`. Falls back to `varinfo` for the `extra` suffix when no call-site name is available.

---

### `luaG_forerror(lua_State *L, const TValue *o, const char *what)`

Raise a numeric `for` loop error: `"bad 'for' <what> (number expected, got <type>)"`. The `what` parameter is `"limit"`, `"step"`, or `"initial value"`.

---

### `luaG_concaterror(lua_State *L, const TValue *p1, const TValue *p2)`

Raise a concatenation error, reporting the operand that is not string-like. If `p1` is string-like (`ttisstring(p1) || cvt2str(p1)`), reports `p2`; otherwise reports `p1`.

---

### `luaG_opinterror(lua_State *L, const TValue *p1, const TValue *p2, const char *msg)`

Raise an arithmetic/bitwise error for a non-numeric operand. If `p1` is not a number, reports `p1`; otherwise reports `p2`.

---

### `luaG_tointerror(lua_State *L, const TValue *p1, const TValue *p2)`

Error when both operands are numeric but neither converts to integer: `"number <info> has no integer representation"`. Uses `luaV_tointegerns` to find the culprit (the first non-integer-convertible operand).

---

### `luaG_ordererror(lua_State *L, const TValue *p1, const TValue *p2)`

Error for comparing two values with no ordering metamethod. Adapts the message: `"attempt to compare two <type> values"` when both types match, `"attempt to compare <t1> with <t2>"` when different.

---

### `luaG_errnnil(lua_State *L, LClosure *cl, int k)`

Raise `"global '<name>' already defined"` for a script that redefined a reserved global. Index `k` (1-based, 0 when absent) is used to look up the global name from the prototype's constants via `kname`. This supports Eira's stricter global handling.

---

### `luaG_addinfo(lua_State *L, const char *msg, TString *src, int line)`

Prepend `"src:line: "` to an error message. Uses `luaO_chunkid` for the shortened source name. If `src == NULL` (no debug info), uses `"?:?:"`.

---

### `luaG_errormsg(lua_State *L)`

Deliver the pending error. If `L->errfunc != 0`, calls the registered error handler with the error object via `luaD_callnoyield` (no yields allowed in the handler). Replaces nil error objects with `"<no error object>"`. Throws `LUA_ERRRUN`.

---

### `luaG_runerror(lua_State *L, const char *fmt, ...)`

Raise a formatted runtime error. Calls `luaC_checkGC` first (error messages may allocate strings). Formats via `pushvfstring`. For Lua frames, prepends source:line via `luaG_addinfo` (replacing the plain message with the annotated version). Dispatches through `luaG_errormsg`.

---

### `changedline(const Proto *p, int oldpc, int newpc)`

Determine whether instructions `oldpc` and `newpc` are on different source lines. For nearby instructions (`newpc - oldpc < MAXIWTHABS/2`), sums relative `lineinfo` deltas in a tight loop — this is the fast path taken most of the time. Stops early if an `ABSLINEINFO` marker is hit (which would require an absolute lookup). For distant instructions or when `ABSLINEINFO` intervenes, calls `luaG_getfuncline` for both and compares.

**Returns:** 1 if the lines differ, 0 if they are the same.

---

### `luaG_tracecall(lua_State *L)`

Hook processing when a Lua function starts executing. Sets `trap = 1` to ensure hooks keep being checked. For the first instruction of the function (`savedpc == p->code`): fires the call hook via `luaD_hookcall` unless the function is vararg (hooks start at `OP_VARARGPREP` instead, so parameters are set up first) or was resumed from a yield (`CIST_HOOKYIELD` — the hook was already called before yielding).

**Returns:** 1 to keep `trap` on.

---

### `luaG_traceexec(lua_State *L, const Instruction *pc)`

Hook processing before each executed instruction — the most performance-critical debug function. Increments `pc` to reference the next instruction, saves it in `ci->u.l.savedpc`. Decrements `L->hookcount` for the count hook; fires the line hook when `changedline` detects a new line (comparing against `L->oldpc`).

Handles `CIST_HOOKYIELD` (the hook yielded last time — clear the mark and skip calling the hook again since the VM didn't advance). Corrects `L->top.p` when the current instruction doesn't use the stack top. If the hook yields, undoes the count decrement (`L->hookcount = 1`) and throws `LUA_YIELD`.

**Returns:** 0 to disarm the trap when no line/count hooks remain, or 1 to keep checking.
