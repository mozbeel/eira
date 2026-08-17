# lcode.c — Code Generator

> **AI-Generated Documentation**

## Overview

`lcode.c` is the **code generator** for the Eira compiler. It translates the `expdesc` expression descriptors built by the recursive-descent parser (`lparser.c`) into `Instruction` streams for `Proto` structures. The code generator owns the **register allocator** (`freereg`/`maxstacksize`), the **constant table** (`k`), **jump patch lists**, and the final **peephole optimization pass** (`luaK_finish`).

Expressions in Eira's compiler are represented lazily: an `expdesc` may describe a value that is not yet in a register — it might be a constant, a local variable, an upvalue, an indexed access, or a pending jump. The `luaK_*` family of functions progressively "discharge" these descriptors into registers and concrete instructions as needed by the parser. This lazy approach enables constant folding, register reuse, and the elimination of redundant loads.

The code generator uses **jump lists** (linked chains of `OP_JMP` instructions) for control flow. Each `expdesc` carries two patch lists — `t` (true/continue) and `f` (false/exit) — which are linked together and patched to their final destinations once the target code position is known. This avoids the need to pre-compute code sizes.

Arithmetic and bitwise operators use a hierarchy of specialized emission strategies: immediate operands (`sC` field), constant-table operands (`K`), or two-register forms. Metamethod fallback instructions (`OP_MMBIN`, `OP_MMBINI`, `OP_MMBINK`) are emitted inline after each arithmetic/bitwise opcode, skipped on success by the VM.

The file also handles **line information** encoding: a compact relative scheme stores the difference from the previous line in a single byte, with absolute line info saved periodically when the difference is too large or after too many instructions.

## Key Types / Macros

| Name | Defined In | Description |
|------|-----------|-------------|
| `BinOpr` | `lcode.h:26` | Enum of binary operators: arithmetic (`OPR_ADD`..`OPR_POW`), bitwise (`OPR_BAND`..`OPR_SHR`), string (`OPR_CONCAT`), comparison (`OPR_EQ`..`OPR_GE`), logical (`OPR_AND`, `OPR_OR`). |
| `UnOpr` | `lcode.h:51` | Enum of unary operators: `OPR_MINUS`, `OPR_BNOT`, `OPR_NOT`, `OPR_LEN`. |
| `NO_JUMP` | `lcode.h:20` | Sentinel value (-1) marking the end of a jump list or an empty patch list. |
| `hasjumps(e)` | `lcode.c:39` | Tests if an expression descriptor has non-trivial jump lists (`t != f`). |
| `foldbinop(op)` | `lcode.h:45` | True if the operator is arithmetic or bitwise (candidates for constant folding). |
| `LIMLINEDIFF` | `lcode.c:327` | Maximum relative line difference before switching to absolute line info (0x80). |
| `MAXIWTHABS` | (from llimits.h) | Maximum instructions between absolute line info entries. |
| `OFFSET_sBx` | `lopcodes.h:88` | Bias for signed Bx field (half of MAXARG_Bx). |
| `OFFSET_sJ` | `lopcodes.h:103` | Bias for signed jump offset field. |
| `OFFSET_sC` | `lopcodes.h:111` | Bias for signed C field. |
| `MAXINDEXRK` | `lopcodes.h:202` | Maximum constant index that fits in an R/K operand field. |

## Functions

### Error Handling

#### `luaK_semerror(LexState *ls, const char *fmt, ...)`

Formats a semantic error message and raises `LUA_ERRSYNTAX`. Resets the token to 0 (removing "near \<token\>" from the final message) and adjusts the line number back to the last consumed token.

### Constant and Numeric Utilities

#### `tonumeral(const expdesc *e, TValue *v)` (static)

Returns 1 if `e` is a numeric constant (`VKINT` or `VKFLT`) without jumps, filling `v` with its value. Returns 0 otherwise.

#### `const2val(FuncState *fs, const expdesc *e)` (static)

Retrieves the `TValue` for a `VCONST` (compile-time constant) expression.

#### `luaK_exp2const(FuncState *fs, const expdesc *e, TValue *v)`

Converts an expression to a constant `TValue` if possible. Handles `VFALSE`, `VTRUE`, `VNIL`, `VKSTR`, `VCONST`, and numeric kinds. Returns 1 on success, 0 if the expression has jumps.

