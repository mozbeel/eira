# lopcodes.c — Opcode Metadata and Properties

> **AI-Generated Documentation**

## Overview

`lopcodes.c` defines the **static per-opcode metadata table** for the Eira virtual machine's instruction set. While `lopcodes.h` declares the `OpCode` enum (with ~85 opcodes), instruction format constants, and bit-manipulation macros, `lopcodes.c` provides the runtime data that describes each opcode's properties: its instruction format mode, whether it sets register A, whether it is a test instruction, whether it produces or consumes a stack "top", and whether it is a metamethod instruction.

This metadata is consumed by both the code generator (`lcode.c`) and the VM (`lvm.c`). The code generator uses it to verify instruction emission (checking format modes via `getOpMode`), while the VM uses it to track the stack top pointer — a critical invariant that ensures open calls, returns, and `SETLIST` instructions correctly reference multiple values on the stack.

The file also implements `luaP_isIT()`, a function that determines whether a given instruction **consumes** the stack top set by the previous instruction. This is part of a pair of queries (`luaP_isOT` / `luaP_isIT`) that enforce the compiler invariant: an instruction may use "top" only if the preceding instruction produces multiple results (open calls, varargs). The code generator asserts this invariant in `luaK_finish`.

The entire opcode set for Eira includes standard Lua opcodes (load, store, arithmetic, comparison, call, return, for-loop, closure, vararg), Eira-specific additions (`OP_GETVARG` for indexed vararg table access, `OP_ERRNNIL` for global-declaration nil checking), and metamethod fallback instructions (`OP_MMBIN`, `OP_MMBINI`, `OP_MMBINK`).

## Key Types / Macros

| Name | Defined In | Description |
|------|-----------|-------------|
| `OpCode` | `lopcodes.h:231` | Enum of all VM opcodes (~85 entries), from `OP_MOVE` to `OP_EXTRAARG`. Each opcode has a defined instruction format (`iABC`, `ivABC`, `iABx`, `iAsBx`, `iAx`, `isJ`). |
| `OpMode` | `lopcodes.h:36` | Enum of instruction format modes: `iABC`, `ivABC` (variant with larger C/B fields), `iABx`, `iAsBx` (signed Bx), `iAx`, `isJ` (signed jump). |
| `luaP_opmodes` | `lopcodes.c:30` | Static array of `lu_byte` values indexed by opcode. Each byte packs 6 properties: mode (bits 0–2), sets A (bit 3), test (bit 4), uses top (bit 5), produces top (bit 6), metamethod (bit 7). |
| `opmode(mm,ot,it,t,a,m)` | `lopcodes.c:20` | Macro to construct an opcode mode byte from its component properties. |
| `testITMode(m)` | `lopcodes.c:122` | Extracts the "uses top" (IT) bit from a mode byte. |
| `testTMode(m)` | `lopcodes.h:429` | Extracts the "test" (T) bit — used by the code generator to find controlling test instructions before jumps. |
| `getOpMode(m)` | `lopcodes.h:427` | Extracts the instruction format (bits 0–2) from a mode byte. |
| `testAMode(m)` | `lopcodes.h:428` | Extracts the "sets register A" bit. |
| `testMMMode(m)` | `lopcodes.h:430` | Extracts the "metamethod" bit — identifies instructions that call metamethods. |
| `luaP_isOT(i)` | `lopcodes.h:436` | Macro: tests if instruction `i` **produces** the stack top (tail calls, or instructions with `OT` bit set and C==0). |
| `NUM_OPCODES` | `lopcodes.h:351` | Total count of opcodes (`OP_EXTRAARG + 1`). |

## The `luaP_opmodes` Table

The table at `lopcodes.c:30–117` has one entry per opcode, in the same order as the `OpCode` enum (enforced by "ORDER OP" comments). Each entry is constructed via the `opmode()` macro with six fields:

| Bit | Name | Meaning |
|-----|------|---------|
| 7 | MM | Instruction is a metamethod fallback (skipped on fast-path success) |
| 6 | OT | Instruction **produces** a stack top (multi-result output) |
| 5 | IT | Instruction **uses** a stack top (reads multiple values from previous instruction) |
| 4 | T | Instruction is a test (must be followed by a jump) |
| 3 | A | Instruction writes to register A |
| 0–2 | mode | Instruction format (`iABC`=0, `ivABC`=1, `iABx`=2, `iAsBx`=3, `iAx`=4, `isJ`=5) |

Key patterns in the table:

- **Load instructions** (`OP_MOVE` through `OP_LOADNIL`): all set A, none are tests or metamethods.
- **Store instructions** (`OP_SETUPVAL`, `OP_SETTABUP`, `OP_SETTABLE`, `OP_SETI`, `OP_SETFIELD`): do not set A (they write to B/C targets).
- **Arithmetic/bitwise K-variants** (`OP_ADDK` through `OP_SHRI`): set A, no test, no metamethod (the preceding `OP_MMBIN*` handles the fallback).
- **Metamethod instructions** (`OP_MMBIN`, `OP_MMBINI`, `OP_MMBINK`): MM bit set, do not set A.
- **Comparison/test instructions** (`OP_EQ` through `OP_TESTSET`): T bit set. `OP_TESTSET` also sets A (it copies the value).
- **Call/return** (`OP_CALL`, `OP_TAILCALL`): both OT and IT bits set (produce and consume multi-values). `OP_RETURN` has only IT. `OP_RETURN0`/`OP_RETURN1` have neither.
- **For-loop** (`OP_FORLOOP`, `OP_FORFORPREP`, `OP_TFORLOOP`): set A, no test (loop control is internal).
- **`OP_SETLIST`**: IT bit set (when `vB==0`, reads up to stack top).
- **`OP_CLOSURE`**: sets A, loads a closure from the prototype table.
- **`OP_VARARG`**: sets A, OT bit set (produces multiple results).
- **`OP_ERRNNIL`**: Eira-specific — does not set A, no test, used for global-declaration nil checking.
- **`OP_VARARGPREP`**: IT bit always set (always adjusts the vararg state).
- **`OP_EXTRAARG`**: `iAx` format, no A bit, no flags — purely auxiliary data.

## Functions

### `luaP_isIT(Instruction i)`

Determines whether instruction `i` **consumes** the stack top set by the previous instruction. This is the inverse of `luaP_isOT` — where `luaP_isOT` checks if an instruction *produces* multi-values, `luaP_isIT` checks if the *next* instruction *reads* them.

Three cases:

1. **`OP_SETLIST`**: Uses top only when `vB == 0` (the "read up to top" form).
2. **`OP_VARARGPREP`**: Always uses top (it adjusts the vararg state).
3. **Default**: Uses top when the instruction's mode has the IT bit set AND `B == 0` (the "variable count" form, e.g., `OP_CALL` with B=0 reads up to top).

The code generator (`luaK_finish`) asserts that `luaP_isOT(prev) == luaP_isIT(curr)` for every pair of adjacent instructions, ensuring the VM's top-tracking invariant is maintained.

## Instruction Format Summary

The six instruction formats encode all operands within a 32-bit word:

- **`iABC`**: 8-bit A, 8-bit B, 8-bit C, 1-bit k flag, 7-bit opcode.
- **`ivABC`**: 8-bit A, 6-bit vB, 10-bit vC, 1-bit k flag, 7-bit opcode (used by `OP_NEWTABLE` and `OP_SETLIST` for larger counts).
- **`iABx`**: 8-bit A, 17-bit unsigned Bx, 7-bit opcode.
- **`iAsBx`**: 8-bit A, 17-bit signed Bx (excess-K encoded), 7-bit opcode.
- **`iAx`**: 25-bit unsigned Ax, 7-bit opcode (used by `OP_EXTRAARG`).
- **`isJ`**: 25-bit signed sJ (excess-K encoded), 1-bit k flag, 7-bit opcode (used by `OP_JMP`).

Signed fields use excess-K encoding: the stored unsigned value is the represented signed value plus half the maximum unsigned range (`OFFSET_sBx`, `OFFSET_sJ`, `OFFSET_sC`).
