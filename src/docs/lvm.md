# lvm.c — Lua Virtual Machine: bytecode interpreter and arithmetic operations

> **AI-Generated Documentation**

## Overview

`lvm.c` is the heart of the Eira runtime — it implements the Lua virtual machine's main interpreter loop (`luaV_execute`), arithmetic/bitwise operations, comparisons, table access finishers, string concatenation, and the `#` length operator. Every executed Lua function ultimately passes through this file. The file is approximately 2050 lines, with the opcode dispatch loop alone accounting for roughly 770 lines.

The interpreter uses a **threaded dispatch** model: a `for(;;)` loop fetches instructions via `vmfetch()`, dispatches through a `vmdispatch` switch (or jump table on GCC when `LUA_USE_JUMPTABLE` is enabled), and executes each opcode handler with a `vmcase`/`vmbreak` pattern. Nested Lua calls reuse the same C stack frame — when `OP_CALL` or `OP_TAILCALL` encounters a Lua callee, it jumps back to the `startfunc:` label rather than recursing. This is critical for performance: deep Lua call chains use only one C stack frame. The `CIST_FRESH` flag on the outermost frame causes the loop to `return` when that frame's `OP_RETURN`/`OP_RETURN0`/`OP_RETURN1` executes, unwinding cleanly to the C caller that invoked `luaV_execute`.

Key design points include: metamethod chains for `__index`/`__newindex` (capped at `MAXTAGLOOP = 2000` to prevent infinite loops), fast-path macros (`luaV_fastget`, `luaV_fastset`) that inline the common table-hit case and avoid function call overhead, a `trap` flag polled on every instruction fetch to honor debug hooks and signal handlers, and careful integer/float comparison helpers that avoid precision loss when values don't fit exactly in the other numeric type.

The file also contains `luaV_finishOp`, which resumes opcodes interrupted by a coroutine yield. When a C function yields (e.g., from `lua_yieldk`), the VM saves its state and longjmps back to the resumer. When resumed, `luaV_finishOp` inspects the interrupted instruction and correctly places results, skips conditional jumps, re-executes deferred close/return instructions, or re-invokes concatenation. This mechanism is what makes coroutines transparent to opcode semantics.

The conversion helpers (`luaV_tonumber_`, `luaV_tointeger`, `luaV_flttointeger`, etc.) form the foundation of Lua's dynamic typing for numeric operations. They handle string coercion, float-to-integer rounding modes (`F2Ieq`/`F2Ifloor`/`F2Iceil`), and the tricky edge cases around integer ranges that don't fit in floating-point mantissas. The `l_intfitsf` macro determines at compile time whether all integers fit exactly in a float, enabling optimized comparison paths.

The arithmetic macro layer (`op_arith`, `op_arithI`, `op_bitwise`, etc.) generates compact opcode handlers by dispatching on operand types: integer-integer paths execute directly with `intop`, while mixed/float paths delegate to platform-specific float operations via `luai_num*` macros. This separation keeps the common integer path fast while correctly handling all numeric coercions.

## Key Types / Macros