#### `const2exp(TValue *v, expdesc *e)` (static)

Converts a `TValue` into an `expdesc`. Used to materialize compile-time constants.

### Jump List Management

#### `getjump(FuncState *fs, int pc)` (static)

Follows a jump instruction at `pc` to its destination (converting the relative offset to absolute). Returns `NO_JUMP` for end-of-list.

#### `fixjump(FuncState *fs, int pc, int dest)` (static)

Patches the `OP_JMP` at `pc` to jump to `dest` (computed as a relative offset).

#### `luaK_concat(FuncState *fs, int *l1, int l2)`

Concatenates two jump lists: appends `l2` to the end of `*l1`.

#### `luaK_jump(FuncState *fs)`

Emits an unconditional `OP_JMP` with `NO_JUMP` destination and returns its pc for later patching.

#### `luaK_getlabel(FuncState *fs)`

Returns the current `pc` and marks it as a jump target (sets `lasttarget`) to prevent wrong optimizations across basic blocks.

#### `luaK_patchlist(FuncState *fs, int list, int target)`

Patches all jumps in `list` to jump to `target`. Calls `patchlistaux` which handles both test-jumps (with value) and plain jumps.

#### `luaK_patchtohere(FuncState *fs, int list)`

Marks the current pc as a label and patches all jumps in `list` to land here.

#### `patchlistaux(FuncState *fs, int list, int vtarget, int reg, int dtarget)` (static)

Traverses a jump list, patching test-set instructions to `vtarget` (with register) and other jumps to `dtarget`.

#### `patchtestreg(FuncState *fs, int node, int reg)` (static)

Patches the destination register of a `TESTSET` instruction, or converts it to a simple `TEST` if no register is needed.

#### `removevalues(FuncState *fs, int list)` (static)

Traverses a jump list, removing value production from all test-set instructions (converting to `TEST`).

#### `getjumpcontrol(FuncState *fs, int pc)` (static)

Returns a pointer to the instruction controlling a jump (the preceding `TEST`/`TESTSET` if present, or the jump itself).

### Instruction Emission

#### `luaK_code(FuncState *fs, Instruction i)`

The fundamental emission function. Appends instruction `i` to the `Proto`'s code array, saves line info, and returns the instruction's pc.

#### `luaK_codeABCk(FuncState *fs, OpCode o, int A, int B, int C, int k)`

Emits an `iABC` instruction with the `k` flag.

#### `luaK_codevABCk(FuncState *fs, OpCode o, int A, int B, int C, int k)`

Emits an `ivABC` (variant ABC) instruction with extended `vB`/`vC` fields.

#### `luaK_codeABx(FuncState *fs, OpCode o, int A, int Bc)`

Emits an `iABx` instruction.

#### `codeAsBx(FuncState *fs, OpCode o, int A, int Bc)` (static)

Emits an `iAsBx` (signed Bx) instruction with bias offset.

#### `codesJ(FuncState *fs, OpCode o, int sj, int k)` (static)

Emits an `isJ` (signed jump) instruction.

#### `codeextraarg(FuncState *fs, int A)` (static)

Emits an `OP_EXTRAARG` instruction for large constant or array indices.

#### `luaK_codek(FuncState *fs, int reg, int k)` (static)

Emits `OP_LOADK` or `OP_LOADKX` + `OP_EXTRAARG` depending on whether `k` fits in 18 bits.

#### `savelineinfo(FuncState *fs, Proto *f, int line)` (static)

Saves line information for the last emitted instruction. Uses relative encoding (difference from previous line) when possible; switches to absolute line info when the difference exceeds `LIMLINEDIFF` or after `MAXIWTHABS` instructions.

#### `removelastlineinfo(FuncState *fs)` (static)

Undoes the line info for the last instruction (used when replacing an instruction).

#### `removelastinstruction(FuncState *fs)` (static)

Removes the last emitted instruction and its line info.

#### `previousinstruction(FuncState *fs)` (static)

Returns a pointer to the previous instruction, or an invalid sentinel if a jump target may exist between it and the current pc (preventing wrong optimizations).

### Register Allocation

#### `luaK_checkstack(FuncState *fs, int n)`

Ensures the register stack can hold `n` more registers, updating `maxstacksize` if needed.

