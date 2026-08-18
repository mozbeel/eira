# ltable.c — Hash Table Implementation

> **AI-Generated Documentation**

## Overview

`ltable.c` implements Lua tables, the sole mutable data structure in the Eira runtime. Each table has two parts: an **array part** (contiguous slots for integer keys 1..`asize`) and a **hash part** (a chained scatter table using Brent's variation for collision resolution). The array part holds keys where more than half of the slots between 1 and n are occupied; everything else goes to the hash part.

The hash part is always a power-of-two sized node array. Each `Node` holds a key, a value, and a `next` offset forming a singly-linked collision chain. A critical invariant is maintained: if a key is not in its "main position" (the node its hash maps to), then the colliding element *is* in its own main position. This ensures good performance even at 100% load factor. Large hash tables (size ≥ 8 nodes) store a `lastfree` pointer just before the node array for O(1) free-slot discovery.

The table length operator (`#t`) is implemented via a hinted binary search: a cached `lenhint` stored in the table accelerates the common `t[#t+1]=v` and `t[#t]=nil` patterns. Rehashing counts keys in each power-of-two slice of the array to find the optimal size that packs the most keys while staying at most 3× cheaper than storing them in the hash part (`arrayXhash`).

## Key Types / Macros

- **`Node`** — A hash-part slot containing a key (`TValue`), value (`TValue`), and `next` offset.
- **`dummynode`** — A shared static singleton used as the hash part when no hash slots are allocated. Its key is `DEADKEY`/`NULL`, ensuring no real key can match it.
- **`Limbox`** — A union stored before the node array in large hash tables, holding the `lastfree` pointer with proper alignment.
- **`haslastfree(t)`** — True when `lsizenode ≥ LIMFORLAST` (i.e., ≥ 8 nodes).
- **`LIMFORLAST`** — Log₂ of the minimum hash size (8) that gets a `lastfree` field.
- **`Counters`** — Rehash state: per-power-of-two histogram `nums[]`, total key count, array-index count `na`, and a deleted-slot flag.
- **`MAXASIZE`, `MAXHSIZE`** — Maximum sizes for array and hash parts, clamped to fit `size_t`.
- **`arrayXhash(na,nh)`** — True when `na ≤ nh * 3` — the memory-efficiency test for array vs hash storage.
- **`absentkey`** — A sentinel `TValue` returned when a key lookup fails (never a valid Lua value).

## Functions

### hashint(const Table *t, lua_Integer i)

Hashes an integer key using `%` (modulo). Uses signed remainder when the integer fits in an `int` for speed; otherwise falls back to unsigned remainder to avoid negative results.

### l_hashfloat(lua_Number n)

Hashes a float by extracting its mantissa and exponent via `frexp`, blending them with `INT_MIN`. Non-finite values (inf, NaN) map to 0.

### mainpositionTV(const Table *t, const TValue *key)

Returns the main (home) node for `key` by dispatching on the key's variant tag: integers use `hashint`, floats use `l_hashfloat`, short strings use their precomputed hash, long strings use `luaS_hashlongstr`, booleans/lightuserdata/functions use pointer-based hashes, and GC objects use pointer identity.

### mainpositionfromnode(const Table *t, Node *nd)

Extracts the key from node `nd` and computes its main position. Used during Brent-invariant maintenance.

### equalkey(const TValue *k1, const Node *n2, int deadok)

Raw (no-metamethod) equality between lookup key `k1` and node key `n2`. Handles short/long string mixing (via `luaS_eqstr`) and, with `deadok`, dead collectable keys compared by identity.

### getgeneric(Table *t, const TValue *key, int deadok)

Walks the collision chain from the main node, returning the value if `key` is found or `&absentkey` otherwise. This is the generic (non-array) lookup path.

### checkrange(lua_Integer k, unsigned limit)

Converts integer `k` to a C array index (1-based) if it falls in `[1, limit]`, else returns 0. The unsigned arithmetic makes the comparison overflow-safe.

### keyinarray(Table *t, const TValue *key)

Returns the array-part index for an integer key, or 0 if the key is not an integer or is outside the array range.

### findindex(lua_State *L, Table *t, TValue *key, unsigned asize)

Turns a `next` key into an iteration index: nil starts traversal, array keys map to `[1..asize]`, hash nodes are numbered after `asize`. Raises an error for invalid keys.

### luaH_next(lua_State *L, Table *t, StkId key)

Implements Lua's `next()`: given the current key at stack slot `key`, stores the following (key, value) pair in the next two slots and returns 1, or 0 at the end of traversal. Scans array first, then hash part.

### sizehash(Table *t)

Returns the byte size of the hash-part block: `sizenode(t) * sizeof(Node)` plus the optional `lastfree` header.

### freehash(lua_State *L, Table *t)

Frees the hash-part block, backing up over the `lastfree` header if present.

### computesizes(Counters *ct)

Computes the optimal array-part size. Walks power-of-two slices, accumulating array-capable keys while the memory cost stays ≤ 3× the hash equivalent. Updates `ct->na` with the count of keys that will go to the array part.

### countint(lua_Integer key, Counters *ct)

If `key` is a valid array index, increments the appropriate power-of-two histogram bucket and `ct->na`.

### arraykeyisempty(const Table *t, unsigned key)

Returns true when 1-based array slot `key` holds an empty (nil) value.

### numusearray(const Table *t, Counters *ct)

Counts used slots per power-of-two slice of the array part, accumulating into the histogram.

### numusehash(const Table *t, Counters *ct)

Counts hash-part keys, flagging deleted slots (nil values with non-nil keys) and adding array-capable integers to the histogram.

### concretesize(unsigned int size)

Returns the byte count of an array part with `size` slots: `size * (sizeof(Value) + 1) + sizeof(unsigned)` (values + tags + alignment).

### resizearray(lua_State *L, Table *t, unsigned oldasize, unsigned newasize)

Reallocates the array part, moving the overlapping portion. Returns the new `array` pointer (NULL when erased). Does a full copy because the pointer offset shifts.

### setnodevector(lua_State *L, Table *t, unsigned size)

Creates the hash part. Size 0 uses the shared `dummynode`; otherwise rounds up to a power of two, allocates nodes (plus `lastfree` when large), and initializes every node as empty.

### reinserthash(lua_State *L, Table *ot, Table *t)

Reinserts every live entry from the old hash part `ot` into the new table `t` after rehashing.

### exchangehashpart(Table *t1, Table *t2)

Swaps the hash parts of two tables (node pointer, size, dummy flag). Used during resize to temporarily borrow a table's hash.

### reinsertOldSlice(Table *t, unsigned oldasize, unsigned newasize)

Moves entries from the shrinking array slice `[newasize, oldasize)` into the hash part.

### clearNewSlice(Table *t, unsigned oldasize, unsigned newasize)

Marks newly grown array slots as empty.

### luaH_resize(lua_State *L, Table *t, unsigned newasize, unsigned nhsize)

Resizes both parts: builds the new hash first so a failing array allocation leaves the table unchanged. Migrates shrinking-array entries to hash, allocates the new array, re-inserts old hash entries, and frees the old hash.

### luaH_resizearray(lua_State *L, Table *t, unsigned int nasize)

Resizes only the array part, keeping the current hash-part size.

### rehash(lua_State *L, Table *t, const TValue *ek)

Full rehash counting the extra key `ek` plus all existing keys. Computes optimal array size, adds 25% headroom to hash when deleted slots exist, then calls `luaH_resize`.

### luaH_new(lua_State *L)

Allocates a fresh table: no metatable, empty array, dummy node as hash part. Returns a GC-managed `Table` object.

### luaH_size(Table *t)

Returns the total memory a table occupies (both parts), for GC size accounting.

### luaH_free(lua_State *L, Table *t)

Frees the hash part, array part, and the `Table` object.

### getfreepos(Table *t)

Finds a free node: rewinds `lastfree` when available, otherwise does a linear scan. Returns NULL when no free slot exists.

### insertkey(Table *t, const TValue *key, TValue *value)

Inserts `key`/`value` into the hash part maintaining Brent's invariant: if the main position is occupied and the collider is out of its main position, the collider is moved to the free slot first. Returns 0 only when no free node exists.

### newcheckedkey(Table *t, const TValue *key, TValue *value)

Inserts into array or hash part when space is guaranteed. Used after a rehash.

### luaH_newkey(lua_State *L, Table *t, const TValue *key, TValue *value)

Main insertion path: ignores nil values, inserts (rehashing when the hash is full), fires a backward GC barrier, and may trigger an emergency collection for stress testing.

### getintfromhash(Table *t, lua_Integer key)

Looks up an integer in the hash part only, following the collision chain.

### hashkeyisempty(Table *t, lua_Unsigned key)

Returns true when an integer key is absent from (or has an empty value in) the hash part.

### finishnodeget(const TValue *val, TValue *res)

Copies a found node value into `res` and returns its tag. `LUA_TNIL` signals absence.

### luaH_getint(Table *t, lua_Integer key, TValue *res)

Gets an integer key, checking the array part first, then the hash part.

### luaH_Hgetshortstr(Table *t, TString *key)

Finds the value slot for a short string using pointer equality (short strings are interned).

### luaH_getshortstr(Table *t, TString *key, TValue *res)

Gets a short-string key, returning its tag or `LUA_TNIL` when absent.

### Hgetlongstr(Table *t, TString *key)

Looks up a long string via `getgeneric` (long strings are not interned).

### Hgetstr(Table *t, TString *key)

Dispatches string lookup by kind: short strings use pointer-identity fast path; long strings use generic search.

### luaH_getstr(Table *t, TString *key, TValue *res)

Gets a string key, returning its tag or `LUA_TNIL` when absent.

### luaH_get(Table *t, const TValue *key, TValue *res)

Main get function: fast paths for short strings and integers, generic search otherwise.

### retpsetcode(Table *t, const TValue *slot)

Encodes where a failed `pset` lives: `HNOTFOUND` or `node_index + HFIRSTNODE`.

### finishnodeset(Table *t, const TValue *slot, TValue *val)

Stores into an existing hash slot (returns `HOK`) or returns an encoding for the caller.

### rawfinishnodeset(const TValue *slot, TValue *val)

Raw store into an existing slot; returns 0 when the key is absent.

### luaH_psetint(Table *t, lua_Integer key, TValue *val)

Pre-set for an integer key known to be outside the array part.

### luaH_psetshortstr(Table *t, TString *key, TValue *val)

Pre-set optimized for constructors: stores in place or inserts directly when there is room and no metamethod/barrier; otherwise returns an encoding.

### luaH_psetstr(Table *t, TString *key, TValue *val)

Pre-set for string keys, dispatching short/long.

### luaH_pset(Table *t, const TValue *key, TValue *val)

Pre-set for any key type with specialized fast paths for short strings and integers.

### luaH_finishset(lua_State *L, Table *t, const TValue *key, TValue *value, int hres)

Completes a set from a `pset` code: normalizes the key (nil/NaN errors, float→integer promotion, external string internalization) then inserts or stores into the encoded slot.

### luaH_set(lua_State *L, Table *t, const TValue *key, TValue *value)

Full set operation: pre-set, then finish when not immediately stored.

### luaH_setint(lua_State *L, Table *t, lua_Integer key, TValue *value)

Full set for integer keys: array fast path, then hash, then new key.

### hash_search(lua_State *L, Table *t, unsigned asize)

From a present index `asize+1`, keeps doubling (with a seeded random twist) to an absent index, then binary-searches the boundary. Returns the largest present integer index. The random element prevents hash-table abuse for `#t` attacks.

### binsearch(Table *array, unsigned int i, unsigned int j)

Binary search inside the array part for a border.

### newhint(Table *t, unsigned hint)

Caches a found border as the `#t` hint and returns it.

### luaH_getn(lua_State *L, Table *t)

Computes `#t`: probes near the cached hint, binary-searches the array part, falls back to the hash part when `t[asize]` is present.

### luaH_mainposition(const Table *t, const TValue *key)

Debug-only export of `mainpositionTV` for the test library.
