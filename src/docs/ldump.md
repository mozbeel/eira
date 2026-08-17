# ldump.c — Bytecode serialization (Proto to binary chunk)

> **AI-Generated Documentation**

## Overview

`ldump.c` serializes a compiled Lua prototype (`Proto`) into the binary chunk format that `lundump.c` can later load. It is the write side of the bytecode persistence system. The public entry point `luaU_dump` is called by `luac.c` and by `luaL_loadfilex` (for cached precompiled chunks).

The serialization format is a linear byte stream. It begins with a header containing a magic signature (the Lua bytecode escape sequence), a version byte, a format byte, a `LUAC_DATA` marker, and native-type size/value probes that let the loader verify ABI compatibility. After the header, the main prototype is serialized recursively: each `Proto` records its line info, parameter count, flags, max stack, instruction array (alignment-padded), constants, upvalue descriptors, nested prototypes, source name, and debug information.

String deduplication is a key optimization: the dumper maintains a hash table (`D.h`) mapping previously-dumped strings to sequential indices. On subsequent occurrences, only a 0-byte + index reference is written instead of re-serializing the full string content. NULL strings are encoded as a special index-0 reference. This can dramatically reduce the size of chunks that reference the same string many times (e.g., repeated field names).

Integers use zig-zag encoding before varint serialization so that small values (including -1) stay compact. The MSB varint format uses 7 payload bits per byte with the high bit indicating continuation, encoding unsigned values most-significant-byte first.

The `strip` flag controls whether debug information (line info, local variable names, upvalue names) is included in the output. Stripped chunks are smaller and cannot be used for error tracebacks or debug introspection, but they execute identically to unstripped chunks.

## Binary Chunk Format

### Header

| Field | Encoding | Description |
|-------|----------|-------------|
| Signature | Raw bytes | `LUA_SIGNATURE` magic escape sequence |
| Version | 1 byte | `LUAC_VERSION` — must match loader's version |
| Format | 1 byte | `LUAC_FORMAT` — reserved, must be 0 |
| Data marker | Raw bytes | `LUAC_DATA` — corruption detection sentinel |
| int size | 1 byte + sample | Size of native `int` and a sample value |
| Instruction size | 1 byte + sample | Size of `Instruction` and a sample value |
| lua_Integer size | 1 byte + sample | Size of `lua_Integer` and a sample value |
| lua_Number size | 1 byte + sample | Size of `lua_Number` and a sample value |

### Per-Prototype Layout

Each `Proto` is serialized in this order:
1. Header fields: `linedefined`, `lastlinedefined`, `numparams`, `flag`, `maxstacksize`
2. Code: instruction count + aligned instruction array
3. Constants: count + per-constant (type byte + value)
4. Upvalues: count + per-upvalue (instack, idx, kind — 3 bytes each)
5. Nested prototypes: count + recursive dump
6. Source name: string (or NULL if stripped)
7. Debug info: line info, absolute line info, locals, upvalue names (all zero counts if stripped)

### String Encoding

Strings use a deduplication scheme:
- **size 0, index 0**: NULL string
- **size 0, index N**: reuse string with dedup index N
- **size > 0**: new string, real size = size-1; the `+1` avoids a `size_t` underflow when the trailing `'\0'` is included

### Integer Encoding

Signed integers use zig-zag encoding:
- `x >= 0` → `2x` (0→0, 1→2, 2→4, ...)
- `x < 0` → `-2x - 1` (-1→1, -2→3, -3→5, ...)

This is then encoded as an MSB varint. Small negative values like -1 become 1 (a single byte).

## Data Structures

### `DumpState`

```c
typedef struct {
  lua_State *L;
  lua_Writer writer;
  void *data;
  size_t offset;
  int strip;
  int status;
  Table *h;
  lua_Unsigned nstr;
} DumpState;
```

- `L`: the Lua state (for GC interaction and error reporting)
- `writer`: the output callback receiving serialized bytes
- `data`: opaque userdata passed through to `writer` (typically a `FILE*`)
- `offset`: current byte position in the dump (for alignment calculations)
- `strip`: if nonzero, debug information is omitted
- `status`: error code from the writer (nonzero aborts all further writes)
- `h`: hash table mapping `TString*` → dedup index for string reuse
- `nstr`: counter for the next dedup index

## Functions

### `dumpBlock(DumpState *D, const void *b, size_t size)`

The fundamental write primitive. Writes `size` bytes through `D->writer`, advancing `D->offset`. After the first error (nonzero `D->status`), all subsequent writes are silently skipped — this avoids cascading errors and lets the dumper continue calculating offsets. The final call with `b == NULL` and `size == 0` signals end-of-dump to the writer.

```c
static void dumpBlock(DumpState *D, const void *b, size_t size) {
  if (D->status == 0) {
    lua_unlock(D->L);
    D->status = (*D->writer)(D->L, b, size, D->data);
    lua_lock(D->L);
    D->offset += size;
  }
}
```

Note the `lua_unlock`/`lua_lock` around the writer call — the writer may call back into Lua (e.g., for memory allocation), so the lock must be released.

### `dumpAlign(DumpState *D, unsigned align)`

