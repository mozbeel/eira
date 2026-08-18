# luac.c — Lua bytecode compiler and disassembler

> **AI-Generated Documentation**

## Overview

`luac.c` is the standalone Lua bytecode compiler. It reads source files (or stdin), compiles each into a `Proto` (prototype), optionally combines multiple files into a single chunk, and either lists the bytecode in human-readable form or dumps a binary `.out` file. It is the offline counterpart to the interpreter in `lua.c`.

The tool supports several modes controlled by CLI flags: `-l` lists bytecodes (use `-l -l` for a full listing with constants, locals, and upvalues), `-p` parses without producing output, `-s` strips debug information, and `-o` names the output file (default `luac.out`). When multiple source files are given, their prototypes are nested under a synthetic `(function()end)();` wrapper so they can be serialized as a single binary chunk.

The bytecode listing infrastructure is substantial — `PrintFunction` and its helpers decode every opcode, operand, and constant, producing output similar to `luac -l` in stock Lua but extended for the full Eira instruction set including new opcodes like `OP_ERRNNIL`, `OP_GETVARG`, `OP_BANDK`/`OP_BORK`/`OP_BXORK`, and `OP_SHLI`/`OP_SHRI`.

The binary dump path delegates to `luaU_dump` in `ldump.c`. The `writer` function here is a trivial `fwrite`-based `lua_Writer` callback that streams the serialized bytes to a `FILE*`.

## CLI Interface

### Usage

```
luac [options] [filenames]
```

### Options

- `-l` — List bytecodes. Use `-l -l` for a full listing that also shows constants, locals, and upvalues.
- `-o name` — Output to file `name` (default: `luac.out`). Use `-o -` for stdout.
- `-p` — Parse only; do not produce output.
- `-s` — Strip debug information from the output.
- `-v` — Show version information. If no other arguments are given, exits successfully.
- `--` — Stop handling options; next argument is a filename.
- `-` — Stop handling options and process stdin.

### Global State

The compiler uses several file-scope globals to track mode:

```c
static int listing = 0;
static int dumping = 1;
static int stripping = 0;
static char Output[] = "luac.out";
static const char *output = Output;
```

`listing` counts `-l` flags (0 = no listing, 1 = basic, 2+ = full). `dumping` is 1 unless `-p` disables it. `stripping` is set by `-s`.

## Prototype Combining

When multiple source files are compiled, their prototypes must be bundled into a single chunk for serialization. This is done by `combine`: a synthetic chunk `(function()end)();` is loaded, and its prototype's `p[]` array is patched to point to each compiled file's prototype.

```c
#define FUNCTION "(function()end)();\n"

static const char* reader(lua_State* L, void* ud, size_t* size) {
  UNUSED(L);
  if ((*(int*)ud)--) {
    *size = sizeof(FUNCTION) - 1;
    return FUNCTION;
  } else {
    *size = 0;
    return NULL;
  }
}
```

The `reader` callback returns the synthetic source exactly once. For `n == 1`, `combine` simply returns the single prototype without the wrapper. For `n > 1`, it clears the `instack` flag on each child's upvalue[0] so they don't accidentally capture from the wrapper.

## Disassembly Output

The listing mode produces formatted bytecode with per-instruction details. Each instruction line includes:

1. Program counter (1-based)
2. Source line number in brackets (or `[-]` for non-line instructions)
3. Opcode name (left-padded to 9 chars)
4. Decoded operands (varying per opcode)
5. Inline comments showing resolved constants, upvalue names, or computed jump targets

Constants are printed with type tags (`N`/`B`/`F`/`I`/`S`) and values. Floats that are exact integers get a trailing `.0` for disambiguation. Strings use C-style escapes with `\NNN` for non-printable bytes.

## Functions

### `fatal(const char *message)`

Prints `"luac: message"` to stderr and calls `exit(EXIT_FAILURE)`. Used for unrecoverable errors such as out-of-memory or I/O failures.

### `cannot(const char *what)`

Prints an OS-level error message including `strerror(errno)` for the current output file. Example: `"luac: cannot open luac.out: Permission denied"`. Then exits.

### `usage(const char *message)`

Prints an option error and the full usage summary to stderr, then exits. Distinguishes between unrecognized options (prefixed with `-`) and other errors.

