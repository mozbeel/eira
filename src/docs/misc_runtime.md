# misc_runtime.md — lzio.c, lctype.c, linit.c, ljumptab.h

> **AI-Generated Documentation**

This document covers four small source files that provide runtime support infrastructure: the buffered I/O abstraction (`lzio.c`), character classification tables (`lctype.c`), standard library initialization (`linit.c`), and the computed-goto VM dispatch table (`ljumptab.h`).

Each file serves a distinct purpose in the runtime pipeline. `lzio.c` sits between all input sources (strings, files, readers) and the parser/loader, providing a uniform byte-stream interface. `lctype.c` replaces the platform `<ctype.h>` with a portable lookup table used by the lexer. `linit.c` is the single point where standard libraries are registered. `ljumptab.h` is the performance-critical dispatch mechanism for the VM's instruction loop.

---

## lzio.c — Buffered stream abstraction

### Overview

`lzio.c` implements the `ZIO` buffered stream type used throughout the Lua parser and bytecode loader. A `ZIO` wraps a `lua_Reader` callback — a function that produces chunks of bytes on demand — and provides a uniform interface for reading bytes and blocks from any source: in-memory strings, files, or nested readers (e.g., a `require` that loads from multiple sources).

The design is minimal: a pointer `p` into the current buffer, a remaining-byte count `n`, and a refill function that calls the reader when the buffer is exhausted. The structure is defined in `lzio.h`:

```c
typedef struct ZIO {
  lua_State *L;
  const char *reader;
  void *data;
  size_t n;
  const char *p;
} ZIO;
```

The `luaZ_read` function copies arbitrary byte counts across buffer boundaries, refilling as needed, and returns the number of bytes still missing at end-of-stream. This is the primary read interface used by `lundump.c` and the lexer.

A zero-copy path (`luaZ_getaddr`) is used by `lundump.c` in fixed-buffer mode: it returns a direct pointer into the current buffer if the requested number of bytes are contiguous, avoiding a `memcpy`. If the bytes span a buffer boundary, it returns `NULL` and the caller falls back to copying. This is valuable for loading precompiled chunks from mmap'd files or persistent string buffers.

The `ZIO` type decouples the parser and loader from the input source. The same lexer code works whether the source is a string (via `luaZ_init` with a string reader), a file (via `luaL_loadfile`), or a nested reader (for `load` with custom sources).

### Functions

#### `luaZ_fill(ZIO *z)`

