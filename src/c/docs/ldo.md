# ldo.c — Stack management, call dispatch, protected calls, and coroutine support

> **AI-Generated Documentation**

## Overview

`ldo.c` manages the Lua call stack and implements all mechanisms for calling functions (C and Lua), handling errors via protected calls, and driving coroutine resume/yield. It sits between the public API (`lapi.c`) and the interpreter (`lvm.c`), providing the infrastructure that makes function calls, error recovery, and coroutine switching possible. The file is approximately 1315 lines and is one of the most architecturally complex files in the Lua core.

The central abstraction is the **CallInfo** frame linked list: each active function call (C or Lua) has a `CallInfo` struct that records the function slot, packed result count, saved PC (for Lua frames), and status flags (`CIST_C`, `CIST_TAIL`, `CIST_HOOKED`, `CIST_YPCALL`, `CIST_FRESH`, etc.). Lua functions are executed by re-entering `luaV_execute` in the caller's C stack frame — rather than recursing, so deep Lua call chains use only one C stack frame. C functions are called directly by `precallC`, which sets up the frame and invokes the function pointer while the state is unlocked.

Error handling uses `setjmp`/`longjmp` (or C++ exceptions when compiling as C++, or `_setjmp`/`_longjmp` on POSIX). A chain of `lua_longjmp` structures threaded through `L->errorJmp` provides nested protected calls. `luaD_pcall` is the main protected-call primitive used by the public `lua_pcallk` API. When an error occurs and no handler exists on the current thread, the error is forwarded to the main thread; if the main thread also has no handler, the panic function is called.

Stack management handles growth (1.5x strategy with `MAXSTACK` cap), shrinkage (after peaks, down to 2x use), and reallocation safety. During reallocation, all pointers into the stack are converted to offsets (`relstack`) and restored afterward (`correctstack`), with Lua frames flagged via `trap` so the interpreter re-syncs `base`. `ERRORSTACKSIZE` (= `MAXSTACK + STACKERRSPACE`) provides extra room for error handling even after overflow.

Coroutine support is built around `lua_resume`, which drives `unroll` — a loop that resumes every interrupted frame (Lua or C) in a coroutine until the stack is empty, another yield occurs, or an unrecoverable error is raised. C functions can use continuations (`lua_KFunction`) to cooperate with yields. The `precover` function handles errors during unroll by finding the next protected call frame and retrying, enabling nested `pcall` inside coroutines to recover from errors gracefully.

## Key Types / Macros

| Name | Purpose |
|------|---------|
| `lua_longjmp` | Chained longjmp buffer: `jmp_buf b`, `TStatus status`, `previous` pointer |
| `STACKERRSPACE` (200) | Extra stack slots reserved for error handling after overflow |
| `MAXSTACK` | Minimum of `LUAI_MAXSTACK` (1000000) and `size_t`-safe maximum |
| `ERRORSTACKSIZE` | `MAXSTACK + STACKERRSPACE` — used when the stack overflows |
| `LUAI_MAXCCALLS` (200) | Maximum nested C calls before stack overflow check |
| `next_ci(L)` | Get or allocate the next CallInfo frame (calls `luaE_extendCI` if list exhausted) |
| `errorstatus(s)` | True when `s > LUA_YIELD` (real error, not yield or OK) |
| `nyci` | C-call increment that also marks the call as non-yieldable |
| `MAX_CCMT` | Maximum `__call` metamethod chain depth before raising an error |
| `Pfunc` | Function pointer type `void (*)(lua_State *L, void *ud)` for protected functions |

## Functions

### `luaD_seterrorobj(lua_State *L, TStatus errcode, StkId oldtop)`

Place the error object at `oldtop`. For memory errors (`LUA_ERRMEM`), uses the pre-registered `memerrmsg` string — this is critical because OOM means no further allocation is possible. For other errors, moves the error object from `L->top - 1` down to `oldtop` via `setobjs2s`. Sets `L->top.p = oldtop + 1`. Asserts that the error object is non-nil.

**Parameters:**
- `errcode`: the error status (determines which branch to take).
- `oldtop`: the stack position where the error object should end up.

---

### `luaD_throw(lua_State *L, TStatus errcode)`

Raise an error via longjmp. Three-tier fallback:
1. If the thread has an error handler (`L->errorJmp`), sets its `status` and calls `LUAI_THROW`.
2. If no handler exists, calls `luaE_resetthread` (closing all upvalues), stores the error status, copies the error object to the main thread's handler, and re-throws there.
3. If the main thread also has no handler, calls `g->panic` if registered (giving the host a last chance to jump out), then `abort()`.

