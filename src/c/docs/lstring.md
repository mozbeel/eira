# lstring.c — String Table and String Interning

> **AI-Generated Documentation**

## Overview

`lstring.c` manages all strings and userdata in the Eira Lua 5.5 runtime. Its central responsibility is **string interning**: short strings (≤ `LUAI_MAXSHORTLEN` bytes) are stored in a global hash table (`G(L)->strt`) and deduplicated by content, so two strings with identical bytes share a single `TString` object and can be compared by pointer. Long strings are never interned by default — each creates a fresh `TString` — because their size makes interning expensive.

The file also implements three kinds of long strings: **regular** (`LSTRREG`, contents stored inline after the header), **fixed-external** (`LSTRFIX`, the header wraps a caller-provided pointer that is never freed), and **memory-managed external** (`LSTRMEM`, the header stores a deallocation callback `falloc` and opaque `ud`). This supports FFI scenarios where C-allocated buffers are exposed to Lua without copying.

A small **API string cache** (`strcache[STRCACHE_N][STRCACHE_M]`) avoids repeated interning of C-string literals passed through the API. It is indexed by the pointer address modulo `STRCACHE_N` and uses LRU eviction within each bucket.

The string table itself is a power-of-two array of singly-linked buckets. Each short `TString` stores its hash (computed once at creation via `luaS_hash`) and chains through `u.hnext`. When the table load factor exceeds 100% (`nuse >= size`), it doubles. During GC sweep, dead short strings are unlinked from the table via `luaS_remove` before being freed, and the table is shrunk when `nuse < size / 4`.

The initial string table size is `MINSTRTABSIZE` (128). A pinned `memerrmsg` string is created at init time via `luaC_fix` so it can never be collected — it is used as a placeholder in the API cache and as a fallback during OOM conditions.

## Key Types / Macros

- **`TString`** — The string object header. For short strings, `shrlen` holds the length, contents follow inline. For long strings, `u.lnglen` holds the length and `shrlen` encodes the kind (`LSTRREG`/`LSTRFIX`/`LSTRMEM`).
- **`LSTRREG` (0)** — Regular long string with inline contents. The `contents` pointer points to memory immediately after the `TString` header (at `offsetof(TString, falloc)`).
- **`LSTRFIX` (1)** — Fixed external string; `contents` points to caller-owned memory that must not be freed. The `TString` header is allocated but does not store `falloc`/`ud`.
- **`LSTRMEM` (2)** — External string with a deallocation callback (`falloc`/`ud`) stored in the `TString` header. The GC calls `(*falloc)(ud, contents, len+1, 0)` to release the external buffer.
- **`stringtable`** — The global string table: a hash array `hash` of `TString*` buckets, `size` (bucket count, always power of 2), and `nuse` (number of live strings).
- **`MINSTRTABSIZE` (128)** — Initial string table size, must be power of 2. Large enough to hold the ~50 core reserved words and metaevent keys without immediate resizing.
- **`MAXSTRTB`** — Maximum string table size, clamped to `INT_MAX / sizeof(TString*)`.
- **`STRCACHE_N`, `STRCACHE_M`** — Dimensions of the API string cache. The cache is a 2D array indexed by `point2uint(str) % STRCACHE_N` with `STRCACHE_M` LRU slots per bucket.
- **`u.hnext`** — In short strings, the next-pointer for the string-table bucket chain.
- **`extra`** — Flag byte on long strings: 0 means the hash has not been computed yet; 1 means it is cached in `hash`.

## Functions

### luaS_eqstr(TString *a, TString *b)

Generic string equality: compares lengths and then contents byte-by-byte via `memcmp`. Works across short, long, and external strings alike. The macro `getlstr` extracts the character pointer and length regardless of string kind, dispatching on the `shrlen` field to distinguish short strings from the three long-string variants. This function is used by the table implementation when comparing keys of different string variants (e.g., a long-string key against a short-string node).

### luaS_hash(const char *str, size_t l, unsigned seed)

Computes a deterministic hash of a string buffer. The seed is XOR'd with the length, then each byte is folded in via an XOR-shift mix:

```c
unsigned int h = seed ^ cast_uint(l);
for (; l > 0; l--)
  h ^= ((h<<5) + (h>>2) + cast_byte(str[l - 1]));
```

The seed is per-state (`G(L)->seed`), making string-table bucket distribution unpredictable to external attackers. Short strings use this hash at creation time; long strings store the seed and defer computation.