#### `luaK_reserveregs(FuncState *fs, int n)`

Reserves `n` registers by advancing `freereg`.

#### `freereg(FuncState *fs, int reg)` (static)

Frees a single register if it is not occupied by a local variable.

#### `freeregs(FuncState *fs, int r1, int r2)` (static)

Frees two registers in the proper order (higher index first).

#### `freeexp(FuncState *fs, expdesc *e)` (static)

Frees the register used by a `VNONRELOC` expression.

#### `freeexps(FuncState *fs, expdesc *e1, expdesc *e2)` (static)

Frees registers for two expressions in proper order.

### Constant Table Management

#### `addk(FuncState *fs, Proto *f, TValue *v)` (static)

Appends constant `v` to the `Proto`'s `k` array and returns its index.

#### `k2proto(FuncState *fs, TValue *key, TValue *v)` (static)

Uses the scanner's constant-cache table (`fs->kcache`) to look up and reuse constants. On a miss, calls `addk` and caches the result.

#### `stringK(FuncState *fs, TString *s)` (static)

Adds a string to the constant table.

#### `luaK_intK(FuncState *fs, lua_Integer n)` (static)

Adds an integer constant.

#### `luaK_numberK(FuncState *fs, lua_Number r)` (static)

Adds a float constant. Uses a clever technique to avoid collisions with integer keys: adds the smallest significant power-of-two fraction to create a unique key. Handles zero specially with the `FuncState` pointer as key.

#### `boolF(FuncState *fs)` / `boolT(FuncState *fs)` (static)

Add `false` / `true` to the constant table.

#### `nilK(FuncState *fs)` (static)

Adds `nil` to the constant table (using the kcache table as the key since nil cannot be a key).

### Expression Discharge and Conversion

#### `luaK_dischargevars(FuncState *fs, expdesc *e)`

Converts any variable expression kind into a concrete value. `VLOCAL` becomes `VNONRELOC`, `VUPVAL` becomes `VRELOC` (emits `OP_GETUPVAL`), indexed forms emit their respective `GET*` instructions, and multi-return expressions are reduced to one result.

#### `discharge2reg(FuncState *fs, expdesc *e, int reg)` (static)

Forces the expression value into a specific register. Handles all expression kinds including constant loads (`VNIL`, `VTRUE`, `VFALSE`, `VK`, `VKFLT`, `VKINT`) and `VRELOC`/`VNONRELOC` movements.

#### `discharge2anyreg(FuncState *fs, expdesc *e)` (static)

Ensures the value is in some register (any register), allocating one if needed.

#### `exp2reg(FuncState *fs, expdesc *e, int reg)` (static)

Ensures the final expression value (including jump list outcomes) is in `reg`. If the expression has jumps, patches them to load booleans (`OP_LFALSESKIP`/`OP_LOADTRUE`) at the appropriate targets.

#### `luaK_exp2nextreg(FuncState *fs, expdesc *e)`

Moves the expression to the next available register. Used for sequential value packing.

#### `luaK_exp2anyreg(FuncState *fs, expdesc *e)`

Ensures the value is in a register and returns that register. Optimizes when the expression already occupies a non-local register.

#### `luaK_exp2anyregup(FuncState *fs, expdesc *e)`

Like `exp2anyreg` but also accepts upvalues and vararg parameters in place.

#### `luaK_exp2val(FuncState *fs, expdesc *e)`

Ensures the value is either in a register or a constant (not a variable descriptor). Used for indexed subscripts.

#### `luaK_exp2K(FuncState *fs, expdesc *e)` (static)

Tries to make the expression a constant with a `K` index that fits in the R/K operand field.

#### `exp2RK(FuncState *fs, expdesc *e)` (static)

Ensures the expression is in R/K range (constant in K or value in a register). Returns 1 if K, 0 if register.

#### `str2K(FuncState *fs, expdesc *e)` (static)

Converts a `VKSTR` expression to `VK` by adding the string to the constant table.

#### `const2exp(TValue *v, expdesc *e)` (static)

Converts a `TValue` to an `expdesc`.

### Assignment Code Generation

#### `luaK_storevar(FuncState *fs, expdesc *var, expdesc *ex)`