This function never returns (`l_noret`).

---

### `luaD_throwbaselevel(lua_State *L, TStatus errcode)`

Like `luaD_throw` but first walks the `errorJmp` chain back to the outermost handler (`previous == NULL`). Used when internal errors must bypass nested handlers to reach the base-level error handler — for example, when an error occurs inside a debug hook or error handler itself.

---

### `luaD_rawrunprotected(lua_State *L, Pfunc f, void *ud)`

Run `f(L, ud)` under a fresh `longjmp` handler. Chains a new `lua_longjmp` onto `L->errorJmp`, calls `LUAI_TRY` (which expands to `setjmp`/`longjmp` or C++ try/catch), then restores the previous handler. Saves and restores `L->nCcalls` since yields between save and restore can run arbitrary C code that changes the count.

**Returns:** `LUA_OK` on normal completion, or the error/yield status from `lj.status`.

---

### `luaD_errerr(lua_State *L)`

Push the fixed string `"error in error handling"` and throw `LUA_ERRERR`. Called when the message handler itself fails. Must fit in `EXTRA_STACK` space since no further allocation is possible at this point. Never returns.

---

### `luaD_checkminstack(lua_State *L)`

Verify enough space exists to run a simple function (such as a finalizer): two free CallInfos (allocated via `luaE_extendCI` if the list is short), C-stack slots (checked via `getCcalls`), and `BASIC_STACK_SIZE` stack slots.

**Returns:** 1 if there is enough space, 0 if any check fails.

---

### `relstack(lua_State *L)`

Convert all pointers into the stack into byte offsets from `L->stack.p`: `L->top`, `L->tbclist`, open upvalues (`up->v`), and every CallInfo's `top` and `func`. Only used in strict-ISO mode (`LUAI_STRICT_ADDRESS`). In non-strict mode, this is a no-op — the old pointer address is used directly after deallocation (technically UB but works everywhere in practice).

---

### `correctstack(lua_State *L, StkId oldstack)`

Restore all stack pointers from offsets (or adjust from old base in non-strict mode). Sets `ci->u.l.trap = 1` on every Lua frame so the interpreter re-syncs `base` from `ci->func` on its next instruction fetch. This is essential because the interpreter caches `base` as a local variable inside `luaV_execute`.

---

### `luaD_reallocstack(lua_State *L, int newsize, int raiseerror)`

Reallocate the value stack to `newsize + EXTRA_STACK` entries. GC is frozen during the move (`gcstopem = 1`) to prevent emergency collections while pointers are invalid. Calls `relstack`, reallocates via `luaM_reallocvector`, then `correctstack`. On allocation failure, restores pointers and either raises (`luaM_error`) or returns 0. Erases the new segment with nils.

**Returns:** 1 on success, 0 on failure (when `raiseerror` is 0).

---

### `luaD_growstack(lua_State *L, int n, int raiseerror)`

Grow the stack by at least `n` slots using a 1.5x growth strategy (`newsize = size + (size >> 1)`). Clamps to `MAXSTACK`. If already at `ERRORSTACKSIZE` (the thread is handling a stack overflow), raises `luaD_errerr` for "stack overflow inside error handler". Otherwise reallocates to the needed size, or to `ERRORSTACKSIZE` and raises "stack overflow".

**Tricky logic:** The `needed` computation (`L->top.p - L->stack.p + n`) must be checked against `MAXSTACK` to avoid arithmetic overflow on the `newsize` calculation.

---

### `stackinuse(lua_State *L)`

Walk all CallInfo frames and return the maximum `top` observed (as a slot count), with `LUA_MINSTACK` as floor. Used by `luaD_shrinkstack` to determine whether shrinking would be beneficial.

---

### `luaD_shrinkstack(lua_State *L)`

If the stack is more than 3x the current use (or larger than `MAXSTACK`), shrink it to 2x current use (or `MAXSTACK`). Failure to reallocate is tolerated — the stack just stays larger. Also trims the CallInfo list via `luaE_shrinkCI` to free unused frames.

---

### `luaD_hook(lua_State *L, int event, int line, int ftransfer, int ntransfer)`

