# lmem.c — Memory Manager Interface

> **AI-Generated Documentation**

## Overview

`lmem.c` is the thin allocation layer between the Eira Lua 5.5 runtime and the host-provided allocator (`lua_Alloc`). Every dynamic memory request in the VM — from small object headers to large arrays — flows through this file. Its primary job is threefold: **(1)** forward all requests to the user-supplied `realloc` function, **(2)** track the **GC debt** (the net difference between allocated and freed bytes) so the garbage collector knows when to step, and **(3)** implement retry-after-emergency-GC logic so that a failed allocation triggers a full collection before giving up.

The file also provides **parser-specific helpers** (`luaM_growaux_`, `luaM_shrinkvector_`) that manage growable arrays used during compilation, and a dedicated `luaM_malloc_` entry point that tags allocations with the collectable-object type for the GC's accounting.

All public functions are thin wrappers; the real complexity lies in the retry and debt bookkeeping. The macros in `lmem.h` provide type-safe shorthand (`luaM_new`, `luaM_newvector`, `luaM_freemem`, etc.) that delegate to these functions. The `lua_Alloc` function itself is set during state creation (`lua_newstate`) and stored in `global_State->frealloc`; the `ud` field is an opaque pointer passed through to every call.

The GC debt mechanism works as follows: whenever memory is allocated, `nsize` bytes are subtracted from `GCdebt` (making it more negative, meaning the collector owes work). Whenever memory is freed, `osize` bytes are added back (making debt more positive, meaning the collector should run soon). When `GCdebt ≤ 0`, `luaC_step` is triggered automatically by `luaC_condGC`.

## Key Types / Macros

- **`callfrealloc(g,block,os,ns)`** — Macro to invoke the user-supplied allocator: `(*g->frealloc)(g->ud, block, os, ns)`. This is the single chokepoint for all host-allocator calls.
- **`cantryagain(g)`** — True when the state is fully built (`completestate(g)`) and the GC is not mid-step (`!g->gcstopem`). Only in this state is it safe to run an emergency collection as a retry strategy.
- **`firsttry(g,block,os,ns)`** — In production, simply calls `callfrealloc`. When `EMERGENCYGCTESTS` is defined, always returns NULL for non-free allocations (size > 0), forcing every allocation to go through the emergency-GC retry path. This is for stress-testing the GC.
- **`MINSIZEARRAY` (4)** — Minimum array size during parsing, avoiding overhead of reallocating to size 1, then 2, then 4. All parser arrays are either resized to exact size or freed when parsing ends.
- **`luaM_error(L)`** — Throws `LUA_ERRMEM` via `luaD_throw`. This is the terminal error for allocation failures.
- **`luaM_toobig(L)`** — Raises a memory error with the message `"memory allocation error: block too big"`. Called when a requested size would overflow `size_t`.
- **`luaM_new(L,t)`** — Allocates a single object of type `t` via `luaM_malloc_`.
- **`luaM_newvector(L,n,t)`** — Allocates an array of `n` elements of type `t`. Does not check for overflow — use `luaM_newvectorchecked` when `n` is untrusted.
- **`luaM_freemem(L,b,s)`** — Frees `b` with explicit size `s`.
- **`luaM_free(L,b)`** — Frees `b` computing its size as `sizeof(*b)`.
- **`luaM_freearray(L,b,n)`** — Frees an array of `n` elements.
- **`luaM_limitN(n,t)`** — Clamps `n` to `MAX_SIZET / sizeof(t)` to prevent overflow when computing byte sizes.
- **`luaM_testsize(n,e)`** — Tests whether multiplying `n` by element size `e` would overflow `size_t`.
- **`luaM_newobject(L,tag,s)`** — Allocates `s` bytes tagged with collectable-object type `tag` (passed as `osize` to the allocator).
- **`luaM_newblock(L, size)`** — Allocates a raw byte block of `size` chars.

## Functions

### luaM_growaux_(lua_State *L, void *block, int nelems, int *psize, unsigned size_elems, int limit, const char *what)