Refills the buffer by calling `z->reader`. Returns the first byte of the new buffer, or `EOZ` (-1) if the reader signals end-of-input. On refill, `z->n` is set to `size - 1` and `z->p` points to the second byte (the first is returned directly as the function's return value).

The `lua_unlock`/`lua_lock` pair around the reader call is necessary because the reader may invoke Lua C functions that need to acquire the state lock (e.g., for memory allocation).

#### `luaZ_init(lua_State *L, ZIO *z, lua_Reader reader, void *data)`

Initializes a `ZIO` with its reader function and opaque data pointer. The buffer starts empty (`n = 0`, `p = NULL`), meaning the first read will trigger a refill.

#### `checkbuffer(ZIO *z)`

Internal helper that ensures at least one byte is buffered. If `z->n == 0`, calls `luaZ_fill`. On successful refill, adjusts `n` and `p` back by one so the caller sees the byte that `luaZ_fill` already consumed (since `luaZ_fill` returns one byte and advances the pointer past it).

```c
static int checkbuffer(ZIO *z) {
  if (z->n == 0) {
    if (luaZ_fill(z) == EOZ)
      return 0;
    else {
      z->n++;
      z->p--;
    }
  }
  return 1;
}
```

This "peek one byte" pattern ensures that `luaZ_read` and `luaZ_getaddr` always have at least one byte available to inspect before committing to a read operation.

#### `luaZ_read(ZIO *z, void *b, size_t n)`

Reads exactly `n` bytes into `b`, crossing buffer refills as needed. Returns 0 on success or the number of missing bytes if the stream ends prematurely. The inner loop copies the minimum of requested bytes and available bytes per iteration, advancing both the source pointer and the destination pointer.

This is the primary read interface used by `lundump.c`'s `loadBlock` — the loader simply calls `luaZ_read` and checks the return value for truncation.

#### `luaZ_getaddr(ZIO *z, size_t n)`

Returns a pointer to the next `n` buffered bytes without copying. Returns `NULL` if `n` bytes are not contiguous in the current buffer or if the stream is exhausted. Advances `p` and decrements `n` on success.

Used exclusively by `lundump.c` in fixed-buffer mode for zero-copy loading of instruction arrays and long strings. When the input is a contiguous memory block (e.g., an mmap'd file), this avoids both allocation and copying for large data structures.

---

## lctype.c — Character classification lookup table

### Overview

`lctype.c` provides the built-in character classification table `luai_ctype_[]` used by the Lua lexer and string library. When the platform's `<ctype.h>` is not trusted (the default when `LUA_USE_CTYPE` is 0), Lua uses its own lookup table instead, ensuring consistent, locale-independent behavior across platforms.

This is important because Lua's lexical rules (identifier characters, numeric literals, whitespace) must be the same on every platform. System `ctype` functions are locale-dependent — in some locales, `isalpha` might treat bytes 0x80-0xFF as alphabetic, which would break Lua's ASCII-based grammar.

The table has `UCHAR_MAX + 2` entries: one for `EOZ` (-1) at index 0, and one for each byte value 0x00–0xFF. Each entry is a bitmask with bits defined in `lctype.h`:

| Bit | Value | Meaning | Macro |
|-----|-------|---------|-------|
| 0   | 0x01  | Alphabetic | `lislalpha(c)` |
| 1   | 0x02  | Digit | `lisdigit(c)` |
| 2   | 0x04  | Printable | `lisprint(c)` |
| 3   | 0x08  | Space | `lisspace(c)` |
| 4   | 0x10  | Hex digit | `lishex(c)` |

Compound classifications: `lislalnum(c)` = alpha | digit = bits 0-1; `lislalpha(c)` = bit 0 only; `lisdigit(c)` = bit 1 only.

The underscore `_` has bit 0 (alpha) set, making it alphabetic and thus alphanumeric — this is essential for Lua identifier rules, where `_` is a valid identifier character.

When `LUA_UCID` is defined, bytes 0x80–0xFF are classified as alphabetic + printable (`NONA = 0x01 | 0x04`), enabling Unicode identifiers. Without it, high bytes have 0x00 (not classified), restricting identifiers to ASCII.

The `luai_ctype_` array is declared in `lctype.h` with `LUAI_DDEF` (typically empty or `const`) and defined here, making it a global visible across translation units. It is indexed as `luai_ctype_[c + 1]` (the +1 accounts for the EOZ entry at index 0, so `luai_ctype_['A' + 1]` gives the flags for 'A').

### Data Structures

#### `luai_ctype_[UCHAR_MAX + 2]`

```c
LUAI_DDEF const lu_byte luai_ctype_[UCHAR_MAX + 2] = {
  0x00,  /* EOZ */
  0x00,  0x00,  0x00,  0x00,  0x00,  0x00,  0x00,  0x00,  /* 0x */
  0x00,  0x08,  0x08,  0x08,  0x08,  0x08,  0x00,  0x00,  /* 1x */
  /* ... */
};
```

The table encodes:
- Bytes 0x00–0x1F: mostly 0 (non-printable), except tab/newline/etc. have space bit (0x08)
- Bytes 0x20–0x2F: printable (0x04), `!` gets alpha too via the `0x0c` entry
- Bytes 0x30–0x39: digit + printable + hex = `0x16`
- Bytes 0x41–0x5A: alpha + printable + hex = `0x15`
- Bytes 0x61–0x7A: alpha + printable + hex = `0x15`
- Bytes 0x80–0xFF: `NONA` (0x01|0x04 if `LUA_UCID`, else 0x00)

---

## linit.c — Standard library registration

### Overview

`linit.c` is responsible for opening (and optionally preloading) the standard Lua libraries. It defines the `stdlibs` array — a table mapping library names to their open functions — and provides `luaL_openselectedlibs` to selectively open libraries based on bitmasks.

The order of entries in `stdlibs` must match the `LUA_<libname>K` bit constants defined in `lualib.h`. Each library occupies one bit position (1, 2, 4, 8, ...), so any subset can be selected with a single bitmask. This design allows the standalone interpreter (which opens all libraries) and embedded applications (which may want only base + string + math) to use the same function.

Libraries in the `load` mask are opened immediately via `luaL_requiref`, which both initializes the library and sets it as a global. Libraries in the `preload` mask are registered in `package.preload` instead — their open functions are stored but not called until the user explicitly `require`s them.

The current library set:

| Order | Constant | Open Function |
|-------|----------|---------------|
| 0 | `LUA_GNAME` | `luaopen_base` |
| 1 | `LUA_LOADLIBNAME` | `luaopen_package` |
| 2 | `LUA_COLIBNAME` | `luaopen_coroutine` |
| 3 | `LUA_DBLIBNAME` | `luaopen_debug` |
| 4 | `LUA_IOLIBNAME` | `luaopen_io` |
| 5 | `LUA_MATHLIBNAME` | `luaopen_math` |
| 6 | `LUA_OSLIBNAME` | `luaopen_os` |
| 7 | `LUA_STRLIBNAME` | `luaopen_string` |
| 8 | `LUA_TABLIBNAME` | `luaopen_table` |
| 9 | `LUA_UTF8LIBNAME` | `luaopen_utf8` |

The standalone interpreter (`lua.c`) defaults to opening all libraries:

```c
#define luai_openlibs(L)  luaL_openselectedlibs(L, ~0, 0)
```

With `LUA_NODEBUGLIB` defined, the debug library is excluded (must be explicitly required):

```c
#define luai_openlibs(L)  luaL_openselectedlibs(L, ~LUA_DBLIBK, LUA_DBLIBK)
```

### Functions

#### `luaL_openselectedlibs(lua_State *L, int load, int preload)`

Iterates the `stdlibs` table with a bitmask that shifts left by 1 for each entry. For each library:

- If its bit is set in `load`: calls `luaL_requiref(L, lib->name, lib->func, 1)` to open it and set it as a global, then pops the result.
- If its bit is set in `preload`: pushes the open function into `package.preload[lib->name]` for lazy loading.

The function asserts `(mask >> 1) == LUA_UTF8LIBK` after the loop to verify that the `stdlibs` array and the `LUA_*K` bitmask constants remain in sync. The final `lua_pop(L, 1)` removes the `package.preload` subtable.

---

## ljumptab.h — Computed-goto VM dispatch table

### Overview

`ljumptab.h` defines the computed-goto dispatch table used by the Lua VM (`lvm.c`) when compiled with a compiler that supports the GNU C `&&label` extension (GCC, Clang, and compatible compilers). This is the performance-critical dispatch mechanism: instead of a `switch` statement with a central dispatch point, the VM jumps directly to the C label for each opcode handler, enabling better branch prediction and eliminating the indirect-jump overhead of a switch.

On platforms without computed-goto support (MSVC, some embedded compilers), `lvm.c` falls back to a `switch`-based dispatch instead. The `ljumptab.h` header is only included when `LUA_USE_JUMPTABLE` is defined.

The file is `#include`d by `lvm.c` and defines three macros that together drive the instruction dispatch loop:

- `vmdispatch(x)` — `goto *disptab[x]` — jumps to the handler for opcode `x`.
- `vmcase(l)` — `L_##l:` — labels each handler with the name `L_<OPNAME>`.
- `vmbreak` — `vmfetch(); vmdispatch(GET_OPCODE(i));` — fetches the next instruction and dispatches.

Every opcode handler in `lvm.c` ends with `vmbreak`, which fetches the next instruction and jumps to its handler. This makes the dispatch a single indirect goto rather than a compare-and-branch chain.

The `disptab` array has `NUM_OPCODES` entries, one per opcode in the `OP_*` enum order from `lopcodes.h`. The entries are address-of-label expressions (`&&L_OP_MOVE`, `&&L_OP_LOADI`, etc.). A `sed` command (shown in a comment) regenerates the list from `lopcodes.h` to keep it in sync:

```bash
sed -n '/^OP_/!d; s/OP_/\&\&L_OP_/ ; s/,.*/,/ ; s/\/.*// ; p' lopcodes.h
```

This produces lines like `&&L_OP_MOVE,` from enum entries like `OP_MOVE,`.

### Data Structures

#### `disptab[NUM_OPCODES]`

```c
static const void *const disptab[NUM_OPCODES] = {
  &&L_OP_MOVE,
  &&L_OP_LOADI,
  &&L_OP_LOADF,
  &&L_OP_LOADK,
  &&L_OP_LOADKX,
  &&L_OP_LOADFALSE,
  /* ... one entry per opcode ... */
  &&L_OP_VARARGPREP,
  &&L_OP_EXTRAARG
};
```

A `const` array of `const` pointers — one computed-goto address per opcode. The array is `static` because it is included directly into `lvm.c` as a header (not compiled separately). Its order must exactly match the `OP_*` enum in `lopcodes.h`, or dispatch will execute the wrong handler — a catastrophic and hard-to-debug failure.

The full opcode list covers: moves, loads (nil/bool/integer/float/constant), table access (get/set by key/index/field/upvalue), arithmetic (add/sub/mul/div/mod/pow/idiv with constant/immediate/register variants), bitwise (band/bor/bxor/shl/shr with constant/immediate variants), comparisons (eq/lt/le with constant/immediate variants), metamethod calls (MMBIN/MMBINI/MMBINK), unary operations (unm/bnot/not/len), concatenation, jumps, calls/returns, loops (for/tfor), closures, varargs, and error handling (ERRNNIL).

### Macros

#### `vmdispatch(x)`

Evaluates to `goto *disptab[x]`. Used at the top of the main dispatch loop and at the end of every handler (via `vmbreak`). This is the computed-goto equivalent of `switch(GET_OPCODE(i))`.

#### `vmcase(l)`

Expands to `L_##l:`, creating a label for opcode handler `l`. In `lvm.c`, handlers are written as:

```c
vmcase(OP_MOVE) {
  StkId ra = RA(i);
  setobjs2s(L, ra, vRB(i));
  vmbreak;
}
```

The label name `L_OP_MOVE` matches the entry `&&L_OP_MOVE` in `disptab`.

#### `vmbreak`

Expands to `vmfetch(); vmdispatch(GET_OPCODE(i));`. `vmfetch` (defined in `lvm.c`) reads the next instruction from the program counter and advances it. Together with `vmdispatch`, this forms the tail of every opcode handler — a single indirect jump to the next handler. This is the hot path of the entire interpreter.
