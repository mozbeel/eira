# Eira / Lua 5.5 — Full Architecture

> **AI-Generated Documentation** — this document was produced by an AI assistant and
> should be read alongside the source files it references. Line numbers and type names
> are taken directly from the C source in `src/`.

This document walks through the entire Lua 5.5 runtime, from raw source text to executed
bytecode, covering every major subsystem. It is meant to be read top-to-bottom or
navigated via the table of contents below.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Data Representation](#2-data-representation)
3. [Compilation Pipeline](#3-compilation-pipeline)
   - 3.1 [Lexer (`llex.c`)](#31-lexer)
   - 3.2 [Parser (`lparser.c`)](#32-parser)
   - 3.3 [Code Generator (`lcode.c`)](#33-code-generator)
4. [Bytecode Format](#4-bytecode-format)
   - 4.1 [Instruction Encoding (`lopcodes.h`)](#41-instruction-encoding)
   - 4.2 [Opcode Reference (`lopcodes.c`)](#42-opcode-reference)
   - 4.3 [Constant Table](#43-constant-table)
   - 4.4 [Serialization / Deserialization (`ldump.c`, `lundump.c`)](#44-serialization--deserialization)
5. [The Virtual Machine](#5-the-virtual-machine)
   - 5.1 [Main Dispatch Loop (`lvm.c`)](#51-main-dispatch-loop)
   - 5.2 [Arithmetic and Comparisons](#52-arithmetic-and-comparisons)
   - 5.3 [Table Operations](#53-table-operations)
   - 5.4 [String Concatenation](#54-string-concatenation)
6. [Call Dispatch and Stack Management](#6-call-dispatch-and-stack-management)
   - 6.1 [The CallInfo Chain (`ldo.c`)](#61-the-callinfo-chain)
   - 6.2 [Protected Calls (`lua_pcall`)](#62-protected-calls)
   - 6.3 [Error Handling and Unwinding](#63-error-handling-and-unwinding)
7. [Object Model](#7-object-model)
   - 7.1 [TValue — Tagged Values](#71-tvalue--tagged-values)
   - 7.2 [Strings (`lstring.c`)](#72-strings)
   - 7.3 [Tables (`ltable.c`)](#73-tables)
   - 7.4 [Functions and Closures](#74-functions-and-closures)
   - 7.5 [UpValues (`lfunc.c`)](#75-upvalues)
8. [Garbage Collection](#8-garbage-collection)
   - 8.1 [GC Object Lists (`lstate.h`)](#81-gc-object-lists)
   - 8.2 [Incremental Mode](#82-incremental-mode)
   - 8.3 [Generational Mode](#83-generational-mode)
   - 8.4 [Write Barriers](#84-write-barriers)
9. [Metamethods and OOP](#9-metamethods-and-oop)
   - 9.1 [Tag Methods (`ltm.c`)](#91-tag-methods)
   - 9.2 [Metamethod Dispatch in the VM](#92-metamethod-dispatch-in-the-vm)
   - 9.3 [The `__index` and `__newindex` Protocol](#93-the-__index-and-__newindex-protocol)
10. [The C API](#10-the-c-api)
    - 10.1 [Stack-Based Programming (`lapi.c`)](#101-stack-based-programming)
    - 10.2 [Auxiliary Library (`lauxlib.c`)](#102-auxiliary-library)
11. [Coroutines](#11-coroutines)
    - 11.1 [Yield Mechanism](#111-yield-mechanism)
    - 11.2 [Continuation Functions](#112-continuation-functions)
12. [Debug and Hooks (`ldebug.c`)](#12-debug-and-hooks)
13. [Standard Libraries](#13-standard-libraries)
    - 13.1 [Library Registration (`linit.c`)](#131-library-registration)
    - 13.2 [The Base Library (`lbaselib.c`)](#132-the-base-library)
14. [Buffered I/O (`lzio.c`)](#14-buffered-io)
15. [CLI Front-Ends (`lua.c`, `luac.c`)](#15-cli-front-ends)

---

## 1. Overview

A Lua program flows through four major stages:

```
  Source text
      │
      ▼
  ┌──────────┐
  │   llex   │  Tokenizes raw bytes into a token stream.
  └────┬─────┘
       ▼
  ┌──────────┐
  │  lparser │  Recursive-descent parser; builds a tree of Statements and Expressions.
  └────┬─────┘
       ▼
  ┌──────────┐
  │  lcode   │  Walks the AST and emits register-based bytecode into a Proto.
  └────┬─────┘
       ▼
  ┌──────────────────┐
  │ ldump / lundump  │  Serializes Proto to binary (and loads it back).
  └────┬─────────────┘
       ▼
  ┌──────────┐
  │   lvm    │  Fetches, decodes, and executes bytecode instructions.
  └──────────┘
```

The VM depends on a runtime layer shared across all stages:

| Subsystem | File(s) | Role |
|-----------|---------|------|
| Value representation | `lobject.c/h`, `lua.h` | `TValue` union, type tags, object headers |
| Interpreter state | `lstate.c/h` | `global_State`, `lua_State`, `CallInfo` |
| Garbage collector | `lgc.c/h` | Incremental & generational mark-sweep |
| Tables | `ltable.c/h` | Array + hash partition, rehash, `#t` border |
| Strings | `lstring.c/h` | Interned short strings, long strings, external strings |
| Functions / upvalues | `lfunc.c/h` | Proto, LClosure, CClosure, UpVal |
| Metamethods | `ltm.c/h` | Tag-method name table, dispatch helpers |
| C API | `lapi.c/h` | Public `LUA_API` functions |
| Auxiliary API | `lauxlib.c/h` | Convenience wrappers for embedding |
| Standard libraries | `lbaselib.c`, `lstrlib.c`, etc. | Lua-level built-in functions |

---

## 2. Data Representation

Every Lua value is represented in memory as a **TValue** (Tagged Value), defined in
`lobject.h:67-69`:

```c
typedef struct TValue {
  Value value_;   // union: gc pointer, light ptr, C func, lua_Integer, lua_Number
  lu_byte tt_;    // type tag (bits 0-3: type, bits 4-5: variant, bit 6: collectable)
} TValue;
```

The `tt_` tag packs:
- **Bits 0–3**: base type (`LUA_TNIL` through `LUA_TTHREAD`, values 0–8)
- **Bits 4–5**: variant (e.g. integer vs. float, short vs. long string, Lua vs. C closure)
- **Bit 6**: `BIT_ISCOLLECTABLE` — if set, the `value_` is a pointer to a GC-managed object

This layout lets Lua test types with a single comparison (`ttype(o) == LUA_TNUMBER`)
and avoids per-value heap allocation for primitives like numbers and booleans.

### The GCObject Hierarchy

Collectable objects all begin with a `CommonHeader` (`lobject.h:302`):

```c
#define CommonHeader  struct GCObject *next; lu_byte tt; lu_byte marked
```

Every GC object — strings, tables, closures, prototypes, threads, userdata, upvalues — is
tagged with `tt` and threaded into the GC's linked lists via `next`. The `marked` byte
stores GC color (white/gray/black) and age bits for the generational collector.

The union `GCUnion` (`lstate.h:394-403`) lets the GC treat any object uniformly via its
`CommonHeader` while C code uses the specific `gco2ts()`, `gco2t()`, etc. macros to
downcast.

### StackValue

The Lua stack is an array of `StackValue` (`lobject.h:148-154`), each of which wraps a
`TValue` plus an optional `delta` field used for the to-be-closed variable linked list.
`StkId` is a pointer into this array. The stack grows upward: `stack` points to the
bottom, `top` points to the first free slot.

---

## 3. Compilation Pipeline

### 3.1 Lexer

**File:** `src/llex.c` (~600 lines)

The lexer is **demand-driven**: it does not tokenize the entire source up front. Instead,
`llex()` reads one token at a time whenever the parser calls `luaX_next()`. This is
efficient because only the tokens that are actually consumed get scanned.

Key data structure: **LexState** (defined in `llex.h`), which holds:
- The current token (`t.token`) and lookahead token (`lookahead`)
- The source `MBuffer` (buffered character stream from `lzio.c`)
- Current and nested function states (for tracking scope)
- The token literal value (`seminfo` — string or number)

The core loop in `llex()` uses a giant `switch` on the current character to dispatch:
- Alphabetic → `read_ident()`: checks against Lua keywords, otherwise emits `TK_NAME`
- Digit / decimal point → `read_numeral()`: parses integers and floats (with hex, exponents)
- `"..."` / `'...'` → `read_string()`: handles escape sequences, `\z` whitespace skip
- `[[` / `[=...=[` → `read_long_string()`: multi-line string literals
- `--` → comment scanning (single-line `--` or long `--[[ ]]`)
- Other → punctuation, operators, `::label::` syntax

The lexer maintains a **table of reserved words** indexed by `TSemKey` so keyword lookup
is O(1) via a perfect hash.

### 3.2 Parser

**File:** `src/lparser.c` (~2200 lines)

A **recursive-descent parser** that builds a tree of `Statement` and `Expression` nodes
entirely within the parser's own `FuncState` — there is no separate AST data structure.
Instead, expressions are described in-place via **expdesc** (expression descriptors), which
record whether the expression is a register, a constant, a local variable, a table access,
a function call, etc. This "virtual register" model means the parser can emit code eagerly
as it recognizes constructs.

The top-level entry point is `luaY_parser()`, called from `luaD_protectedparser()` in
`ldo.c`. It creates a root `FuncState`, initializes the first block (`block()`), and
begins recursive descent.

Key parsing functions:
- **`statement()`**: dispatches on `t.token` to `ifstat()`, `whilestat()`, `forstat()`,
  `retstat()`, `exprstat()`, `localstat()`, etc.
- **`block()`**: parses a sequence of statements until an `end` or block-closing token.
- **`subexpr()`**: handles operator precedence via a Pratt-parser–style recursive descent
  (`expr1()` calls `subexpr()` with a precedence argument).
- **`funcbody()`**: parses `function(args) body end`, creating a child `FuncState` for
  the nested function prototype.

The parser uses **expdesc** extensively. An expression descriptor carries:
- `k`: the expression kind (`VVOID`, `VNIL`, `VTRUE`, `VK`, `VINDEXED`, `VRELOCABLE`,
  `VCALL`, `VVAR`, etc.)
- `u`: union holding register index, constant index, or variable info
- `t` / `f`: true/false jump lists for short-circuit evaluation

When the code generator needs an actual value, `luaK_exp2val()` / `luaK_exp2reg()` is
called to force the expression into a register.

### 3.3 Code Generator

**File:** `src/lcode.c` (~2000 lines)

The code generator emits Lua bytecode instructions directly into the `Proto.code[]` array.
It does not build an intermediate representation — it walks the parser's expdesc nodes and
emits instructions on the fly.

Key concepts:

**Register allocation** — Lua uses a flat register file within each function frame. The
code generator maintains `fs->freereg` (next free register). Registers are allocated by
bumping `freereg` and freed when leaving a scope (`luaK_free()`). The `OP_MOVE` instruction
can shuffle registers when needed.

**Constants** — Each `Proto` has a constant array `k[]` (TValue). When the code generator
encounters a literal (number, string, boolean), it stores it in `k[]` via `luaK_stringK()`,
`luaK_intK()`, etc., and references it by index in the instruction.

**RK encoding** — Many instructions support an "RK" operand: if bit `k` is set, the
operand references a constant `K[C]`; otherwise it references a register `R[C]`. This
avoids loading constants into registers for one-off uses.

**Jump management** — The code generator builds chains of pending jumps using the `pc`
field of instructions. `luaK_concat()` links jump lists, and `luaK_patchtohere()` resolves
them. `luaK_patchclose()` handles closing upvalues for forward jumps.

**Function emission** — `luaK_code()` appends an instruction to `fs->f->code[]`.
`luaK_setlist()` handles the `OP_SETLIST` for table constructors (with batching via
`OP_EXTRAARG` when the list is long).

The generated bytecode lives inside a **Proto** (`lobject.h:603-626`):

```c
typedef struct Proto {
  Instruction *code;    // bytecode array
  TValue *k;            // constants (numbers, strings, etc.)
  Proto **p;            // child function prototypes
  Upvaldesc *upvalues;  // upvalue descriptors
  ls_byte *lineinfo;    // compressed line number info
  LocVar *locvars;      // local variable debug info
  TString *source;      // source file name
  int sizecode, sizek, sizep, sizeupvalues, ...;
  lu_byte numparams, maxstacksize;
  ...
} Proto;
```

---

## 4. Bytecode Format

### 4.1 Instruction Encoding

**File:** `src/lopcodes.h` (header, 444 lines)

Every instruction is a 32-bit unsigned integer with the following bit layout:

```
Bits  [31..25]  [24..23]  [22..15]  [14]  [13..6]   [5..0]
       Op(7)    k(1)       A(8)     k-bit   B(8)      C(8)     ← iABC format
       Op(7)    --          A(8)     --     Bx(17)              ← iABx format
       Op(7)    --          A(8)     --     sBx(17) signed      ← iAsBx format
       Op(7)    --          Ax(25)                             ← iAx format
       Op(7)    --          sJ(25) signed                      ← isJ format (jumps)
```

There is also a "variant" `ivABC` format where B and C are swapped to vB(6)/vC(10) for
instructions that need a larger C operand (e.g. `OP_NEWTABLE`, `OP_SETLIST`).

The 7-bit opcode field allows 128 possible opcodes; Lua 5.5 uses about 80 of them.

### 4.2 Opcode Reference

**File:** `src/lopcodes.c` (131 lines)

This file defines three tables:
- **`luaP_opnames[]`**: human-readable names for each opcode (for `luac -l` disassembly)
- **`luaP_opmodes[]`**: packed metadata per opcode — mode (iABC/iABx/etc.), whether it
  sets register A, whether it's a test, whether it triggers a metamethod
- **`luaP_isIT()`**: checks whether an instruction is "inverse test" (used by the
  compiler to fuse `if not X then` patterns)

The full opcode enum is in `lopcodes.h:231-348`. Key groups:

| Category | Opcodes | Description |
|----------|---------|-------------|
| Load | `LOADI`, `LOADK`, `LOADNIL`, `LOADTRUE`, `LOADFALSE` | Push constants into registers |
| Move | `MOVE`, `GETUPVAL`, `SETUPVAL` | Register ↔ register, register ↔ upvalue |
| Table | `GETTABLE`, `SETTABLE`, `GETFIELD`, `SETFIELD`, `GETI`, `SETI`, `NEWTABLE` | Table access and creation |
| Arithmetic | `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `POW`, `IDIV`, `UNM`, `BNOT`, `NOT`, `LEN` | Numeric and bitwise ops |
| Comparison | `EQ`, `LT`, `LE`, `EQK`, `EQI`, `LTI`, `LEI`, `GTI`, `GEI` | Conditional tests |
| Control | `JMP`, `TEST`, `TESTSET` | Branching |
| Call | `CALL`, `TAILCALL` | Function invocation |
| Return | `RETURN`, `RETURN0`, `RETURN1` | Returning from functions |
| Loop | `FORLOOP`, `FORPREP`, `TFORPREP`, `TFORCALL`, `TFORLOOP` | Numeric and generic for-loops |
| Closures | `CLOSURE`, `VARARG`, `VARARGPREP` | Closure creation, varargs |
| Misc | `CONCAT`, `SELF`, `SETLIST`, `CLOSE`, `TBC`, `ERRNNIL` | String concat, method calls, table literals, to-be-close, error |

### 4.3 Constant Table

Each `Proto` carries `TValue *k` — an array of compile-time constants. The code generator
populates this during compilation:
- Numbers are stored as `LUA_VNUMINT` or `LUA_VNUMFLT`
- Strings are interned as `TString` objects (short strings are interned globally)
- Booleans, nil are handled inline (no constant table entry needed)

Instructions reference constants by index: `K[Bx]` for `iABx` format, or via the RK
encoding `K[C]` when the `k` bit is set in `iABC`.

### 4.4 Serialization / Deserialization

**Files:** `src/ldump.c` (~300 lines), `src/lundump.c` (~400 lines)

**`ldump.c`** writes a compiled `Proto` to a binary stream:
- Header: 12-byte signature (`\x1bLua`), version (5.5), format, endianness, sizeof(int),
  sizeof(size_t), sizeof(Instruction), sizeof(lua_Number), float check
- Each `Proto` is written recursively: upvalue descriptors, bytecode, constants, line info,
  local variable info, child protos
- Strings are deduplicated via a hash table so repeated strings are written once

**`lundump.c`** reads the binary back into a live `Proto`:
- Validates the header (version mismatch → `LUA_ERRSYNTAX`)
- Reconstructs `TString` objects by interning strings
- Uses varint + zig-zag encoding for variable-length integers in the header
- Handles "fixed" and "memory" external long strings (`LSTRFIX`, `LSTRMEM`)

The standalone `lua` interpreter can load both source and bytecode; the `luac` compiler
produces `.luac` bytecode files.

---

## 5. The Virtual Machine

### 5.1 Main Dispatch Loop

**File:** `src/lvm.c` (~2000 lines)

The heart of the interpreter is `luaV_execute()`, a massive function (~1900 lines) that
runs a **threaded fetch-decode-execute** loop:

```c
void luaV_execute (lua_State *L) {
  CallInfo *ci = L->ci;
  ...
  startfunc:  // entry point for a new function call
  ...
  dispatch:
    switch (GET_OPCODE(i)) {
      vmcase(OP_MOVE)   { ... goto dispatch; }
      vmcase(OP_LOADI)  { ... goto dispatch; }
      vmcase(OP_ADD)    { ... goto dispatch; }
      vmcase(OP_CALL)   { ... goto startfunc; }  // may tail-call
      vmcase(OP_RETURN) { ... goto ret; }
      ...
    }
}
```

The VM uses **computed goto** (GCC/Clang extension) via `ljumptab.h` — a jump table of
`&&label` addresses — for the fastest possible dispatch on compilers that support it.
Fallback is a standard `switch`.

The VM maintains:
- `pc`: pointer into `ci->u.l.savedpc` (the instruction pointer)
- `base`: base of the current function's register window (computed from `ci->func + 1`)
- `L->top`: marks the high-water mark for stack checks

### 5.2 Arithmetic and Comparisons

Arithmetic opcodes use large macro families (defined in `lvm.c`) to handle all type
combinations:

```c
// Example: OP_ADD
op_add(L, ra, rb, rc) →
  if (ttisinteger(rb) && ttisinteger(rc))
      ivalue(ra) = ivalue(rb) + ivalue(rc);        // fast integer path
  else if (ttisnumber(rb) && ttisnumber(rc))
      fltvalue(ra) = nvalue(rb) + nvalue(rc);       // float path
  else
      luaT_trybinTM(L, rb, rc, ra, TM_ADD);         // metamethod fallback
```

The `op_arithI`, `op_arithK`, `op_bitwise` macro families follow the same pattern but
with one operand as an immediate or constant. If both operands are integers, the fast path
runs; if both are numbers, floats; otherwise metamethods are tried.

Comparisons follow a similar structure: fast integer compare → fast float compare →
metamethod → fall back to `luaT_orderTM()`.

### 5.3 Table Operations

Table access opcodes (`GETTABLE`, `GETFIELD`, `GETI`) go through `luaV_gettable()`,
which:
1. Checks the table's `flags` byte for a cached fast-get (no `__index` metamethod)
2. Tries the array partition (integer key 1..asize)
3. Tries the hash partition (open addressing with `next` chaining)
4. Falls back to `luaT_getTMByObj()` for `__index` metamethods

`OP_NEWTABLE` creates an empty table with a hint for the hash-part size (log2 encoded in
vB). The `k` bit signals that the array size follows in `OP_EXTRAARG`.

### 5.4 String Concatenation

`OP_CONCAT` concatenates B consecutive registers `R[A] .. R[A+1] .. .. R[A+B-1]` by
calling `luaV_concat()`. The implementation:
- Reverses operands so the leftmost string is on top
- Iteratively calls `luaV_strconcat()` which tries fast-path (copy bytes), then
  falls back to GC-aware concatenation for very long strings (using `luaC_checkGC`)
- Numbers are auto-coerced to strings via `luaO_tostring()`

---

## 6. Call Dispatch and Stack Management

### 6.1 The CallInfo Chain

**File:** `src/ldo.c` (~1200 lines)

Each active function call is represented by a **CallInfo** struct (`lstate.h:187-209`),
linked into a doubly-linked list:

```
L->ci → [base_ci] → [ci_1] → [ci_2] → ... → [current]
```

Each CallInfo stores:
- `func`: stack slot of the function being called
- `top`: stack top for this call (for stack overflow checks)
- `previous` / `next`: doubly-linked list pointers
- `callstatus`: bit flags (`CIST_C` for C functions, `CIST_FRESH` for new Lua frames,
  `CIST_TAIL` for tail calls, `CIST_YPCALL` for protected calls that can yield, etc.)
- Union `u`:
  - For Lua functions: `u.l.savedpc` (instruction pointer) and `u.l.trap` (hook flag)
  - For C functions: `u.c.k` (continuation), `u.c.ctx` (continuation context),
    `u.c.old_errfunc`

The `CallInfo` list grows dynamically via `luaE_extendCI()` when the stack of calls
exceeds the pre-allocated `base_ci[]` array. Shrinkage happens via `luaE_shrinkCI()`.

### 6.2 Protected Calls

`lua_pcallk()` (in `lapi.c`, delegates to `luaD_pcall()` in `ldo.c`) sets up a
**protected call**:
1. Pushes a `struct lua_longjmp` onto the thread's error-recovery stack
   (`L->errorJmp` linked list)
2. Calls `luaD_callnoyield()` or `luaD_call()` to actually invoke the function
3. If an error occurs, `luaD_throw()` does a `longjmp` back to the protected call
4. The protected call cleans up (closes to-be-closed variables, restores `errfunc`,
   adjusts `nCcalls`) and returns the error status

`lua_callk()` uses a continuation function (`lua_KFunction`) so that yields inside
protected calls can resume via `luaV_execute()`'s `OP_CALL`/`OP_RETURN` re-entry.

### 6.3 Error Handling and Unwinding

When `luaD_throw()` fires:
1. It sets `L->status` to the error code
2. Finds the nearest `lua_longjmp` via `L->errorJmp`
3. Calls `longjmp()` back to the protected call site
4. The protected call handler:
   - Closes all to-be-closed variables on the affected stack
   - Restores `L->errfunc` and `L->nCcalls`
   - Formats the error message (calling `luaG_errormsg()` if the error handler itself fails)
   - Returns the error code to `lua_pcall()`

The `__close` metamethod (introduced in Lua 5.4, continued in 5.5) runs during unwinding:
the VM marks variables with `OP_TBC`, and on scope exit they are closed by `luaF_close()`.

---

## 7. Object Model

### 7.1 TValue — Tagged Values

Covered in detail in [Section 2](#2-data-representation). Key points:
- All Lua values fit in a `TValue` (16 bytes on 64-bit: 8-byte union + 1-byte tag +
  padding)
- Numbers are unboxed (no heap allocation)
- Booleans and nil are unboxed
- Strings, tables, closures, userdata, threads are heap-allocated GC objects referenced
  via `Value.gc`

### 7.2 Strings

**File:** `src/lstring.c` (~350 lines)

Lua has two string types:
- **Short strings** (`TString` with `shrlen >= 0`): always interned in the global string
  table (`G(L)->strt`). Two short strings with the same content are pointer-equal.
  Hash = FNV-1a over the bytes.
- **Long strings** (`TString` with `shrlen < 0`): not interned. Can be either "regular"
  (`LSTRREG`, owned by GC), "fixed" (`LSTRFIX`, externally managed, no dealloc), or
  "memory" (`LSTRMEM`, externally managed with a dealloc function).

String interning (`luaS_newlstr()`) checks the global hash table first; on miss, it
allocates a new `TString` and inserts it. The hash table is dynamically resized.

There is also a **string cache** (`G(L)->strcache[53][2]`) for recently used strings,
reducing hash lookups in the C API.

### 7.3 Tables

**File:** `src/ltable.c` (~1350 lines)

A Lua table is the only composite data type. It has two partitions:

```
┌─────────────────────────────────────────────────────┐
│ Table                                                  │
│  array[]  ──→  [1] [2] [3] ... [asize]   (integer keys 1..asize)  │
│  node[]   ──→  hash table (open addressing with chaining)         │
│ metatable ──→  optional metatable for metamethods                 │
└─────────────────────────────────────────────────────┘
```

**Array partition** — A flat `Value *array` of size `asize`. If a key is an integer in
range [1, asize], access is O(1) via direct indexing.

**Hash partition** — An open-addressed hash table of `Node` structs. Each node holds a
key-value pair plus a `next` offset for collision chaining. The size is always a power of
2, stored as `lsizenode = log2(size)`. Hash probing uses `lmod(hash, size)`.

**Access path** (`luaH_get()`):
1. If key is an integer in [1, asize] → array access
2. Otherwise → `findindex()` which hashes the key, probes the hash table, returns the
   node's value

**The `#t` operator** (`luaH_getn()` and the "border" algorithm):
- Uses a binary search to find the boundary: the largest integer index `n` such that
  `t[n]` is not nil and `t[n+1]` is nil
- O(log n) time

**Rehash** (`luaH_resize()`): allocates new arrays, redistributes all keys. The split
point (asize vs hash) is chosen by `computesizes()`, which picks the largest power-of-2
asize that stores at least half of the integer keys.

**Dead keys** — When a key is removed from a table during traversal, its node is marked
with `LUA_TDEADKEY` so that `next()` can still find it. The original value is preserved
so the key can be found when iterated.

### 7.4 Functions and Closures

Three function kinds, discriminated by the variant tag in `tt_`:
- **`LUA_VLCL`** (Lua closure): wraps a `Proto` and an array of `UpVal` pointers
- **`LUA_VLCF`** (light C function): a bare `lua_CFunction` pointer, no upvalues
- **`LUA_VCCL`** (C closure): a `CClosure` with up to `nupvalues` TValue upvalues

An `LClosure` (`lobject.h:707-711`):

```c
typedef struct LClosure {
  ClosureHeader;       // CommonHeader + nupvalues + gclist
  struct Proto *p;     // the compiled prototype
  UpVal *upvals[1];    // variable-length array of upvalue pointers
} LClosure;
```

The `Proto` (compiled function body) is shared across all closures created from the same
function expression. Different closures differ only in their upvalue bindings.

### 7.5 UpValues

**File:** `src/lfunc.c` (~300 lines)

An **UpVal** (`lobject.h:680-693`) bridges a closure's reference to a local variable in
an enclosing function:

- **Open upvalue**: `v.p` points directly into the stack slot. Open upvalues are threaded
  through `L->openupval` (sorted by stack address) so that multiple closures sharing the
  same variable point to the same UpVal.
- **Closed upvalue**: When the stack frame is destroyed (`luaF_close()`), the upvalue's
  value is copied into its own `u.value` field, and it is removed from the open list.

`luaF_findupval()` either finds an existing open upvalue at the given stack position or
creates a new one. This ensures that all closures capturing the same local variable share
the same upvalue, so mutations are visible across closures.

---

## 8. Garbage Collection

**File:** `src/lgc.c` (~1800 lines)

### 8.1 GC Object Lists

All GC objects are tracked in linked lists rooted in `global_State`. The lists use the
`next` field of `CommonHeader`:

| List | Purpose |
|------|---------|
| `allgc` | All live non-finalized objects |
| `finobj` | Objects with `__gc` finalizers |
| `tobefnz` | Objects ready to be finalized |
| `fixedgc` | Objects that are never collected (small interned strings, etc.) |
| `gray` | Gray objects awaiting propagation |
| `grayagain` | Objects to revisit in the atomic phase |
| `weak` | Tables with weak values |
| `ephemeron` | Tables with weak keys (ephemeron tables) |
| `allweak` | Tables with all-weak entries |

### 8.2 Incremental Mode

The default GC mode is **incremental mark-and-sweep**. It progresses through these states
(`lgc.h:35-43`):

```
GCSpause → GCSpropagate → GCSenteratomic → GCSatomic
         → GCSswpallgc → GCSswpfinobj → GCSswptobefnz → GCSswpend
         → GCScallfin → GCSpause (next cycle)
```

**`luaC_step()`** — the main entry point, called periodically when `GCdebt` goes
negative. Each step does a fixed amount of work (traversing one gray object or sweeping
one object), controlled by `GCMUL` (step multiplier) and `GCSTEPSIZE`.

**Mark phase** (`propagatemark()`) — visits gray objects, colors them black, and pushes
their references as new gray objects. The main invariant: a black object never points to
a white object.

**Atomic phase** — finishes propagation (all remaining gray objects), processes weak
tables, finalizes objects, and flips the "current white" bit.

**Sweep phase** — walks `allgc`, turning black objects white, freeing dead white objects,
and repairing the invariant for the next cycle.

### 8.3 Generational Mode

Lua 5.5 also supports a **generational collector** that distinguishes objects by age:
`G_NEW` → `G_SURVIVAL` → `G_OLD0` → `G_OLD1` → `G_OLD`.

Minor collections only scan young objects (new and survival). If too many bytes age
(`MINORMAJOR` threshold), the collector switches to major mode and does a full cycle.

The generational mode uses the same barrier mechanisms as the incremental mode but with
age-specific policies: young objects stay white, old objects stay black, and "touched"
objects get special handling to avoid full traversals of the old generation.

### 8.4 Write Barriers

Write barriers maintain the GC invariant. Two flavors:

- **Forward barrier** (`luaC_barrier_`): when a black object P gains a reference to a
  white object O, O is promoted to gray (added to `grayagain`) so it gets traversed.
- **Backward barrier** (`luaC_barrierback_`): when an old object is modified, it is
  demoted to gray and added to `grayagain` to be revisited at the end of the cycle.

The barrier macros are defined in `lgc.h:241-252` and inline the fast path (no barrier
needed if the invariant isn't violated).

---

## 9. Metamethods and OOP

### 9.1 Tag Methods

**File:** `src/ltm.c` (~360 lines)

Metamethods are methods that define operator behavior for user-defined types. They are
stored in the `__index`, `__newindex`, `__add`, `__concat`, `__len`, `__eq`, `__lt`,
`__le`, `__call`, `__close`, `__gc`, `__name`, `__metatable`, `__iter`, `__tostring` etc.
entries of a metatable.

The tag-method names are defined in `TM_N` entries of the `luaT_tname[]` array (pre-interned
strings). `luaT_getTMByObj()` looks up a metatable (via `getmetatable` or per-type
metatable in `G(L)->mt[]`) and retrieves the named method.

Key helpers:
- `luaT_trybinTM()`: attempts a binary metamethod (e.g. `__add`); raises an error if not
  found
- `luaT_callTM()`: calls a metamethod with a protected or unprotected call
- `luaT_callorderTM()`: implements `<` / `<=` via `__lt` / `__le` metamethods

### 9.2 Metamethod Dispatch in the VM

In `lvm.c`, after a fast-path failure (e.g. integer add fails because types are wrong),
the VM calls `luaT_trybinTM()`. If the metamethod exists, it is called and the result is
stored; if not, `luaG_ordererror()` raises a descriptive error.

Metamethod results are always type-checked at the C level — the VM does not trust
metamethod return values.

### 9.3 The `__index` and `__newindex` Protocol

When `GETTABLE` finds a nil value in a table, it calls `luaV_gettable()` which tries
the `__index` metamethod. If the metatable's `__index` is a table, access recurses into
that table. If it is a function, the function is called with `(table, key)`.

`__newindex` works similarly for table writes: if the key is not found and `__newindex`
exists, it is called (or the write goes to the `__newindex` table).

---

## 10. The C API

### 10.1 Stack-Based Programming

**File:** `src/lapi.c` (~1500 lines)

The C API is stack-based: all interactions with Lua values go through a virtual stack
of `TValue` slots. The API is organized into families:

**Push functions** (C → stack): `lua_pushnil()`, `lua_pushinteger()`, `lua_pushstring()`,
`lua_pushcclosure()`, `lua_createtable()`, etc.

**Access functions** (stack → C): `lua_tonumber()`, `lua_toboolean()`, `lua_tolstring()`,
`lua_tocfunction()`, etc.

**Get/set functions** (Lua ↔ stack): `lua_getglobal()`, `lua_settable()`,
`lua_rawget()`, `lua_setmetatable()`, etc.

**Stack manipulation**: `lua_settop()`, `lua_rotate()`, `lua_copy()`, `lua_insert()`,
`lua_remove()`, `lua_checkstack()`.

**Call/load**: `lua_callk()`, `lua_pcallk()`, `lua_load()`, `lua_dump()`.

Key internal mechanics:
- **Pseudo-indices**: `LUA_REGISTRYINDEX` and `lua_upvalueindex()` are negative indices
  that don't correspond to real stack slots but provide access to the registry and
  upvalues.
- **API re-entry**: `api_incr_top()` increments `L->top` after pushes. `api_checktop()`
  validates `L->top` hasn't been violated by C code.
- **GC interaction**: any API call that may allocate (`lua_pushstring`, `lua_settable`,
  etc.) must call `luaC_checkGC()` or `luaC_condGC()`.

### 10.2 Auxiliary Library

**File:** `src/lauxlib.c` (~1200 lines)

The auxiliary library provides higher-level helpers built on the raw C API:
- **Error handling**: `luaL_error()`, `luaL_argerror()`, `luaL_typeerror()` — format
  error messages and raise Lua errors from C
- **Type checking**: `luaL_checkinteger()`, `luaL_checkstring()`, `luaL_checktype()`,
  `luaL_checkany()` — validate argument types with descriptive errors
- **Table helpers**: `luaL_getsubtable()`, `luaL_setfuncs()`, `luaL_requiref()` —
  module registration patterns
- **Buffered output**: `luaL_Buffer` for efficient string building (`luaL_pushresult()`)
- **Chunk loading**: `luaL_loadfilex()`, `luaL_loadstring()`, `luaL_loadbuffer()` —
  combines source reading + `lua_load()` with error formatting
- **References**: `luaL_ref()` / `luaL_unref()` — store values in the registry with
  integer keys, for deferred access

---

## 11. Coroutines

**Files:** `src/lvm.c`, `src/ldo.c`, `src/lcorolib.c`

### 11.1 Yield Mechanism

A coroutine yield (`coroutine.yield()` → `lua_yieldk()` → `luaD_throw(LUA_YIELD)`)
works by:
1. Setting `L->status = LUA_YIELD`
2. Closing all to-be-closed variables on the current frame
3. Saving `ci->u.l.savedpc` so execution can resume later
4. `longjmp`ing back to the most recent `lua_resume()` call site

When `coroutine.resume()` is called:
1. `lua_resume()` (in `ldo.c`) sets up the target thread's state
2. Calls `luaD_rawrunprotected()` → `luaV_execute()` to resume
3. The VM picks up at the saved `pc` and continues

The `CIST_YPCALL` flag marks a protected call that can yield; `CIST_FRESH` marks a
brand-new `luaV_execute` frame.

### 11.2 Continuation Functions

For C functions that need to yield across multiple Lua calls (e.g. `string.gmatch`),
the `lua_KFunction` continuation protocol allows the C function to register a
continuation (`k` in `CallInfo.u.c`) that will be called when the coroutine resumes.
The continuation receives the yield status and a `lua_KContext` token.

The VM handles this in `OP_CALL` / `OP_TAILCALL`: when a C function returns
`LUA_YIELD`, the VM saves the continuation and context, then yields. On resume, the
continuation is called instead of the original function.

---

## 12. Debug and Hooks

**File:** `src/ldebug.c` (~1000 lines)

The debug API (`lua_getinfo()`, `lua_getlocal()`, `lua_setlocal()`, `lua_sethook()`)
is implemented in `ldebug.c`. It provides:

- **Stack introspection**: `lua_getstack()` returns a `lua_Debug` for a given call level;
  `lua_getinfo()` fills in source file, line numbers, function name, parameter count, etc.
- **Local variable access**: walks the `LocVar` arrays of each `Proto` in the call stack
  to find variable names and their register slots.
- **Hooks**: `lua_sethook()` registers a hook function that is called on call, return,
  line, and/or count events. The `trap` field in `CallInfo.u.l` is checked by the VM's
  dispatch loop; when set, control is transferred to `luaD_hook()`.

The debug system uses **line number compression**: `Proto.lineinfo` stores deltas between
consecutive instruction lines (packed into bytes when possible), with an `abslineinfo`
array for binary-search acceleration on large functions.

---

## 13. Standard Libraries

### 13.1 Library Registration

**File:** `src/linit.c` (~60 lines)

A tiny file that defines `luaL_openlibs()`, which calls `luaopen_*` for each standard
library:
- `luaopen_base()`, `luaopen_package()`, `luaopen_coroutine()`, `luaopen_table()`,
  `luaopen_io()`, `luaopen_os()`, `luaopen_string()`, `luaopen_math()`,
  `luaopen_debug()`, `luaopen_utf8()`

Each library is defined as an array of `{name, function}` pairs (e.g. `base_funcs[]` in
`lbaselib.c`) and registered via `luaL_setfuncs()`.

### 13.2 The Base Library

**File:** `src/lbaselib.c` (~550 lines)

Implements the core Lua functions: `print`, `type`, `tostring`, `tonumber`, `pairs`,
`ipairs`, `pcall`, `xpcall`, `select`, `load`, `loadfile`, `assert`, `error`, `rawget`,
`rawset`, `rawlen`, `collectgarbage`, `dofile`, `require`.

The `pcall`/`xpcall` implementation uses `lua_pcallk()` with error handlers; `loadfile`
uses `luaL_loadfilex()` plus file handle management.

---

## 14. Buffered I/O

**File:** `src/lzio.c` (~90 lines)

The `MBuffer` type provides a small reusable buffer (`n` bytes used out of `buffsize`,
with `buffer` pointing to the allocated memory). It is used by the lexer and by
`luaL_Buffer`.

`luaZ_fill()` fills the buffer from a `lua_Reader` callback (used by `lua_load()` to
read chunks from files, strings, or custom readers). The `luaZ_read()` function reads
a fixed number of bytes, used by `lundump.c` during bytecode loading.

---

## 15. CLI Front-Ends

### `lua.c` — Standalone Interpreter

**File:** `src/lua.c` (~800 lines)

Implements the `lua` command-line tool:
- **Argument parsing** (`collectargs()`): handles `-e` (evaluate), `-l` (load library),
  `-` (stdin), `-v` (version), `--` (end of options), and file arguments.
- **REPL** (`doREPL()`): reads lines with `readline()` or `fgets()`, loads them via
  `lua_load()`, and calls `lua_pcall()`. Supports interactive continuation on incomplete
  input.
- **Script execution**: `pmain()` is the protected main; it calls `luaL_openlibs()`,
  processes `-e`/`-l` arguments, and runs files or stdin.
- **Signal handling**: sets `SIGINT` to `lstop()` which calls `lua_sethook()` to
  interrupt the VM on the next instruction.

### `luac.c` — Bytecode Compiler

**File:** `src/luac.c` (~730 lines)

Implements the `luac` command-line tool:
- Compiles `.lua` files to `.luac` bytecode using `lua_load()` with a reader callback
- `-o` flag specifies output file
- `-s` flag strips debug information (line numbers, local names)
- `-p` flag only compiles (no output), used for syntax checking
- `-l` flag lists bytecode (calls `PrintFunction()` which disassembles `Proto`)

---

*This document covers the architecture of the Lua 5.5 interpreter as implemented in
Eira's `src/` directory. For per-function documentation, see the individual `.md` files
in this directory, each marked with `> **AI-Generated Documentation**`.*