Pads with zero bytes so that `D->offset` becomes a multiple of `align`. Used before instruction arrays and struct arrays so the loader can potentially use zero-copy reads on aligned platforms. Asserts `align <= sizeof(lua_Integer)` because the padding content is a zero `lua_Integer`.

### `dumpByte(DumpState *D, int y)`

Dumps a single byte (cast from `int` to `lu_byte`). Used for type tags, flags, and small counts. Implemented via `dumpVar` → `dumpVector`.

### `dumpVarint(DumpState *D, lua_Unsigned x)`

Encodes `x` as an MSB varint: 7 payload bits per byte, high bit = "more bytes follow", most significant byte first. The buffer is pre-sized to `DIBS` = `ceil(l_numbits(lua_Unsigned) / 7)`.

```c
static void dumpVarint(DumpState *D, lua_Unsigned x) {
  lu_byte buff[DIBS];
  unsigned n = 1;
  buff[DIBS - 1] = x & 0x7f;
  while ((x >>= 7) != 0)
    buff[DIBS - (++n)] = cast_byte((x & 0x7f) | 0x80);
  dumpVector(D, buff + DIBS - n, n);
}
```

### `dumpSize(DumpState *D, size_t sz)`

Dumps a `size_t` value as a varint. Widens to `lua_Unsigned` first.

### `dumpInt(DumpState *D, int x)`

Dumps a non-negative `int` as a varint. Asserts `x >= 0`.

### `dumpNumber(DumpState *D, lua_Number x)`

Dumps a float in raw native memory format via `dumpVar`. The header records the size of `lua_Number` so the loader can verify compatibility.

### `dumpInteger(DumpState *D, lua_Integer x)`

Applies zig-zag encoding then dumps as a varint:

```c
lua_Unsigned cx = (x >= 0) ? 2u * l_castS2U(x)
                           : (2u * ~l_castS2U(x)) + 1;
dumpVarint(D, cx);
```

This keeps -1 encoded as 1 (a single byte after varint encoding), which is important because -1 is a very common value (e.g., the `sizecode` sentinel for empty functions).

### `dumpString(DumpState *D, TString *ts)`

Serializes a nullable string with deduplication:
- **NULL**: writes varint `0, 0` (size=0 signals "reuse", index=0 is the NULL sentinel).
- **Previously seen**: writes varint `0, <index>` — the `luaH_getstr` lookup in `D.h` finds the index.
- **New**: writes varint `<size+1>` followed by the string bytes (including trailing `'\0'`), then records it in `D.h` with `D->nstr` as the index.

The `size+1` encoding ensures that a real size of 0 (empty string) encodes as varint 1, avoiding ambiguity with the "reuse" sentinel of 0.

### `dumpCode(DumpState *D, const Proto *f)`

Dumps the instruction count as an int, aligns to `sizeof(Instruction)`, then dumps the raw instruction array via `dumpVector`.

### `dumpConstants(DumpState *D, const Proto *f)`

Dumps the constant count, then for each constant: a type byte (`LUA_VNIL`, `LUA_VFALSE`, `LUA_VTRUE`, `LUA_VNUMFLT`, `LUA_VNUMINT`, `LUA_VSHRSTR`, `LUA_VLNGSTR`) followed by the value. Nil, false, and true carry no payload beyond the type byte.

### `dumpProtos(DumpState *D, const Proto *f)`

Dumps the nested prototype count, then recursively dumps each child via `dumpFunction`.

### `dumpUpvalues(DumpState *D, const Proto *f)`

Dumps the upvalue count and, for each upvalue, three bytes: `instack` (is this upvalue in the local stack frame?), `idx` (which local slot or outer upvalue), and `kind` (read/write/close information). Upvalue names are omitted here — they appear in the debug section.

### `dumpDebug(DumpState *D, const Proto *f)`

Dumps debug information in four sections:
1. Line info: count + byte array (one relative line delta per instruction)
2. Absolute line info: count + aligned struct array (PC → absolute line mappings)
3. Local variables: count + per-local (name string + start PC + end PC)
4. Upvalue names: count + per-upvalue name string

When `D->strip` is true, all counts are written as 0 and no data follows, producing a compact chunk without any debug information.

### `dumpFunction(DumpState *D, const Proto *f)`

Serializes one complete `Proto`:
1. `linedefined` and `lastlinedefined` as ints
2. `numparams`, `flag`, and `maxstacksize` as bytes
3. Code, constants, upvalues, and prototypes (recursive)
4. Source name string
5. Debug information

### `dumpHeader(DumpState *D)`

Writes the chunk header: the `LUA_SIGNATURE` magic bytes, `LUAC_VERSION`, `LUAC_FORMAT`, `LUAC_DATA` marker, and native-type probes. Each numeric type is dumped as a size byte + sample value via the `dumpNumInfo` macro:

```c
#define dumpNumInfo(D, tvar, value) \
  { tvar i = value; dumpByte(D, sizeof(tvar)); dumpVar(D, i); }
```

### `luaU_dump(lua_State *L, const Proto *f, lua_Writer w, void *data, int strip)`

Public entry point. Initializes a `DumpState`, creates a string-dedup hash table (`luaH_new`) and anchors it on the Lua stack (to prevent GC from collecting it during serialization), writes the header, dumps the main function's upvalue count, recursively dumps the main function, signals end-of-dump, and returns the writer status.