Emits the store instruction for different variable kinds: `VLOCAL` (MOVE), `VUPVAL` (SETUPVAL), `VINDEXUP` (SETTABUP), `VINDEXI` (SETI), `VINDEXSTR` (SETFIELD), `VINDEXED` (SETTABLE).

#### `luaK_setreturns(FuncState *fs, expdesc *e, int nresults)`

Patches a multi-return expression (VCALL or VVARARG) to return exactly `nresults` values.

#### `luaK_setoneret(FuncState *fs, expdesc *e)`

Fixes a multi-return expression to return exactly one result.

#### `luaK_vapar2local(FuncState *fs, expdesc *var)`

Converts a vararg parameter into a regular local variable (requiring a vararg table).

#### `codeABRK(FuncState *fs, OpCode o, int A, int B, expdesc *ec)` (static)

Emits an instruction whose B operand is an R/K index, folding `ec` into a constant when possible.

### Comparison and Conditional Code

#### `luaK_goiftrue(FuncState *fs, expdesc *e)`

Emits code to branch when `e` is true. Inverts existing conditions, folds known constants, and generates test instructions for unknown values.

#### `luaK_goiffalse(FuncState *fs, expdesc *e)` (static)

Emits code to branch when `e` is false.

#### `jumponcond(FuncState *fs, expdesc *e, int cond)` (static)

Emits a conditional jump for expression `e`. Optimizes `NOT` by inverting the condition and removing the opcode.

#### `negatecondition(FuncState *fs, expdesc *e)` (static)

Flips the `k` flag of a comparison instruction, inverting its sense.

#### `codenot(FuncState *fs, expdesc *e)` (static)

Emits `not e` with constant folding: inverts known constants, negates existing conditions, and emits `OP_NOT` for runtime values. Swaps the true/false jump lists.

#### `need_value(FuncState *fs, int list)` (static)

Checks if any jump in a list needs a value-producing instruction (i.e., is not a `TESTSET`).

### Operator Code Generation

#### `luaK_prefix(FuncState *fs, UnOpr opr, expdesc *e, int line)`

Emits a prefix (unary) operator. Attempts constant folding for `-` and `~`, falls back to `codeunexpval` for runtime operations, and calls `codenot` for `not`.

#### `luaK_infix(FuncState *fs, BinOpr op, expdesc *v)`

Prepares the first operand of a binary operation. For logical operators (`and`/`or`), sets up the short-circuit jump. For arithmetic, tries to keep numerals unexpanded for folding. For comparisons, ensures the operand is in R/K form.

#### `luaK_posfix(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int line)`

Emits code for the complete binary operation after both operands are available. Dispatches to specialized emitters: `codecommutative` for `+`/`*`, `codearith` for other arithmetic, `codebitwise` for bitwise, `codeeq`/`codeorder` for comparisons, `codeconcat` for `..`, and logical joinery for `and`/`or`.

#### `codearith(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int flip, int line)` (static)

Emits arithmetic operations, using K-operand or immediate forms when possible.

#### `codecommutative(FuncState *fs, BinOpr op, expdesc *e1, expdesc *e2, int line)` (static)

For commutative operators, swaps operands to put constants on the right for better code generation.

#### `codebitwise(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int line)` (static)

Emits bitwise operations, always trying K-operand form.

#### `codebinexpval(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int line)` (static)

Emits a two-register binary operation with a following metamethod fallback instruction.

#### `codebinK(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int flip, int line)` (static)

Emits a binary operation with a K-operand (constant table index) form.

#### `codebini(FuncState *fs, OpCode op, expdesc *e1, expdesc *e2, int flip, int line, TMS event)` (static)

Emits a binary operation with an immediate integer operand in the `sC` field.

#### `codebinNoK(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2, int flip, int line)` (static)

Emits a two-register binary operation (no constant folding possible).

#### `finishbinexpval(FuncState *fs, expdesc *e1, expdesc *e2, OpCode op, int v2, int flip, int line, OpCode mmop, TMS event)` (static)

Common tail for binary operations: emits the operation instruction and its metamethod fallback.

#### `finishbinexpneg(FuncState *fs, expdesc *e1, expdesc *e2, OpCode op, int line, TMS event)` (static)

Optimizes subtraction/shift by negating a small integer constant to use an addition/shift-right form.

#### `codeorder(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2)` (static)