### `doargs(int argc, char *argv[])`

Parses all CLI options, mutating globals `listing`, `dumping`, `stripping`, and `output`. Returns the index of the first non-option argument (the first filename). Special behaviors:

- No filenames with `-l` or `-p`: reads from the default `luac.out` instead of compiling.
- `-v` alone (no other arguments): prints version and exits successfully.
- `--` ends option handling; `-` alone also ends options.

### `reader(lua_State *L, void *ud, size_t *size)`

A `lua_Reader` callback used by `combine`. The `ud` parameter points to a counter. Returns the synthetic source string `"(function()end)();\n"` while the counter is positive, then signals end-of-input with `*size = 0` and `return NULL`.

### `combine(lua_State *L, int n)`

Bundles `n` already-loaded prototypes as nested children of a single wrapper prototype. For `n == 1`, returns the prototype directly. For `n > 1`, loads the synthetic `FUNCTION` string via `lua_load`, patches the wrapper's `p[]` array, and clears child upvalue `instack` flags.

### `writer(lua_State *L, const void *p, size_t size, void *u)`

A `lua_Writer` callback: writes `size` bytes to the `FILE*` given as userdata via `fwrite`. Returns nonzero on write failure, which causes `luaU_dump` to abort.

### `pmain(lua_State *L)`

Protected-mode body. Loads each input file via `luaL_loadfile` (or from stdin if filename is `"-"`), combines prototypes via `combine`, optionally lists them via `luaU_print`, and optionally dumps the bytecode to the output file. Acquires the Lua lock around the dump call.

```c
for (i = 0; i < argc; i++) {
  const char* filename = IS("-") ? NULL : argv[i];
  if (luaL_loadfile(L, filename) != LUA_OK)
    fatal(lua_tostring(L, -1));
}
f = combine(L, argc);
```

### `main(int argc, char *argv[])`

Parses args via `doargs`, adjusts `argc`/`argv` past the options, creates a `lua_State`, calls `pmain` in protected mode, and exits. Fatal on any error.

### `PrintString(const TString *ts)`

Prints a `TString` to stdout in quoted form with C-style escape sequences. Handles `\"`, `\\`, `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, and `\NNN` for non-printable bytes.

### `PrintType(const Proto *f, int i)`

Prints a single-character type tag for constant `i`: `N` (nil), `B` (boolean), `F` (float), `I` (integer), `S` (string). Followed by a tab.

### `PrintConstant(const Proto *f, int i)`

Prints the value of constant `i`. Floats that look like integers (no non-digit characters after the number) get a trailing `.0` to disambiguate from integer constants. Strings are printed via `PrintString`.

### `PrintCode(const Proto *f)`

Disassembles the instruction array. For each instruction, prints the PC, source line, opcode name, and decoded operands. The operand format varies per opcode:

- `OP_LOADK`/`OP_LOADKX`: includes an inline comment with the constant value
- `OP_GETUPVAL`/`OP_SETUPVAL`: shows the upvalue name
- `OP_GETTABUP`/`OP_SETTABUP`: shows the upvalue name and key constant
- `OP_JMP`: shows the absolute target as `to <pc>`
- `OP_CALL`/`OP_TAILCALL`/`OP_RETURN`: shows `N in` / `N out` semantics
- `OP_CLOSURE`: shows the child prototype pointer
- `OP_MMBIN`/`OP_MMBINI`/`OP_MMBINK`: shows the metamethod event name and "flip" flag

### `PrintHeader(const Proto *f)`

Prints the prototype banner: `"main"` or `"function"`, source file (stripping `@`/`=` prefix), line range, instruction count, pointer address, parameter count (with `+` for varargs), stack size, upvalue count, local count, constant count, and nested function count.

### `PrintDebug(const Proto *f)`

Prints three tables: constants (index, type tag, value), locals (index, name, start PC, end PC), and upvalues (index, name, instack flag, index into enclosing scope). Only called with the full listing flag (`-l -l`).

### `PrintFunction(const Proto *f, int full)`

Recursively prints a prototype: calls `PrintHeader`, `PrintCode`, optionally `PrintDebug`, then recurses into all nested prototypes in `f->p[]`. This produces a depth-first traversal of the entire prototype tree.