Call the debug hook `L->hook` for the given event. Saves `L->top` and `ci->top` as stack offsets. For Lua frames, raises `ci->top` to protect the entire activation register (preventing GC from collecting live values). Ensures `LUA_MINSTACK` is available. Disables reentrancy (`allowhook = 0`, sets `CIST_HOOKED`), unlocks the state so the hook can call back into Lua, invokes the hook, then re-locks and restores everything.

**Tricky logic:** The `ftransfer`/`ntransfer` parameters communicate which results were transferred for return hooks, allowing the hook to inspect both the caller's and callee's views of the stack.

---

### `luaD_hookcall(lua_State *L, CallInfo *ci)`

Run the call hook for a Lua function about to start. Distinguishes tail calls (`CIST_TAIL → LUA_HOOKTAILCALL`). Temporarily increments `ci->u.l.savedpc` so the hook sees the "next" instruction as current (hooks expect `savedpc` to already be advanced). Passes `p->numparams` as the argument count. Restores `savedpc` afterward.

---

### `rethook(lua_State *L, CallInfo *ci, int nres)`

Run the return hook for a finishing call. For vararg Lua functions (`PF_VAHID` flag), adjusts `func` back to the virtual function position by adding `nextraargs + numparams + 1`. Calculates `ftransfer` as the offset from the (virtual) function to the first result. Always updates `L->oldpc` for the caller's line hook, even when no return hook is installed.

---

### `tryfuncTM(lua_State *L, StkId func, unsigned status)`

Handle calls to non-function values by fetching the `__call` metamethod via `luaT_gettmbyobj`. Shifts all arguments up by one slot (using a loop from `L->top` down to `func + 1`) to make room, places the metamethod in the `func` slot, and increments the `CIST_CCMT` counter packed into `status`. Raises "call error" if no metamethod exists. Raises "__call chain too long" if `MAX_CCMT` is reached.

**Returns:** the updated `status` with incremented metamethod count. The caller must `goto retry` to re-dispatch.

---

### `genmoveresults(lua_State *L, StkId res, int nres, int wanted)`

Move `nres` results from the top of the stack down to `res`. If `nres > wanted`, truncates (discards extras). If `nres < wanted`, pads the remaining slots with nil. Sets `L->top.p = res + wanted`. This is the generic (slow) path used for complex cases.

---

### `moveresults(lua_State *L, StkId res, int nres, l_uint32 fwanted)`

Fast dispatcher for moving results, handling common cases inline to avoid function call overhead:
- **0 results** (`fwanted == 0+1`): sets `top = res`, returns.
- **1 result** (`fwanted == 1+1`): moves one result or nil, sets `top = res + 1`, returns.
- **`LUA_MULTRET`** (`fwanted == LUA_MULTRET+1`): all results via `genmoveresults`.
- **Default** (two+ results and/or TBC): extracts `wanted` from `fwanted`, closes to-be-closed variables if `CIST_TBC` is set (may yield — `CIST_CLSRET` preserves `nres` across the yield), fires the return hook after `__close` methods complete, then calls `genmoveresults`.

---

### `luaD_poscall(lua_State *L, CallInfo *ci, int nres)`

Finish a function call. Fires the return hook (via `rethook`) unless TBC variables are pending (handled later after closing). Calls `moveresults` to place results in the caller's function slot. Asserts the function is not in a hooked/yielding/finishing state. Pops the CallInfo: `L->ci = ci->previous`.

---

### `prepCallInfo(lua_State *L, StkId func, unsigned status, StkId top)`

Allocate (from the CI list via `next_ci`) and initialize a new `CallInfo` frame. Sets `ci->func = func`, `ci->callstatus = status` (packed nresults + `CIST_C` flag + `__call` metamethod count), and `ci->top = top`. Makes `ci` the current frame (`L->ci = ci`). Returns the new CallInfo.

---

### `precallC(lua_State *L, StkId func, unsigned status, lua_CFunction f)`

Set up a C-call frame and invoke `f` immediately. Ensures `LUA_MINSTACK` via `checkstackp` (preserving `func` across potential reallocation), creates the CallInfo via `prepCallInfo`, fires the call hook if `LUA_MASKCALL` is set, unlocks the state, calls `n = f(L)`, re-locks, and finishes via `luaD_poscall`.

**Returns:** the number of results.

---

### `luaD_pretailcall(lua_State *L, CallInfo *ci, StkId func, int narg1, int delta)`

Prepare a tail call by reusing the current `CallInfo` frame. `delta` is the difference between the virtual and real function position (for vararg functions).

