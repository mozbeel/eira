# lundump.c — Bytecode deserialization (binary chunk to Proto)

> **AI-Generated Documentation**

## Overview

`lundump.c` is the read side of the bytecode persistence system. It loads a binary chunk produced by `ldump.c`, reconstructing `Proto` objects, closures, and all associated data structures. The public entry point `luaU_undump` is called when `luaL_loadfilex` or `luaL_loadbufferx` detects a binary chunk (starts with the `LUA_SIGNATURE` escape byte).

The loader mirrors `ldump.c` structurally: it reads the header first, validating the magic signature, version, format marker, and native-type size/format probes. If any check fails, a descriptive `"bad binary format"` error is raised with a specific reason (e.g., "version mismatch", "not a binary chunk", "truncated chunk"). After the header, the main prototype is loaded recursively — instructions, constants, upvalues, nested prototypes, source name, and debug info, in exactly the order `ldump.c` writes them.

A notable feature is the "fixed buffer" mode (`S->fixed`): when loading from a contiguous memory block (e.g., an mmap'd file or a string buffer), the loader can point instruction arrays and strings directly into the buffer rather than copying them. This avoids allocation overhead and is signaled by the `PF_FIXED` flag on prototypes. The `getaddr_` function handles zero-copy addressing, returning `NULL` if the requested bytes span a buffer boundary (forcing the caller to fall back to copying).

String deserialization uses the same deduplication scheme as the dumper: a hash table (`S.h`) maps sequential indices to previously-loaded strings. The loader must carefully anchor strings and prototypes on the GC stack before any allocation-triggering operations to prevent the garbage collector from collecting live objects. This is visible in the careful ordering of `setsvalue` calls before `luaH_setint` in `loadString`.

The loader also validates the upvalue count: after loading the main function, it checks that `cl->nupvalues` (recorded in the closure header) matches `cl->p->sizeupvalues` (from the prototype). A mismatch indicates a corrupted chunk.

## Binary Chunk Format

The format is documented in detail in `ldump.md`. In summary:
1. **Header**: signature, version, format, data marker, native-type probes
2. **Main prototype** (recursive):
   - Line range, parameter count, flags, max stack
   - Code (aligned instruction array)
   - Constants (type byte + value per constant)
   - Upvalues (3 bytes each: instack, idx, kind)
   - Nested prototypes (recursive)
   - Source name string
   - Debug info (line info, absolute line info, locals, upvalue names)

Strings use dedup encoding: size=0 + index means "reuse"; size>0 means "new string with real size = size-1". Signed integers use zig-zag + varint encoding.

## Data Structures

### `LoadState`

```c
typedef struct {
  lua_State *L;
  ZIO *Z;
  const char *name;
  Table *h;
  size_t offset;
  lua_Unsigned nstr;
  lu_byte fixed;
} LoadState;
```

- `L`: the Lua state (for allocation, GC barriers, and error handling)
- `Z`: the input stream (`ZIO` buffered reader)
- `name`: source name for error messages (e.g., `"@main.lua"` or `"binary string"`)
- `h`: hash table mapping dedup indices → `TString*` for string reuse
- `offset`: current byte position in the stream (for alignment calculations)
- `nstr`: count of strings loaded so far (next dedup index = nstr+1)
- `fixed`: nonzero if the input is a contiguous fixed buffer (enables zero-copy)

## Functions

### `error(LoadState *S, const char *why)`

Raises a `LUA_ERRSYNTAX` error formatted as `"name: bad binary format (why)"` via `luaD_throw`. This is a `l_noret` function — it never returns. The `why` parameter provides the specific failure reason (e.g., `"truncated chunk"`, `"version mismatch"`, `"integer overflow"`).

### `loadBlock(LoadState *S, void *b, size_t size)`

Reads exactly `size` bytes from the `ZIO` stream into buffer `b`. Raises `"truncated chunk"` on premature EOF (when `luaZ_read` returns nonzero). Advances `S->offset` by `size`.

### `loadAlign(LoadState *S, unsigned align)`

Skips padding bytes so that `S->offset` becomes a multiple of `align`. Reads the padding into a throwaway `lua_Integer` variable to advance the stream correctly. This mirrors `dumpAlign` in `ldump.c`.

### `getaddr_(LoadState *S, size_t size)`

For fixed-buffer mode: returns a pointer to the next `size` contiguous bytes in the input buffer without copying. Returns NULL if the bytes are not contiguous (the `luaZ_getaddr` call returns NULL). Advances `S->offset` by `size`.

```c
static const void *getaddr_(LoadState *S, size_t size) {
  const void *block = luaZ_getaddr(S->Z, size);
  S->offset += size;
  if (block == NULL)
    error(S, "truncated fixed buffer");
  return block;
}
```

### `loadByte(LoadState *S)`

Reads one byte via `zgetc`. Raises `"truncated chunk"` on `EOZ` (-1). Advances `S->offset`.

### `loadVarint(LoadState *S, lua_Unsigned limit)`

Reads an MSB varint (7 payload bits per byte, high bit = continuation) and checks against `limit` to prevent overflow. The `limit` parameter is shifted right by 7 before the comparison loop, matching the varint encoding's scale.

```c
static lua_Unsigned loadVarint(LoadState *S, lua_Unsigned limit) {
  lua_Unsigned x = 0;
  int b;
  limit >>= 7;
  do {
    b = loadByte(S);
    if (x > limit)
      error(S, "integer overflow");
    x = (x << 7) | (b & 0x7f);
  } while ((b & 0x80) != 0);
  return x;
}
```

The overflow check (`x > limit`) is performed before each shift-and-OR, preventing values that would exceed the target type's range.

### `loadSize(LoadState *S)`

Reads a `size_t` varint, capped by `MAX_SIZE`.

### `loadInt(LoadState *S)`

Reads an `int` varint, capped by `INT_MAX`.

### `loadNumber(LoadState *S)`

Reads a `lua_Number` in raw native memory format via `loadVar`.

### `loadInteger(LoadState *S)`

Reads a zig-zag encoded varint and decodes it:
- Even values → `cx >> 1` (non-negative)
- Odd values → `~(cx >> 1)` cast to signed (negative)

This reverses the encoding done by `dumpInteger`: 0→0, 1→-1, 2→1, 3→-2, 4→2, ...

### `loadString(LoadState *S, Proto *p, TString **sl)`

Loads a nullable string into the slot `*sl`. Three code paths:

1. **size == 0** (reuse): reads the dedup index. Index 0 means NULL (asserts `*sl == NULL`). Otherwise looks up the string in `S.h` via `luaH_getint` and sets `*sl`.

2. **Short string** (`size-1 <= LUAI_MAXSHORTLEN`): reads into a stack buffer, creates a short string via `luaS_newlstr`.

3. **Fixed buffer**: points directly into the buffer via `luaS_newextlstr` (zero-copy).

4. **Long string (copy mode)**: creates a long string object via `luaS_createlngstrobj` and reads directly into it.

After loading, the string is recorded in `S.h` for future dedup. GC barriers are emitted after every string creation to keep the collector consistent — this is critical because `loadVector` and `luaH_setint` can both trigger GC.

### `loadCode(LoadState *S, Proto *f)`

Reads the instruction count, aligns to `sizeof(Instruction)`, then either points `f->code` into the fixed buffer (zero-copy) or allocates and copies the instruction array.

### `loadConstants(LoadState *S, Proto *f)`

Reads the constant count, allocates the `k[]` array, pre-fills with nil (for GC safety), then loads each constant by type byte. String constants temporarily use `f->source` as an anchor slot during loading, then move to their final position — this prevents GC from collecting the string while it's only partially constructed.

### `loadProtos(LoadState *S, Proto *f)`

Reads the nested prototype count, allocates `f->p[]` pre-filled with NULL (for GC safety), then creates each child `Proto` via `luaF_newproto` and fills it via `loadFunction`. Each new proto gets a GC barrier after creation.

### `loadUpvalues(LoadState *S, Proto *f)`

Reads the upvalue count, allocates the array with `name` fields pre-filled to NULL (for GC consistency during error paths — an error message allocation can trigger emergency GC, and all prototypes must be consistent at that point), then reads each descriptor's three bytes.

### `loadDebug(LoadState *S, Proto *f)`

Loads four sections:
1. Line info: count + byte array (direct copy via `loadVector`)
2. Absolute line info: count + aligned struct array
3. Local variables: count + per-local (name string + start/end PC ints)
4. Upvalue names: if count is nonzero (debug info present), loads `f->sizeupvalues` name strings

Zero counts indicate stripped debug info — the arrays are not allocated.

### `loadFunction(LoadState *S, Proto *f)`

Fills one `Proto`: reads `linedefined`, `lastlinedefined`, `numparams`, `flag` (with `PF_FIXED` handling), and `maxstacksize`, then loads code, constants, upvalues, prototypes, source name, and debug info. The flag byte is masked with `~PF_FIXED` before loading, then `PF_FIXED` is re-applied if `S->fixed` is set.

### `checkliteral(LoadState *S, const char *s, const char *msg)`

Reads `strlen(s)` bytes and compares against `s` via `memcmp`. Raises `msg` on mismatch. Used to validate the signature (minus the first byte already consumed) and the `LUAC_DATA` marker.

### `checknumsize(LoadState *S, int size, const char *tname)`

Reads a byte and verifies it equals `sizeof(tvar)` for the given native type. Raises `"size mismatch"` on failure.

### `checknumformat(LoadState *S, int eq, const char *tname)`

Verifies that the loaded sample value matches the expected value (an endianness/format probe). Raises `"format mismatch"` on failure.

### `checkHeader(LoadState *S)`

Validates the entire chunk header in sequence: remaining signature bytes, version, format, data marker, and native-type probes for `int`, `Instruction`, `lua_Integer`, and `lua_Number`. Each failure produces a specific error message.

### `luaU_undump(lua_State *L, ZIO *Z, Table *anchor, const char *name, int fixed)`

Public entry point. Strips leading `@`/`=`/signature-byte from `name` for error messages. Initializes a `LoadState`, calls `checkHeader`, creates an `LClosure` with the recorded upvalue count (loaded as a byte from the stream), anchors it, creates and fills the main `Proto` via `loadFunction`, verifies that `cl->nupvalues == cl->p->sizeupvalues`, and optionally calls `luai_verifycode` for runtime verification. Returns the loaded closure.