### luaS_hashlongstr(TString *ts)

Lazily computes and caches a long string's hash on first use. The `extra` field tracks whether the hash has been computed: if `extra == 0`, calls `luaS_hash` with the stored seed and sets `extra = 1`; subsequent calls return the cached `ts->hash` directly.

### tablerehash(TString **vect, int osize, int nsize)

Rebuilds the string table for a new size. First clears all new slots (from `osize` to `nsize`). Then walks each old bucket chain, re-chaining every string into the bucket given by `hash % nsize`. This is called during both growth and shrinking — on shrink, the vanishing portion is depopulated first to maintain correctness.

```c
for (i = 0; i < osize; i++) {
  TString *p = vect[i];
  vect[i] = NULL;
  while (p) {
    TString *hnext = p->u.hnext;
    unsigned int h = lmod(p->hash, nsize);
    p->u.hnext = vect[h];
    vect[h] = p;
    p = hnext;
  }
}
```

### luaS_resize(lua_State *L, int nsize)

Resizes the string table. On shrink, first calls `tablerehash` to depopulate the vanishing buckets. Then calls `luaM_reallocvector` to reallocate the bucket array. If the reallocation fails, rolls back to the original size by rehashing back. On growth, rehashes into the newly allocated (initially NULL) slots. The table size is always kept as a power of two.

### luaS_clearcache(global_State *g)

Scans every entry in the API string cache (`g->strcache[i][j]`) and replaces any entry that the GC would collect (i.e., is white) with the immortal `memerrmsg` string. This prevents dangling pointers in the cache after a GC cycle.

### luaS_init(lua_State *L)

Allocates the initial string table (`MINSTRTABSIZE` = 128 buckets), clears all slots via `tablerehash`, pre-creates the `memerrmsg` string via `luaS_newliteral`, pins it with `luaC_fix` (so it is never collected), and fills the entire string cache with it. Called once during state creation.

### luaS_sizelngstr(size_t len, int kind)

Returns the header byte size for a long string of the given kind:

- `LSTRREG`: `offsetof(TString, falloc) + (len + 1) * sizeof(char)` — includes space for inline content and trailing NUL.
- `LSTRFIX`: `offsetof(TString, falloc)` — only the header; contents are external and not freed.
- `LSTRMEM`: `sizeof(TString)` — full header including `falloc` and `ud` fields; contents are external and freed via callback.

### createstrobj(lua_State *L, size_t totalsize, lu_byte tag, unsigned h)

Low-level allocator for all string types. Calls `luaC_newobj(L, tag, totalsize)` to allocate a GC-managed object, then initializes `ts->hash = h` and `ts->extra = 0`. The caller is responsible for filling in kind-specific fields (length, content pointer, `shrlen`, etc.). The object starts as white and will be managed by the GC like any other collectable.

### luaS_createlngstrobj(lua_State *L, size_t l)

Creates a regular (`LSTRREG`) long string of length `l`. Allocates `luaS_sizelngstr(l, LSTRREG)` bytes, sets `shrlen = LSTRREG` as the kind tag, points `contents` to `cast_charp(ts) + offsetof(TString, falloc)`, and writes a trailing NUL at `contents[l]`. The hash is seeded with `G(L)->seed` but not yet computed (lazy via `luaS_hashlongstr`).

### luaS_remove(lua_State *L, TString *ts)

Unlinks short string `ts` from its bucket chain in the global string table. Walks the chain starting at `tb->hash[lmod(ts->hash, tb->size)]` to find the predecessor of `ts`, then splices `ts` out by setting the predecessor's `u.hnext` to skip `ts`. Decrements `tb->nuse`. Called by `freeobj` in `lgc.c` before freeing a short string.

### growstrtab(lua_State *L, stringtable *tb)

Doubles the string table when `nuse >= size` (load factor ≥ 100%). If `nuse` has reached `INT_MAX`, runs `luaC_fullgc(L, 1)` (emergency collection) first to try to free some strings; raises `luaM_error` if `nuse` is still `INT_MAX` after the collection. Otherwise doubles via `luaS_resize(L, tb->size * 2)`.

### internshrstr(lua_State *L, const char *str, size_t l)