| Name | Purpose |
|------|---------|
| `F2Imod` (enum) | Float-to-integer rounding mode: `F2Ieq` (exact — rejects non-integral), `F2Ifloor` (floor), `F2Iceil` (ceiling) |
| `MAXTAGLOOP` (2000) | Maximum depth of `__index`/`__newindex` metamethod chains before raising a run-time error |
| `NBM` | Mantissa bits of `lua_Number` — determines which integers fit exactly in a float via `l_intfitsf` |
| `intop(op,v1,v2)` | Unsigned-cast integer binary operation: `l_castU2S(l_castS2U(v1) op l_castS2U(v2))`, avoids UB on overflow |
| `luaV_fastget(t,k,res,f,tag)` | Inline fast-path for table index: if `t` is a table, calls `f(hvalue(t),k,res)`; otherwise sets tag to `LUA_VNOTABLE` |
| `luaV_fastgeti(t,k,res,tag)` | Specialized fast-path for integer-keyed table access, inlining the fast case of `luaH_getint` |
| `luaV_fastset` / `luaV_fastseti` | Fast-path for table writes, returning `HOK` on success or an error code for the slow path |
| `vmfetch()` | Fetch next instruction, poll `trap` for hooks/stack reallocation, update `base` if needed |
| `Protect(exp)` | Save PC and top, execute `exp`, re-check trap — used around code that may allocate or trigger hooks |
| `halfProtect(exp)` | Save PC and top, execute `exp` — for code that may error but not change hooks |
| `ProtectNT(exp)` | Save PC only, execute `exp`, re-check trap — for code that cannot change the stack top |
| `op_arith` / `op_arithI` / `op_bitwise` | Dispatcher macros for arithmetic/bitwise opcodes inside the main loop |
| `op_order` / `op_orderI` | Dispatcher macros for comparison opcodes with fast integer/number paths |
| `dojump(ci,i,e)` | Execute a jump: `pc += GETARG_sJ(i) + e`, then `updatetrap(ci)` |
| `checkGC(L,c)` | Trigger GC if needed and yield for thread safety, with `c` as the live-stack ceiling |

## Functions

### `l_strton(const TValue *obj, TValue *result)`

Attempts to convert a string-typed `TValue` to a number via `luaO_str2num`. Returns 1 on success (stores result in `result`), 0 on failure (leaves `result` unchanged). Used as a helper by `luaV_tonumber_` and `luaV_tointeger` for string coercion. The check `cvt2num(obj)` gates whether the type is eligible — by default this is true for all string types but can be disabled with `LUA_NOCVTS2N`.

**Parameters:**
- `obj`: the value to convert (must not alias `result`).
- `result`: destination for the converted number on success.

**Returns:** 1 on success, 0 on failure.

---

### `luaV_tonumber_(const TValue *obj, lua_Number *n)`

Convert any value to a float. Handles integer-to-float promotion directly (`*n = cast_num(ivalue(obj))`). For strings, delegates to `l_strton`, then converts the resulting `TValue` to a float via `nvalue`. Returns 1 on success, 0 on failure. This is the general-purpose "to-number" for the float type, called by the `tonumber` macro in the interpreter.

**Parameters:**
- `obj`: the value to convert.
- `n`: destination float on success.

**Returns:** 1 on success, 0 if the value cannot be converted.

---

### `luaV_flttointeger(lua_Number n, lua_Integer *p, F2Imod mode)`

Convert a float to an integer according to rounding mode. Computes `l_floor(n)`: if `n` equals its floor, the value is integral and the conversion proceeds. For `F2Ieq`, non-integral values fail immediately (return 0). For `F2Iceil`, adds 1 to the floor when `n != f`. The final conversion to `lua_Integer` is delegated to `lua_numbertointeger`, which may fail if the float is outside the representable integer range.

**Parameters:**
- `n`: the float value to convert.
- `p`: destination integer on success.
- `mode`: rounding mode (`F2Ieq`, `F2Ifloor`, `F2Iceil`).

**Returns:** 1 on success, 0 if the float cannot be represented as an integer in the given mode.

---

### `luaV_tointegerns(const TValue *obj, lua_Integer *p, F2Imod mode)`

Convert a numeric value (integer or float, **no** string coercion) to an integer. Fast-path: integers are copied directly with `*p = ivalue(obj)`. Floats go through `luaV_flttointeger`. Returns 0 for non-numeric types. Used by the `tointegerns` macro in the interpreter loop for performance-sensitive paths.

---

### `luaV_tointeger(const TValue *obj, lua_Integer *p, F2Imod mode)`

Like `luaV_tointegerns` but allows string coercion via `l_strton`. If the value is a string representing a number, it is first converted to a temporary numeric `TValue` (`v`), then `obj` is redirected to point at `v`, and forwarded to `luaV_tointegerns`. This is the general-purpose value-to-integer conversion used by the public API and `tointeger` macro.