Grows a parser/prototype array by **doubling** (at least `MINSIZEARRAY`) so that `nelems + 1` fits. The growth strategy is:

1. If `nelems + 1 ≤ size`, return immediately (current capacity is sufficient).
2. If `size ≥ limit / 2`, clamp to `limit` (one final growth).
3. Otherwise, double `size` (with a floor of `MINSIZEARRAY`).

If `size ≥ limit`, raises `"too many %s (limit is %d)"`. Updates `*psize` only after a successful reallocation via `luaM_saferealloc_`, ensuring the table stays consistent on failure. The `limit` parameter is pre-clamped by `luaM_limitN` at the call site to prevent overflow in the multiplication `size * size_elems`.

### luaM_shrinkvector_(lua_State *L, void *block, int *size, int final_n, unsigned size_elem)

Shrinks an array to exactly `final_n` elements. Used by prototypes (e.g., the constant array, upvalue array) where the logical size equals the allocation size — the array cannot be partially filled. Calls `luaM_saferealloc_` and raises `LUA_ERRMEM` on failure, since prototypes have no smaller valid size. Updates `*size = final_n` unconditionally after the call.

### luaM_toobig(lua_State *L)

Raises a memory error with the message `"memory allocation error: block too big"`. Called when a requested size would overflow `size_t` — typically detected by `luaM_testsize` or explicit checks in the caller. This is a terminal error (`luaD_throw`); it does not return.

### luaM_free_(lua_State *L, void *block, size_t osize)

Frees `block` (which must have been `osize` bytes) by calling `callfrealloc(g, block, osize, 0)`. Then credits `osize` bytes back to the GC debt: `g->GCdebt += cast(l_mem, osize)`. A positive debt tells the collector to run a step soon. Asserts the invariant `(osize == 0) == (block == NULL)` — a NULL block has zero size and vice versa.

### tryagain(lua_State *L, void *block, size_t osize, size_t nsize)

Retry logic for failed allocations. When `cantryagain(g)` is true (state is complete and GC is not mid-step), calls `luaC_fullgc(L, 1)` — an emergency full collection — then retries the allocator once via `callfrealloc`. Returns NULL if the state is not ready for emergency collection (e.g., during early init) or if the retry still fails. The caller (`luaM_realloc_`) does not update `GCdebt` when this returns NULL.

### luaM_realloc_(lua_State *L, void *block, size_t osize, size_t nsize)

Generic reallocation — the core of the memory manager. Calls `firsttry(g, block, osize, nsize)`. If it fails and `nsize > 0`, calls `tryagain`. On success, updates the GC debt:

```c
g->GCdebt -= cast(l_mem, nsize) - cast(l_mem, osize);
```

This means allocations make debt more negative (collector owes work) and frees make it more positive (collector should step). Returns NULL on failure without updating debt, keeping the GC state consistent. Asserts `(nsize == 0) == (newblock == NULL)`.

### luaM_saferealloc_(lua_State *L, void *block, size_t osize, size_t nsize)

Same as `luaM_realloc_` but raises `LUA_ERRMEM` via `luaM_error(L)` when the reallocation fails. Used by most internal callers that cannot tolerate a NULL return — for example, `resizearray` in `ltable.c` and `luaM_shrinkvector_`. The semantic guarantee is: if this function returns, the pointer is valid.

### luaM_malloc_(lua_State *L, size_t size, int tag)

Allocates a fresh block of `size` bytes. The `tag` parameter identifies the collectable-object type and is passed as `osize` to the allocator (the host allocator can use this for per-type accounting). If `size == 0`, returns NULL immediately without calling the allocator — this is a legal no-op matching C `malloc(0)` semantics. On failure, retries once after an emergency GC via `tryagain`, then raises `LUA_ERRMEM`. Credits `size` bytes to the GC debt on success: `g->GCdebt -= cast(l_mem, size)`.

## Design Notes

### The Allocator Contract

The `lua_Alloc` function pointer stored in `global_State->frealloc` must遵守 these rules:

- `frealloc(ud, p, x, 0)` — free block `p` of old size `x`; returns NULL.
- `frealloc(ud, NULL, x, s)` — allocate `s` bytes (ignoring `x`); returns NULL on failure.
- `frealloc(ud, b, x, y)` — reallocate block `b` from `x` to `y` bytes; returns NULL on failure.

ISO C `realloc(NULL, 0)` is undefined, but Lua's allocator treats it as a no-op (returns NULL, size 0).

### GC Debt and Step Timing

The GC debt (`g->GCdebt`) controls when the collector runs. The key invariant:

```c
// After allocation:
g->GCdebt -= cast(l_mem, nsize) - cast(l_mem, osize);
// After free:
g->GCdebt += cast(l_mem, osize);
```

When `GCdebt ≤ 0`, the runtime calls `luaC_step` via the `luaC_condGC` macro (typically after every allocation). A positive debt means the collector has work to do; a zero or negative debt means the collector can rest. The step function consumes work units proportional to the debt, then sets a new debt for the next step.

### Emergency GC Flow

When `firsttry` returns NULL (allocation failed), `tryagain` runs this sequence:

1. Check `cantryagain(g)` — is the state fully built and the GC not mid-step?
2. If yes, call `luaC_fullgc(L, 1)` — emergency full collection (finalizers are skipped, shrinking is skipped).
3. Retry the allocator once.
4. If still NULL, return NULL to the caller.

The `EMERGENCYGCTESTS` build flag makes `firsttry` always fail for non-free allocations, forcing every allocation through this retry path. This stress-tests the GC's ability to recover from OOM.

### Parser Array Growth

The parser uses `luaM_growaux_` for its dynamic arrays (e.g., the list of locals, the expression stack). The growth pattern is:

- Start with at least `MINSIZEARRAY` (4) elements.
- Double the size on each growth (4 → 8 → 16 → ...).
- Clamp at a per-array `limit` (e.g., `MAXSRC` for source names).
- When `size ≥ limit`, make one final growth to exactly `limit`, then raise an error if more space is needed.

This avoids the overhead of incremental growth (reallocating to 1, then 2, then 4) while staying bounded by the limit.

### Prototype Shrinkage

After parsing completes, prototype arrays (constants, upvalues, debug info) are shrunk to their exact size via `luaM_shrinkvector_`. This saves memory: during parsing the arrays are oversized to avoid repeated reallocation, but the final `Proto` should only hold what it needs. The shrink is mandatory — if the realloc fails, it raises an error because the oversized array cannot be kept (the size field must match the allocation).

### Convenience Macros

The macros in `lmem.h` provide type-safe wrappers:

- `luaM_new(L, t)` — single object: `luaM_malloc_(L, sizeof(t), 0)`
- `luaM_newvector(L, n, t)` — array: `luaM_malloc_(L, n * sizeof(t), 0)`
- `luaM_newobject(L, tag, s)` — tagged alloc: `luaM_malloc_(L, s, tag)`
- `luaM_free(L, b)` — free with sizeof: `luaM_free_(L, b, sizeof(*b))`
- `luaM_freemem(L, b, s)` — free with explicit size: `luaM_free_(L, b, s)`
- `luaM_freearray(L, b, n)` — free array: `luaM_free_(L, b, n * sizeof(*b))`
- `luaM_growvector(L, v, n, s, t, lim, e)` — grow with limit check
- `luaM_shrinkvector(L, v, s, fs, t)` — shrink to exact size
- `luaM_reallocvector(L, v, old, new, t)` — realloc array

All macros expand to calls that handle the size arithmetic internally, preventing overflow in the multiplication `n * sizeof(t)`. For untrusted counts, `luaM_newvectorchecked` first runs `luaM_checksize` which calls `luaM_toobig` if the multiplication would overflow.

### Size Overflow Protection

The macros `luaM_testsize(n,e)` and `luaM_checksize(L,n,e)` prevent overflow when computing `n * sizeof(e)`. The test uses a trick to avoid compiler warnings:

```c
#define luaM_testsize(n,e) \
  (sizeof(n) >= sizeof(size_t) && cast_sizet((n)) + 1 > MAX_SIZET/(e))
```