Emits order comparisons, with immediate-operand variants (`OP_LTI`, `OP_GTI`, etc.) and swapped-operand forms.

#### `codeeq(FuncState *fs, BinOpr opr, expdesc *e1, expdesc *e2)` (static)

Emits equality comparisons (`OP_EQ`, `OP_EQK`, `OP_EQI`).

#### `codeconcat(FuncState *fs, expdesc *e1, expdesc *e2, int line)` (static)

Emits string concatenation. Merges adjacent `OP_CONCAT` instructions when the right operand is already a concatenation (exploiting right-associativity).

### Self / Indexing

#### `luaK_self(FuncState *fs, expdesc *e, expdesc *key)`

Emits the `OP_SELF` instruction for method calls: `e.key(e, ...)`. Falls back to `OP_MOVE` + `OP_GETTABLE` when the method name cannot be a short string constant.

#### `luaK_indexed(FuncState *fs, expdesc *t, expdesc *k)`

Creates an indexed expression `t[k]`. Chooses the optimal index form: `VINDEXUP` (upvalue + string key), `VINDEXSTR` (register + string key), `VINDEXI` (register + integer key), `VVARGIND` (vararg table index), or `VINDEXED` (generic register index).

### Nil and Return

#### `luaK_nil(FuncState *fs, int from, int n)`

Emits `OP_LOADNIL` with a range optimization: merges with a preceding `OP_LOADNIL` when their ranges overlap or are adjacent.

#### `luaK_ret(FuncState *fs, int first, int nret)`

Emits a return instruction: `OP_RETURN0`, `OP_RETURN1`, or `OP_RETURN` depending on the count.

### Table Construction

#### `luaK_settablesize(FuncState *fs, int pc, int ra, int asize, int hsize)`

Patches an `OP_NEWTABLE` instruction with the final array and hash sizes (hash stored as log2+1). Emits `OP_EXTRAARG` for large array sizes.

#### `luaK_setlist(FuncState *fs, int base, int nelems, int tostore)`

Emits `OP_SETLIST` for table array construction. Handles large element counts with `OP_EXTRAARG`.

### Utility

#### `luaK_fixline(FuncState *fs, int line)`

Replaces the line info for the last emitted instruction.

#### `luaK_int(FuncState *fs, int reg, lua_Integer i)`

Emits an integer load: `OP_LOADI` when the value fits in sBx, otherwise constant-table load.

#### `luaK_float(FuncState *fs, int reg, lua_Number f)` (static)

Emits a float load: `OP_LOADF` when the float is an exact small integer, otherwise constant-table load.

#### `luaK_codecheckglobal(FuncState *fs, expdesc *var, int k, int line)`

Emits `OP_ERRNNIL` to check that a global variable is defined (raises an error if nil).

#### `constfolding(FuncState *fs, int op, expdesc *e1, const expdesc *e2)` (static)

Attempts constant folding for arithmetic/bitwise operations. Applies `luaO_rawarith` and folds the result unless it is NaN or negative zero.

#### `validop(int op, TValue *v1, TValue *v2)` (static)

Checks whether constant folding is safe: verifies integer conversion for bitwise ops and non-zero divisor for division.

#### `binopr2op(BinOpr opr, BinOpr baser, OpCode base)` (inline)

Converts a binary operator enum to the corresponding opcode.

#### `unopr2op(UnOpr opr)` (inline)

Converts a unary operator enum to the corresponding opcode.

#### `binopr2TM(BinOpr opr)` (inline)

Converts a binary operator to its metamethod event.

#### `swapexps(expdesc *e1, expdesc *e2)` (static)

Swaps two expression descriptors.

### Peephole Optimization

#### `luaK_finish(FuncState *fs)`

Final pass over the compiled code. Adjusts `OP_RETURN0`/`OP_RETURN1` to `OP_RETURN` when upvalue closing or hidden varargs are needed. Sets the `k` flag on return/tailcall instructions for upvalue closing. Converts `OP_GETVARG` to `OP_GETTABLE` and flags `OP_VARARG` when a vararg table is used. Optimizes jump-to-jump chains via `finaltarget`.

#### `finaltarget(Instruction *code, int i)` (static)

Follows a chain of `OP_JMP` instructions to find the ultimate non-jump target (limited to 100 hops to prevent infinite loops).