Core short-string interning routine. Hashes the content with `luaS_hash(str, l, g->seed)`, then scans the bucket for a string with matching length and `memcmp`. If found and the match is dead (white), resurrects it via `changewhite(ts)`. If not found, creates a new `TString` via `createstrobj`, sets `shrlen`, copies the content, prepends to the bucket, and increments `nuse`. If `nuse >= size`, calls `growstrtab` first and recomputes the bucket index.

### luaS_newlstr(lua_State *L, const char *str, size_t l)

Creates a string with explicit length. If `l <= LUAI_MAXSHORTLEN`, delegates to `internshrstr` for deduplication. Otherwise, checks for overflow (`l * sizeof(char) >= MAX_SIZE - sizeof(TString)`) and calls `luaS_createlngstrobj`, then copies the content into the inline buffer. Long strings are never interned — each call creates a new `TString`.

### luaS_new(lua_State *L, const char *str)

Creates or reuses a zero-terminated C string. First checks the API cache: hashes the pointer value modulo `STRCACHE_N`, scans `STRCACHE_M` entries for a `strcmp` match. On hit, returns the cached string. On miss, shifts the cache entries down (LRU eviction), creates the string via `luaS_newlstr(L, str, strlen(str))`, and inserts it at the front of the cache bucket. The cache only holds zero-terminated strings, making `strcmp` safe.

### luaS_newudata(lua_State *L, size_t s, unsigned short nuvalue)

Allocates a userdata of byte size `s` with `nuvalue` user values. Checks for overflow (`s > MAX_SIZE - udatamemoffset(nuvalue)`). Allocates via `luaC_newobj(L, LUA_VUSERDATA, sizeudata(nuvalue, s))`, then initializes `len`, `nuvalue`, `metatable = NULL`, and sets all user values to nil. The userdata is registered as a white collectable object.

### f_newext(lua_State *L, void *ud)

Allocation-only helper for external long strings. Runs under `luaD_rawrunprotected` so a memory error can be reported cleanly without leaking the caller's external buffer. Creates just the `TString` header (no content copy) via `createstrobj`. The `struct NewExt` parameter carries the kind, external pointer, length, and output `TString*`.

### luaS_newextlstr(lua_State *L, const char *s, size_t len, lua_Alloc falloc, void *ud)

Wraps caller-provided external memory as a long string. Two modes:

- **`falloc == NULL`** (`LSTRFIX`): Creates a header-only `TString` pointing at the caller's buffer. The GC will **never** free the external buffer — the caller retains ownership.
- **`falloc != NULL`** (`LSTRMEM`): Creates a header via `f_newext` under protection; if the allocation fails, calls `(*falloc)(ud, s, len+1, 0)` to release the external buffer before re-raising the error. On success, stores `falloc` and `ud` in the `TString` so `freeobj` can call `(*falloc)(ud, contents, len+1, 0)` during collection.

### luaS_normstr(lua_State *L, TString *ts)

Normalizes an external string for use as a table key. Long strings (length > `LUAI_MAXSHORTLEN`) pass through unchanged — they are already valid keys as-is. Short strings are copied into a new `TString` and interned into the global string table via `internshrstr`. This is necessary because short-string table lookups use pointer identity (`eqshrstr` is just a pointer comparison), so the string must be registered in the table first.

## Design Notes

### Short vs Long String Interning

Short strings are always interned because they are extremely common (field names, method names, small constants) and pointer-identity comparison makes table lookups O(1) after the hash probe. The tradeoff is that every short string stays alive as long as any reference to it exists in the VM, but since short strings are small, this memory cost is acceptable.

Long strings are never interned because their size makes deduplication expensive (requiring `memcmp` over potentially megabytes), and they are typically used as data rather than structural keys. However, `luaS_normstr` provides a bridge: when a long string is used as a table key, it is copied and interned if it turns out to be short.

### External String Lifecycle

External strings (`LSTRFIX` and `LSTRMEM`) allow C code to expose pre-allocated buffers to Lua without copying. The lifecycle differs:

1. **`LSTRFIX`**: The caller owns the buffer forever. The `TString` header is GC-managed, but the `contents` pointer is not. The caller must ensure the buffer outlives all Lua references.

2. **`LSTRMEM`**: The caller transfers ownership to Lua. When the `TString` is collected, `freeobj` in `lgc.c` calls `(*falloc)(ud, contents, len+1, 0)` to release the buffer. If the allocation of the `TString` header fails during `luaS_newextlstr`, the external buffer is freed immediately to prevent leaks.