The `+1` prevents the compiler from optimizing the comparison to a constant (since the original comparison is always false for narrow types). For `char` arrays (`luaM_reallocvchar`), no test is needed since `sizeof(char) == 1`.

The macro `luaM_limitN(n,t)` clamps `n` to `MAX_SIZET / sizeof(t)`, ensuring the result can safely be multiplied by `sizeof(t)` without overflowing `size_t`. This is used at call sites that pass limits to `luaM_growaux_`.

### Interaction with the GC

Every allocation and free updates `g->GCdebt`, creating a feedback loop with the garbage collector. The `luaC_condGC` macro (typically expanded after every `luaM_malloc_` or `luaM_saferealloc_` call) checks if debt has dropped to zero or below and triggers `luaM_step`. This ensures the collector runs proportionally to allocation rate, preventing any single large allocation from causing a long GC pause.

In generational mode, the debt mechanism is different: `setminordebt` arms a fixed debt based on the live byte count from the last major collection. In incremental mode, `setpause` computes a debt based on the `GCPAUSE` parameter and the bytes marked in the last cycle.

### State Building Constraint

The `cantryagain(g)` check is critical: it uses `completestate(g)` to verify the state is fully built (the main thread exists, the registry is populated, the GC is initialized). During early state creation (e.g., `lua_newstate`), calling `luaC_fullgc` would crash because the collector's internal lists are not yet set up. Similarly, `g->gcstopem` prevents re-entrant GC — if the collector is already running a step, starting another full collection from within an allocation callback would corrupt the GC state.

### Return Value Semantics

The allocation functions have different return-value contracts:

- `luaM_realloc_` returns NULL on failure — the caller must handle the error (typically by propagating it up).
- `luaM_saferealloc_` never returns NULL — it raises `LUA_ERRMEM` on failure. This is the preferred function for internal allocations where there is no sensible fallback.
- `luaM_malloc_` never returns NULL for non-zero sizes — it raises `LUA_ERRMEM`. Size 0 returns NULL without calling the allocator.
- `luaM_growaux_` can raise (if the limit is exceeded) or return the same block pointer (if capacity suffices) or a new pointer (after reallocation).
- `luaM_shrinkvector_` never returns NULL — it raises on failure.

### Memory Accounting Summary

| Operation | `GCdebt` effect | Notes |
|-----------|-----------------|-------|
| `luaM_realloc_` grow | `debt -= (nsize - osize)` | More debt = collector owes work |
| `luaM_realloc_` shrink | `debt += (osize - nsize)` | Less debt = collector can step |
| `luaM_free_` | `debt += osize` | Frees credit back to debt |
| `luaM_malloc_` | `debt -= size` | New allocation costs debt |

This accounting ensures the GC step rate is proportional to the allocation rate, regardless of whether memory is being freed or allocated.

### Why Two Free Functions?

`luaM_free_` takes an explicit size parameter because not all callers know the size via `sizeof(*block)`. For example, freeing a variable-length structure (like a `Proto` with N constants) requires the caller to compute the size. The convenience macros `luaM_free(L, b)` and `luaM_freearray(L, b, n)` compute the size automatically when it can be determined from the type or element count.

### Allocation Tag Purpose

The `tag` parameter in `luaM_malloc_(L, size, tag)` serves the host allocator: it receives `tag` as the `osize` argument, allowing the allocator to track per-type memory usage. When `tag == 0`, the allocation is for non-collectable memory (internal VM structures). Collectable objects pass their type tag (`LUA_VTABLE`, `LUA_VLCL`, etc.) so the allocator can maintain type-specific statistics or pools.

### File Organization Summary

The file is structured from general to specific: the allocator contract is documented first (comments at the top), followed by the `cantryagain` / `firsttry` macros that control retry behavior, then the parser array helpers (`luaM_growaux_`, `luaM_shrinkvector_`), and finally the core allocation functions (`luaM_free_`, `luaM_realloc_`, `luaM_saferealloc_`, `luaM_malloc_`). The convenience macros in `lmem.h` sit above this file and are the primary entry points for most callers.