For **C closures** and **light C functions**, runs immediately via `precallC`. For **Lua functions**, moves function and arguments down into the caller's frame, fills missing fixed params with nil, sets `ci->top`, `ci->u.l.savedpc = p->code`, marks `CIST_TAIL`, and returns -1 to signal `luaV_execute` to re-enter the loop. Non-function values retry through `tryfuncTM`.

---

### `luaD_precall(lua_State *L, StkId func, int nresults)`

Prepare a regular (non-tail) call. C functions (both `LUA_VCCL` closures and `LUA_VLCF` light functions) run immediately via `precallC` and return NULL. Lua functions get a new `CallInfo` via `prepCallInfo`, with `savedpc` set to `p->code` (the start of the function's bytecode), missing fixed params padded with nil, and `top` set to `func + 1 + maxstacksize`.

**Returns:** the `CallInfo*` for `luaV_execute`, or NULL for C calls.

---

### `ccall(lua_State *L, StkId func, int nResults, l_uint32 inc)`

Internal call driver. Bumps `L->nCcalls` by `inc`. If `getCcalls(L) >= LUAI_MAXCCALLS`, checks C-stack depth via `luaE_checkcstack`. Calls `luaD_precall`: if it returns a `CallInfo` (Lua function), marks `CIST_FRESH` and enters `luaV_execute`. Decrements `nCcalls` on return. The `inc` parameter is 1 for ordinary calls or `nyci` (which adds a yield-forbid bit plus the C-call count).

---

### `luaD_call(lua_State *L, StkId func, int nResults)`

Public C-API wrapper for `ccall` with `inc = 1` (yieldable call). Used throughout the C API for internal calls.

---

### `luaD_callnoyield(lua_State *L, StkId func, int nResults)`

Call variant that forbids yields (used for error handlers and finalizers). Passes `nyci` as the increment. If the callee attempts to yield, it will raise "attempt to yield across a C-call boundary".

---

### `finishpcallk(lua_State *L, CallInfo *ci)`

Finish a `lua_pcallk` that was interrupted. Retrieves the original error status from `CIST_RECST`. On success (no error), returns `LUA_YIELD`. On error, closes to-be-closed variables via `luaF_close` (which may yield or error again), reinstalls the error object via `luaD_seterrorobj`, and shrinks the stack. Clears `CIST_YPCALL` and restores `L->errfunc`.

**Tricky logic:** Preserves `CIST_RECST` across reentries so repeated calls close one `__close` method per invocation. The `ci` pointer must be valid across multiple entries because `luaF_close` can yield.

---

### `finishCcall(lua_State *L, CallInfo *ci)`

Resume a C function whose execution was interrupted by a yield. Two cases:
1. If `CIST_CLSRET` is set (was closing TBC variables in `moveresults`), simply redoes `luaD_poscall` with the saved result count.
2. Otherwise, calls `finishpcallk` if it was a `lua_pcallk`, adjusts results, invokes the C continuation function `ci->u.c.k(L, APIstatus(status), ci->u.c.ctx)`, and finishes with `luaD_poscall`.

---

### `unroll(lua_State *L, void *ud)`

Resume every interrupted frame in a coroutine. Walks the CallInfo stack from `L->ci` down to `&L->base_ci`. C functions are completed via `finishCcall`. Lua functions have `luaV_finishOp` called to complete the interrupted instruction, then `luaV_execute` re-enters the loop to execute remaining instructions. The loop stops when the base frame is reached (normal return), another yield occurs (longjmp with `LUA_YIELD`), or an error occurs.

---

### `findpcall(lua_State *L)`

Walk the CallInfo list to find the innermost frame with `CIST_YPCALL` — the "recover point" for error recovery during resume. Returns the CallInfo or NULL if no protected call is pending.

---

### `resume_error(lua_State *L, const char *msg, int narg)`

Report an error in `lua_resume` itself (not the coroutine body). Pops `narg` arguments from the stack, pushes the error message string via `luaS_new`, and returns `LUA_ERRRUN`.

---

### `resume(lua_State *L, void *ud)`

Protected body of `lua_resume`. For a **fresh** coroutine (`L->status == LUA_OK`), calls its function via `ccall` with `inc = 0`. For a **yielding** coroutine (`L->status == LUA_YIELD`), marks it OK, then resumes: if the interrupted frame is Lua and yielded inside a hook (`CIST_HOOKYIELD`), undoes the hook-incremented `savedpc` and re-enters `luaV_execute`; if it's a C frame with a continuation, calls `ci->u.c.k`, then `luaD_poscall`. After resuming the first frame, calls `unroll` to complete all remaining suspended frames.

---

### `precover(lua_State *L, TStatus status)`

Continue `unroll` across recoverable errors. Each error longjumps out; if `findpcall` finds a pending protected call, the error status is stored in `CIST_RECST` and `unroll` is retried via `luaD_rawrunprotected`. This loop continues until normal end (`LUA_OK`), yield (`LUA_YIELD`), or unrecoverable error (no `pcall` found).

---

### `lua_resume(lua_State *L, lua_State *from, int nargs, int *nresults)`

Public API to start or resume a coroutine. Validates state: `LUA_OK` requires base-level with a function; `LUA_YIELD` means resuming; any other status means dead. Inherits C-call count from `from` (or 0 if no caller). Runs `resume` under protection via `luaD_rawrunprotected`, then `precover` for error recovery. On unrecoverable error, marks the thread dead (`L->status = status`), pushes the error object via `luaD_seterrorobj`, and updates `ci->top`.

**Returns:** API status code. Sets `*nresults` to the yield result count or the number of values on the stack.

---

### `lua_isyieldable(lua_State *L)`

Returns whether the current thread can yield: true if not on the main thread and not inside a non-yieldable C call (checked via `yieldable(L)`).

---

### `lua_yieldk(lua_State *L, int nresults, lua_KContext ctx, lua_KFunction k)`

Yield from the running C function. Stores `nresults` in `ci->u2.nyield`. For C continuations, saves `k` and `ctx` in the CallInfo. Hooks may only yield with zero values and no continuation (checked via `api_check`). Calls `luaD_throw(L, LUA_YIELD)` to longjmp back to the resume point. Returns 0 only when called from a hook (the throw is skipped and control returns to `luaD_hook`).

---

### `closepaux(lua_State *L, void *ud)`

Protected wrapper that calls `luaF_close` once, extracting level and status from the `CloseP` payload struct.

---

### `luaD_closeprotected(lua_State *L, ptrdiff_t level, TStatus status)`

Run `luaF_close` in a loop under protection. If a `__close` method errors, restores the saved `ci` and `allowhook` and retries. Continues until no more errors occur, then returns the original status or a new error status if `__close` itself errored and no more closable variables remain.

---

### `luaD_pcall(lua_State *L, Pfunc func, void *u, ptrdiff_t old_top, ptrdiff_t ef)`

Main protected-call primitive. Saves `ci`, `allowhook`, and `errfunc`; sets `errfunc = ef`. Runs `func` under `luaD_rawrunprotected`. On error: restores `ci` and `allowhook`, calls `luaD_closeprotected` to close TBC variables down to `old_top`, pushes the error object via `luaD_seterrorobj`, and shrinks the stack. Restores `errfunc` in all cases.

**Returns:** the status (`LUA_OK` on success, error/yield status otherwise).

---

### `checkmode(lua_State *L, const char *mode, const char *x)`

Raise a syntax error if the chunk kind (`"text"` or `"binary"`) is not found in the load `mode` string via `strchr`. Used by `f_parser` to enforce load restrictions.

---

### `f_parser(lua_State *L, void *ud)`

Protected function for chunk loading. Creates an anchor table on the stack (survives GC during parsing), reads the first character to determine binary (`LUA_SIGNATURE[0]`) vs text, then either undumps (`luaU_undump`) or compiles (`luaY_parser`). Both paths receive the anchor table for GC anchoring. Restores the stack and pushes the resulting closure with all upvalues initialized via `luaF_initupvals`.

---

### `luaD_anchorobj(lua_State *L, Table *anchor, GCObject *obj)`

Anchor a GC object during a protected call by inserting it into the `anchor` table (key == object, value == object). Temporarily pushes the object onto the stack first to guard against emergency GC during the `luaH_set` call, then pops it. The GC barrier from `luaH_set` handles the rest.

---

### `luaD_protectedparser(lua_State *L, ZIO *z, const char *name, const char *mode)`

Parse a chunk under `luaD_pcall`. Initializes the `SParser` structure with the ZIO stream, scanner buffer, and dynamic parser data (actvar, gt, label arrays — all initialized to NULL/0). Forbids yields during parsing via `incnny`. Calls `f_parser`, then frees all dynamic data in all cases (success or error) to prevent memory leaks.