---

### `forlimit(lua_State *L, lua_Integer init, const TValue *lim, lua_Integer *p, lua_Integer step)`

Compute the integer limit for a numeric `for` loop, preserving loop semantics. Tries integer conversion first via `luaV_tointeger` with floor/ceil mode matching the step direction. If that fails, tries float conversion via `tonumber`. Floats beyond integer range are clipped: a positive float exceeding `LUA_MAXINTEGER` becomes `LUA_MAXINTEGER` (for positive step) or causes the loop to be skipped (for negative step). Raises `luaG_forerror` if the limit is not numeric.

**Returns:** 1 if the loop body should be skipped entirely, 0 otherwise (with `*p` set to the integer limit).

---

### `forprep(lua_State *L, StkId ra)`

Prepare a numeric `for` loop (opcode `OP_FORPREP`). Before execution, the stack contains `ra` (init), `ra+1` (limit), `ra+2` (step).

For **integer loops** (both init and step are integers): validates step is non-zero, computes the iteration count using unsigned arithmetic: `count = |limit - init| / |step|`. For negative steps, the divisor is computed as `-(step+1) + 1u` to avoid negating `LUA_MININTEGER` (which has no positive counterpart in two's complement). Then rearranges the stack to `count` (ra), `step` (ra+1), `init` (ra+2) for the optimized `OP_FORLOOP`.

For **float loops**, converts all values to floats, validates step ≠ 0, checks if the loop should be skipped, and stores `limit`/`step`/`control` as float `TValue`s. Returns 1 if the loop should be skipped.

---

### `floatforloop(lua_State *L, StkId ra)`

Execute one iteration of a **float** numeric `for` loop: adds `step` to the control variable (`ra+2`), then returns 1 (continue/jump back) while the index is still within bounds. The comparison direction depends on the sign of step. The integer case is inlined in `OP_FORLOOP` for performance — it only needs a counter decrement and step addition, no comparison against the limit.

---

### `luaV_finishget(lua_State *L, const TValue *t, TValue *key, StkId val, lu_byte tag)`

Complete a table read `t[key]` that missed the fast path (`tag` indicates empty). For non-table objects (`tag == LUA_VNOTABLE`), looks up the `__index` metamethod via `luaT_gettmbyobj`; if absent, raises a type error. For tables, checks the metatable's `__index` via `fasttm`; if absent, writes nil and returns `LUA_VNIL`.

When the metamethod is a function, calls it via `luaT_callTMres` and returns the result tag. When the metamethod is itself a table, re-attempts the access on that table via `luaV_fastget` and loops. The chain is capped at `MAXTAGLOOP` (2000) iterations.

**Tricky logic:** The `tag` parameter serves double duty — it indicates the fast-path miss type (`LUA_VNOTABLE` for non-tables, empty for table misses) and is reused to track the current state of the loop iteration.

---

### `luaV_finishset(lua_State *L, const TValue *t, TValue *key, TValue *val, int hres)`

Complete a table write `t[key] = val` that missed the fast path. Chases `__newindex` similarly to `luaV_finishget`. When a raw table write succeeds (no metamethod needed), the table `h` is **anchored** on the stack via `sethvalue2s(L, L->top.p, h)` before calling `luaH_finishset` — this is critical because the call may trigger an emergency GC, and if the metatable is weak, an unanchored table could be collected mid-update. Uses `luaC_barrierback` and `invalidateTMcache` after writes.

**Tricky logic:** The anchor is pushed to `L->top.p` and `L->top` is incremented, then decremented after the call. This assumes `EXTRA_STACK` space is available.

---

### `l_strcmp(const TString *ts1, const TString *ts2)`

Locale-aware string comparison that handles embedded `\0` bytes. Compares segment-by-segment with `strcoll` (respecting the current locale). After each equal segment, advances past the `\0` and continues. Segments may compare equal via `strcoll` but have different lengths, so both the collation result and string exhaustion are checked.

**Returns:** `<0` if `ts1 < ts2`, `0` if equal, `>0` if `ts1 > ts2`.

---

### `LTintfloat(lua_Integer i, lua_Number f)` / `LEintfloat`

Inline helpers for `i < f` and `i <= f`. When the integer fits exactly in a float (`l_intfitsf`), compares directly as floats. Otherwise uses `ceil`/`floor` equivalences: `i < f ⟺ i < ceil(f)` and `i <= f ⟺ i <= floor(f)`. If the ceiling/floor is out of integer range, resolves by the sign of `f`. These are `l_sinline` for performance in the comparison dispatch.

---

### `LTfloatint(lua_Number f, lua_Integer i)` / `LEfloatint`

Inline helpers for `f < i` and `f <= i`. Same precision-safe approach: `f < i ⟺ floor(f) < i` and `f <= i ⟺ ceil(f) <= i`. Falls back to direct float comparison when the integer fits.

---

### `LTnum(const TValue *l, const TValue *r)` / `LEnum`

Dispatch numeric `<` / `<=` across all four type combinations using the inline helpers. The int-int path is the fastest (`li < ivalue(r)`). Mixed paths use `LTintfloat`/`LTfloatint` etc. Float-float paths use `luai_numlt`/`luai_numle` directly. Both functions assert that both operands are numeric.

---

### `lessthanothers(lua_State *L, const TValue *l, const TValue *r)` / `lessequalothers`

Handle `<` / `<=` for non-numeric operands. String-vs-string uses `l_strcmp`. Otherwise calls the `__lt` / `__le` metamethod via `luaT_callorderTM`, which may raise a type error if no metamethod exists.

---

### `luaV_lessthan(lua_State *L, const TValue *l, const TValue *r)` / `luaV_lessequal`

Main `<` and `<=` operations exported to the rest of the VM. Checks if both operands are numbers (fast path via `LTnum`/`LEnum`), otherwise falls back to `lessthanothers`/`lessequalothers`.

---

### `luaV_equalobj(lua_State *L, const TValue *t1, const TValue *t2)`

Main equality test `t1 == t2`. With `L == NULL`, performs **raw** equality (no metamethods — used internally for table hashing and constant folding). First checks if types match; if not, handles cross-type comparisons: integer-float uses `luaV_flttointeger` with `F2Ieq` (exact); short-string-long-string uses `luaS_eqstr`.

Same-variant comparisons are direct value checks (pointer equality for strings, `==` for floats, pointer for userdata, etc.). For tables/userdata, identity equality is checked first; if different objects and `L != NULL`, looks up `__eq` metamethod. The result is tested via `!tagisfalse(tag)`.

---

### `copy2buff(StkId top, int n, char *buff)`

Copy `n` strings from just below `top` (from `top-n` to `top-1`) into a contiguous `char buff` for short-string concatenation. Used when the total length ≤ `LUAI_MAXSHORTLEN`.

---

### `luaV_concat(lua_State *L, int total)`

Concatenate `total` values on the stack into one string. If `total == 1`, returns immediately. Otherwise, repeatedly coalesces pairs: if either operand is not a string, tries coercion (numbers via `luaO_tostring`) or `luaT_tryconcatTM` (for `__concat`). Empty short strings are optimized (the other operand becomes the result).

For string runs, measures total length, checks for overflow (`MAX_SIZE - sizeof(TString) - tl`), then creates either a short string (via `luaS_newlstr` with a stack buffer) or a long string (via `luaS_createlngstrobj`). The loop reduces `total` until one result remains.

---

### `luaV_objlen(lua_State *L, StkId ra, const TValue *rb)`

Implement `ra = #rb`. Tables: uses `luaH_getn` for the primitive length unless a `__len` metamethod exists (checked via `fasttm`). Short strings: returns `shrlen` directly. Long strings: returns `u.lnglen`. Other types: looks up `__len` metamethod; raises a type error if absent.

---

### `luaV_idiv(lua_State *L, lua_Integer m, lua_Integer n)`

Integer floor division `m // n`. C's `/` truncates toward zero, so for negative non-integer quotients the result is corrected by subtracting 1 (tested via `(m ^ n) < 0 && m % n != 0`). Special-cases: `n == 0` raises "divide by zero"; `n == -1` returns `intop(-, 0, m)` to avoid `LUA_MININTEGER / -1` overflow.

---

### `luaV_mod(lua_State *L, lua_Integer m, lua_Integer n)`

Integer modulus `m % n` with **floor-division semantics** (the result has the sign of the divisor). Corrects C's truncation-based `%` when the remainder and divisor have different signs (`(r ^ n) < 0`) by adding `n`. Returns 0 for `n == -1` (avoids `LUA_MININTEGER % -1` overflow). Errors on `n == 0`.

---

### `luaV_modf(lua_State *L, lua_Number m, lua_Number n)`

Float modulus via the platform's `luai_nummod` macro. Returns the result directly.

---

### `luaV_shiftl(lua_Integer x, lua_Integer y)`

Bit shift: positive `y` shifts left, negative `y` shifts right. Shifts at or beyond integer width (`NBITS`) return 0 to avoid undefined behavior. Uses `intop` for unsigned-cast shift operations.

---

### `pushclosure(lua_State *L, Proto *p, UpVal **encup, StkId base, StkId ra)`

Create a new `LClosure` for prototype `p` at stack slot `ra` and initialize its upvalues. Local upvalues (`uv[i].instack`) are linked via `luaF_findupval` — this creates or reuses an open upvalue pointing to the local on the stack. Non-local upvalues come from the enclosing closure's `encup` array by index. Each upvalue is protected by a GC barrier (`luaC_objbarrier`).

---

### `luaV_finishOp(lua_State *L)`

Resume execution of an opcode interrupted by a coroutine yield. Inspects the instruction saved at `*(ci->u.l.savedpc - 1)` and completes the result placement.

**Key cases:**
- **Arithmetic metamethods** (`OP_MMBIN`/`MMBINI`/`MMBINK`): move the result from `L->top - 1` into the destination register by decrementing `L->top`.
- **Unary ops / table gets** (`OP_UNM`, `OP_BNOT`, `OP_LEN`, `OP_GET*`, `OP_SELF`): pop the result from top into the `A` register.
- **Comparisons** (`OP_LT`/`OP_LE`/`OP_EQ` and integer variants): pop the result, test it against `GETARG_k`, and conditionally skip the following `OP_JMP`.
- **`OP_CONCAT`**: restore proper stack positions and re-invoke `luaV_concat` (which may yield again).
- **`OP_CLOSE`/`OP_RETURN`**: decrement `savedpc` to re-execute the instruction (closing remaining variables).
- **`OP_CALL`/`OP_TAILCALL`/`OP_SET*`/`OP_TFORCALL`**: no special action needed — the caller will re-enter the instruction.

---

### `luaV_execute(lua_State *L, CallInfo *ci)`

The main interpreter loop — approximately 770 lines of opcode dispatch. Uses labels `startfunc:` and `returning:` to handle nested Lua calls without recursion: `OP_CALL` and `OP_TAILCALL` jump back to `startfunc` for Lua callees, reusing the same C stack frame. The `ret:` label handles return from any frame; when `CIST_FRESH` is set, the function returns to the C caller.

The `trap` variable is polled via `vmfetch()` — when set, `luaG_traceexec` fires and may even yield (from a hook). The interpreter processes approximately 60 distinct opcodes across the categories of loads, upvalue access, table operations, arithmetic (with K/I variants), metamethod fallbacks, unary ops, string operations, control flow, calls/returns, loops, closures, varargs, and error guards.

**Tricky logic in OP_RETURN:** For vararg functions, `ci->func.p` is adjusted back by `nextraargs + nparams1` to restore the original function position before calling `luaD_poscall`. The `nparams1` field is packed into the instruction's C argument.