### Hash Computation Strategy

Short strings compute their hash once at creation time via `luaS_hash` and store it in `ts->hash`. This makes bucket lookup O(1) (just `hash % tablesize`). Long strings defer the actual hash computation: the `hash` field is initialized with the state seed, and `luaS_hashlongstr` computes the real hash on first access (for table operations), caching it via the `extra` flag.

### API Cache Eviction

The API string cache (`strcache`) uses a simple LRU scheme: each bucket has `STRCACHE_M` slots. On a cache miss, all slots shift down (slot M-1 is evicted), and the new string is inserted at slot 0. This means frequently accessed C-string literals stay cached while rare ones are evicted quickly. The cache is cleared of collectable entries by `luaS_clearcache` at the end of each GC cycle.

### Table Key Normalization

When a string is used as a table key, it must go through `luaS_normstr` if it is an external string. This is because:

- Short-string table lookups (via `luaH_Hgetshortstr`) use pointer identity — two short strings with the same content must be the same object.
- External long strings may point to arbitrary C memory, so they use generic (content-based) comparison.
- If an external string happens to be short, it must be interned first so the short-string fast path can be used.

This normalization happens in `luaH_finishset` when `hres == HNOTFOUND` and the key `isextstr`.

### String Table Sizing

The string table is always a power of two. It starts at 128 buckets and grows by doubling when the load factor reaches 100%. Shrinking occurs when `nuse` drops below `size / 4` (triggered by `checkSizes` in `lgc.c` during the sweep phase). The maximum size is `MAXSTRTB`, which is `INT_MAX / sizeof(TString*)` — on a 64-bit system with 8-byte pointers, this is roughly 268 million buckets.

The resize operation (`luaS_resize`) is not free: it must rehash every string into its new bucket. For this reason, shrinking is conservative (25% threshold) and growth is always by doubling.

### GC Interaction with Strings

Short strings are collectable objects managed by the GC like any other `GCObject`. During a GC cycle, dead short strings (marked with the old white) are freed by `freeobj` in `lgc.c`, which first calls `luaS_remove` to unlink them from the string table, then frees the `TString` memory. The string table is also shrunk during the sweep phase if `nuse` drops below `size / 4`.

Long strings with external memory (`LSTRMEM`) are freed differently: `freeobj` calls `(*ts->falloc)(ts->ud, ts->contents, ts->u.lnglen + 1, 0)` to release the external buffer, then frees the `TString` header. `LSTRFIX` strings only free the header — the external buffer is the caller's responsibility.

The `memerrmsg` string is pinned via `luaC_fix`, which moves it to the `fixedgc` list where it is gray and old forever, immune to collection.

### External String Hashing

External strings (`LSTRFIX` and `LSTRMEM`) participate in table operations like any other string. When used as a table key, they go through the generic lookup path (`getgeneric` / `mainpositionTV`) which calls `luaS_eqstr` for equality. Their hash is computed via `luaS_hashlongstr`, which lazily computes and caches the hash on first access. This means the first table lookup with an external string key is slightly more expensive (hash computation + `memcmp`), but subsequent lookups use the cached hash for O(1) bucket selection.

### Thread Safety of String Operations

The string table is not thread-safe. All operations assume single-threaded access within one Lua state. Multi-state programs (each with their own `global_State`) have independent string tables, so strings created in one state are not visible in another. Cross-state string sharing requires explicit API calls.

### File Organization Summary

The file is organized bottom-up: basic utilities first (`luaS_eqstr`, `luaS_hash`), then table management (`tablerehash`, `luaS_resize`), then string creation (`createstrobj`, `luaS_createlngstrobj`), then interning (`internshrstr`), then public API (`luaS_newlstr`, `luaS_new`), and finally external strings and userdata (`luaS_newextlstr`, `luaS_newudata`, `luaS_normstr`). The `luaS_init` function ties everything together at state creation time.

### String Content Layout

For short strings, the content bytes follow immediately after the `TString` header, with a trailing NUL byte. Access is via `getshrstr(ts)`, which returns a pointer into the same allocation. For `LSTRREG` long strings, the content also follows the header but at `offsetof(TString, falloc)`. For `LSTRFIX` and `LSTRMEM`, `ts->contents` points to external memory managed elsewhere. This layout means short strings and regular long strings are fully self-contained in a single allocation, while external strings are lightweight headers referencing external buffers that are managed by the caller or the GC.
