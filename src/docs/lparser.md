# lparser.c — Recursive-Descent Parser

> **AI-Generated Documentation**

## Overview

`lparser.c` implements the **recursive-descent parser** for the Eira Lua dialect. It consumes the token stream produced by `llex.c`, applies the grammar rules for statements, expressions, table constructors, variable declarations, labels/gotos, and function definitions, and drives `lcode.c` to emit bytecode into `Proto` structures. The parser is fully integrated with code generation: there is no separate AST — expressions are represented as `expdesc` descriptors that are progressively lowered into instructions.

The parser handles Eira-specific extensions beyond standard Lua 5.4, notably the `global` keyword for explicit global declarations, `fn` as an alias for `function`, and `global function` declarations. The `LUA_COMPAT_GLOBAL` flag provides backward compatibility when `global` is not a reserved word.

Key architectural features include: a linked-list stack of `FuncState` objects (one per nested function), a linked-list stack of `BlockCnt` objects (one per scope block), a shared `Dyndata` structure for active variables, pending gotos, and labels, and a one-token look-ahead mechanism used for disambiguation (e.g., table constructor fields).

The file is the third stage of the compilation pipeline (lexing → parsing → **code generation** → bytecode → VM). It sits between the lexer and the code generator, calling `luaX_next()` / `luaX_lookahead()` for tokens and `luaK_*()` functions for instruction emission.

## Key Types / Macros

| Name | Defined In | Description |
|------|-----------|-------------|
| `FuncState` | `lparser.h:166` | Per-function compilation state: `Proto *f`, enclosing `FuncState *prev`, `LexState *ls`, block chain `BlockCnt *bl`, constant cache `Table *kcache`, pc counter, register allocator (`freereg`), upvalue/variable/constant counts. |
| `BlockCnt` | `lparser.c:53` | Scope block descriptor: links to enclosing block (`previous`), tracks `firstlabel`, `firstgoto`, `nactvar` at entry, `upval` (escaping upvalues), `isloop` (0/1/2 for non-loop/loop/loop-with-breaks), `insidetbc`. |
| `expdesc` | `lparser.h:78` | Expression descriptor with `expkind k`, a union for values/indices, and `t`/`f` patch lists for conditional jumps. |
| `expkind` | `lparser.h:25` | Enum of expression kinds: `VVOID`, `VNIL`, `VTRUE`, `VFALSE`, `VK`, `VKFLT`, `VKINT`, `VKSTR`, `VNONRELOC`, `VLOCAL`, `VVARGVAR`, `VGLOBAL`, `VUPVAL`, `VCONST`, `VINDEXED`, `VVARGIND`, `VINDEXUP`, `VINDEXI`, `VINDEXSTR`, `VJMP`, `VRELOC`, `VCALL`, `VVARARG`. |
| `Vardesc` | `lparser.h:118` | Active variable descriptor: `kind` (VDKREG, RDKCONST, RDKVAVAR, RDKTOCLOSE, RDKCTC, GDKREG, GDKCONST), `ridx` (register), `pidx` (debug index), `name`. |
| `Labeldesc` | `lparser.h:132` | Label/goto descriptor: `name`, `pc`, `line`, `nactvar`, `close` flag. |
| `Labellist` | `lparser.h:142` | Growable array of `Labeldesc` entries with count and capacity. |
| `Dyndata` | `lparser.h:150` | Shared dynamic data: `actvar` (all active variable descriptors), `gt` (pending gotos), `label` (active labels). |
| `ConsControl` | `lparser.c:957` | Parser-local struct for table constructor parsing: tracks last value `v`, table descriptor `t`, record/array/pending counts, and batch limit `maxtostore`. |
| `LHS_assign` | `lparser.c:1520` | Chained list of left-hand-side variables in multi-assignment. |
| `MAXVARS` | `lparser.c:39` | Maximum local variables per function (200, must be < 250 due to bytecode format). |
| `MAX_CNST` | `lparser.c:973` | Maximum constructor elements (INT_MAX/2, capped by opcode field limits). |
| `hasmultret(k)` | `lparser.c:42` | Tests if expression kind `k` can produce multiple results (`VCALL` or `VVARARG`). |
| `eqstr(a,b)` | `lparser.c:47` | Pointer equality for strings (safe because the scanner unifies all strings). |

## Functions

### Error and Limit Helpers

#### `error_expected(LexState *ls, int token)` (static)

Raises a syntax error saying the given `token` was expected. Never returns.

#### `errorlimit(FuncState *fs, int limit, const char *what)` (static)

Raises "too many \<what\> (limit is \<limit\>)" with the function name or "main function". Reports errors with exact source location.

#### `luaY_checklimit(FuncState *fs, int v, int l, const char *what)`

Public check used by code generation: calls `errorlimit` when `v > l`.

#### `luaY_nvarstack(FuncState *fs)`

Returns the number of registers occupied by the function's local variables (computed by scanning backward through the variable list for the highest register-using variable).

### Token Helpers

#### `testnext(LexState *ls, int c)` (static)

If the next token is `c`, consumes it and returns 1; otherwise returns 0.

#### `check(LexState *ls, int c)` (static)

Asserts the next token is `c`; raises `error_expected` otherwise.

#### `checknext(LexState *ls, int c)` (static)

Asserts and consumes the next token.

#### `check_match(LexState *ls, int what, int who, int where)` (static)

Asserts and consumes `what`, raising a detailed error referencing the matching opening token `who` at line `where` if not found.

#### `str_checkname(LexState *ls)` (static)

Reads and consumes a `TK_NAME` token, returning its `TString`.

### Expression Descriptors

#### `init_exp(expdesc *e, expkind k, int i)` (static)

Initializes an `expdesc` with kind `k`, generic info `i`, and empty patch lists (`f = t = NO_JUMP`).

#### `codestring(expdesc *e, TString *s)` (static)

Creates a `VKSTR` expression descriptor for string constant `s`.

#### `codename(LexState *ls, expdesc *e)` (static)

Reads a `TK_NAME` and wraps it as a `VKSTR` expression.

### Variable Management

#### `registerlocalvar(LexState *ls, FuncState *fs, TString *varname)` (static)

Registers a local variable's debug info in the `Proto`'s `locvars` array. Returns the index and increments `ndebugvars`.

#### `new_varkind(LexState *ls, TString *name, lu_byte kind)` (static)

Creates a new variable entry in `Dyndata.actvar` with the given name and kind. Returns the variable's index relative to the current function.

#### `new_localvar(LexState *ls, TString *name)` (static)

Creates a new regular local variable (`VDKREG`).

#### `getlocalvardesc(FuncState *fs, int vidx)` (static)

Returns a pointer to the `Vardesc` for variable `vidx`.

#### `reglevel(FuncState *fs, int nvar)` (static)

Converts a variable index to its register number by finding the highest register-using variable below that level.

#### `localdebuginfo(FuncState *fs, int vidx)` (static)

Returns the debug `LocVar` entry for a given variable, or NULL if it has no debug info.

#### `init_var(FuncState *fs, expdesc *e, int vidx)` (static)

Creates a `VLOCAL` expression descriptor for a local variable.

#### `check_readonly(LexState *ls, expdesc *e)` (static)

Raises an error if the variable `e` is read-only (`<const>`). Also converts `VVARGIND` (indexed vararg table) to `VINDEXED` when needed.

#### `adjustlocalvars(LexState *ls, int nvars)` (static)

Activates scope for the last `nvars` declared variables, assigning them registers and debug indices. Checks `MAXVARS` limit.

#### `removevars(FuncState *fs, int tolevel)` (static)

Closes scope: deactivates variables above `tolevel`, setting their `endpc`.

### Upvalues

#### `searchupvalue(FuncState *fs, TString *name)` (static)

Searches the current function's upvalue list for one matching `name`.

#### `allocupvalue(FuncState *fs)` (static)

Grows the `Proto`'s upvalue array and returns a pointer to the new entry.

#### `newupvalue(FuncState *fs, TString *name, expdesc *v)` (static)

Adds an upvalue `name` to the current function, pointing either at a local variable (with `instack=1`) or at an upvalue of the enclosing function (with `instack=0`). Returns the upvalue index.

### Variable Resolution

#### `searchvar(FuncState *ls, TString *n, expdesc *var)` (static)

Searches active variables at the current function level. Handles local variables, compile-time constants, and global declarations (including collective `*` declarations). Returns the expression kind or -1 if not found.

#### `markupval(FuncState *fs, int level)` (static)

Marks the block where variable at `level` was defined as having escaping upvalues, triggering `OP_CLOSE` emission later.

#### `marktobeclosed(FuncState *fs)` (static)

Marks the current block as containing a to-be-closed variable.

#### `singlevaraux(FuncState *fs, TString *n, expdesc *var, int base)` (static)

Recursive variable lookup: searches locals, then existing upvalues, then encloses functions. Creates new upvalues as needed. If the variable is a vararg parameter (`VVARGVAR`), converts it to a regular local.

#### `buildglobal(LexState *ls, TString *varname, expdesc *var)` (static)

Resolves a global variable access: ensures `_ENV` is in a register, then creates the indexed expression `_ENV["varname"]`.

#### `buildvar(LexState *ls, TString *varname, expdesc *var)` (static)

Full variable resolution: calls `singlevaraux`, then `buildglobal` for unresolved globals. Handles read-only globals and undeclared variable errors.

#### `singlevar(LexState *ls, expdesc *var)` (static)

Convenience: reads a `TK_NAME` and resolves it via `buildvar`.

### Assignment

#### `adjust_assign(LexState *ls, int nvars, int nexps, expdesc *e)` (static)

Adjusts the number of expression results to match `nvars` targets. Handles multi-return expressions, padding with nil, and register reservation.

#### `check_conflict(LexState *ls, struct LHS_assign *lh, expdesc *v)` (static)

Detects conflicts in multiple assignments where an upvalue/local is used as a table index in a previous LHS, requiring a temporary copy.

#### `storevartop(FuncState *fs, expdesc *var)` (static)

Stores the top-of-stack register into `var`, freeing that register.

#### `restassign(LexState *ls, struct LHS_assign *lh, int nvars)` (static)

Parses the right side of a multi-assignment: comma-separated suffixed expressions followed by `= explist`. Recursively builds the LHS chain.

### Blocks and Scopes

#### `enterblock(FuncState *fs, BlockCnt *bl, lu_byte isloop)` (static)

Pushes a new scope block: records label/goto boundaries, active variable level, loop flag; inherits `insidetbc` from the enclosing block.

#### `leaveblock(FuncState *fs)` (static)

Pops the current scope block: emits `OP_CLOSE` if upvalues escape, frees registers, removes locals, fixes pending breaks, and resolves gotos. Reports undefined gotos at the outermost block.

#### `block_follow(LexState *ls, int withuntil)` (static)

Tests whether the current token terminates a block: `else`, `elseif`, `end`, `EOS`, and optionally `until`.

#### `statlist(LexState *ls)` (static)

Parses a statement list until a block-follow token. `return` must be the last statement.

### Labels and Gotos

#### `findlabel(LexState *ls, TString *name, int ilb)` (static)

Searches for an active label with the given name starting at index `ilb`.

#### `newlabelentry(LexState *ls, Labellist *l, TString *name, int line, int pc)` (static)

Adds a new label or goto entry to the given list.

#### `newgotoentry(LexState *ls, TString *name, int line)` (static)

Creates a goto: emits a jump instruction followed by a dead `OP_CLOSE` placeholder, then adds the goto entry.

#### `createlabel(LexState *ls, TString *name, int line, int last)` (static)

Creates a label at the current pc. If the label is the last no-op in its block, adjusts `nactvar` to assume locals are out of scope.

#### `closegoto(LexState *ls, int g, Labeldesc *label, int bup)` (static)

Resolves a pending goto against a label. If the goto enters a variable's scope, raises an error. If upvalues need closing, inserts/swaps an `OP_CLOSE` instruction before the jump.

#### `solvegotos(FuncState *fs, BlockCnt *bl)` (static)

Traverses pending gotos of a finishing block, matching them against labels. Unmatched gotos are "exported" to the outer block with adjusted `nactvar`.

#### `undefgoto(LexState *ls, Labeldesc *gt)` (static)

Raises an error for a goto with no visible label.

### Functions

#### `addprototype(LexState *ls)` (static)

Creates a new `Proto` and adds it to the current function's `p` array.

#### `codeclosure(LexState *ls, expdesc *v)` (static)

Emits `OP_CLOSURE` in the parent function. The result is a `VRELOC` expression placed at the last available register (for GC safety).

#### `open_func(LexState *ls, FuncState *fs, BlockCnt *bl)` (static)

Initializes a fresh `FuncState` for a new function: links to enclosing state, resets all counters, anchors the constant cache table, and enters the first block.

#### `close_func(LexState *ls)` (static)

Finalizes a function: emits the implicit return, runs peephole optimization (`luaK_finish`), shrinks all arrays, drops the constant cache anchor.

#### `setvararg(FuncState *fs)` (static)

Sets the `PF_VAHID` flag and emits `OP_VARARGPREP` for vararg functions.

#### `parlist(LexState *ls)` (static)

Parses the parameter list: names become locals, `...` optionally with a named vararg parameter. Sets `numparams` and marks the function as vararg.

#### `body(LexState *ls, expdesc *e, int ismethod, int line)` (static)

Parses a full function body: creates a new prototype, opens a `FuncState`, adds `self` for methods, parses parameters and body, emits the closure instruction.

### Expressions

#### `primaryexp(LexState *ls, expdesc *v)` (static)

Parses primary expressions: `NAME` or `(expr)`. Parenthesized expressions are discharged to plain values.

#### `suffixedexp(LexState *ls, expdesc *v)` (static)

Parses suffixed expressions: primary followed by any number of `.NAME`, `[expr]`, `:NAME args`, or function-call suffixes.

#### `simpleexp(LexState *ls, expdesc *v)` (static)

Parses atomic expressions: literals (float, int, string, nil, true, false), `...` (vararg), constructors, function bodies, or suffixed expressions.

#### `subexpr(LexState *ls, expdesc *v, int limit)` (static)

Pratt-style expression parser: handles unary operators recursively and binary operators by priority. Returns the first unprocessed binary operator.

#### `expr(LexState *ls, expdesc *v)` (static)

Entry point for expression parsing: calls `subexpr` at priority 0.

#### `getunopr(int op)` (static)

Maps a token to its unary operator (`OPR_MINUS`, `OPR_BNOT`, `OPR_NOT`, `OPR_LEN`, or `OPR_NOUNOPR`).

#### `getbinopr(int op)` (static)

Maps a token to its binary operator (`OPR_ADD` through `OPR_OR`, or `OPR_NOBINOPR`).

#### `priority[]` (static, line 1433)

Priority table for all binary operators with left and right binding powers. `^` and `..` are right-associative (left > right). `and` and `or` have the lowest priority.

#### `explist(LexState *ls, expdesc *v)` (static)

Parses a comma-separated expression list, closing each value into the next register. Returns the count.

#### `funcargs(LexState *ls, expdesc *f)` (static)

Parses function call arguments: `(explist)`, constructor, or string literal. Emits `OP_CALL` and rewrites `f` as a `VCALL` expression.

### Table Constructors

#### `recfield(LexState *ls, ConsControl *cc)` (static)

Parses a record field: `NAME = expr` or `[expr] = expr`. Stores the value into the table via `luaK_storevar`.

#### `listfield(LexState *ls, ConsControl *cc)` (static)

Parses a list (array) field: a plain expression. Increments the pending store count.

#### `field(LexState *ls, ConsControl *cc)` (static)

Dispatches a constructor field: looks ahead to disambiguate `NAME` tokens (record if followed by `=`, list otherwise); `[` always starts a record field.

#### `closelistfield(FuncState *fs, ConsControl *cc)` (static)

Flushes pending array elements when the batch limit is reached.

#### `lastlistfield(FuncState *fs, ConsControl *cc)` (static)

Emits the final `SETLIST` for the array part of a constructor.

#### `maxtostore(FuncState *fs)` (static)

Computes the batch limit for `SETLIST` based on available registers.

#### `constructor(LexState *ls, expdesc *t)` (static)

Parses a full table constructor: emits `OP_NEWTABLE`, fills array/record parts via `field`, patches table sizes.

### Statements

#### `statement(LexState *ls)` (static)

The top-level statement dispatcher. Switches on the current token to call the appropriate statement parser: `;` (empty), `if`, `while`, `do`, `for`, `repeat`, `function`, `local`, `global`, `::` (label), `return`, `break`, `goto`, or expression statements (assignments and calls). Resets `freereg` after each statement.

#### `ifstat(LexState *ls, int line)` (static)

Parses an `if`/`elseif`/`else`/`end` chain.

#### `whilestat(LexState *ls, int line)` (static)

Parses a `while cond do block end` loop.

#### `repeatstat(LexState *ls, int line)` (static)

Parses a `repeat block until cond` loop with two nested blocks so the condition sees the body's locals.

#### `forstat(LexState *ls, int line)` (static)

Parses `for` statements, dispatching to numeric or generic variants.

#### `fornum(LexState *ls, TString *varname, int line)` (static)

Parses a numeric `for`: creates three internal state registers (initial, limit, step) plus the control variable.

#### `forlist(LexState *ls, TString *indexname)` (static)

Parses a generic `for`: creates iterator/state/closing/control variables plus user names; marks the closing variable as to-be-closed.

#### `forbody(LexState *ls, int base, int line, int nvars, int isgen)` (static)

Emits the for-loop body: `OP_FORPREP`/`OP_TFORPREP`, block, `OP_FORLOOP`/`OP_TFORLOOP` with patched jumps.

#### `retstat(LexState *ls)` (static)

Parses `return [explist]`. A single trailing call becomes a tail call (`OP_TAILCALL`) unless inside a to-be-closed scope.

#### `exprstat(LexState *ls)` (static)

Parses expression statements: either an assignment or a bare call (with results discarded).

#### `localfunc(LexState *ls)` (static)

Parses `local function NAME body`: creates the local before the body so recursive references work.

#### `localstat(LexState *ls)` (static)

Parses `local NAME [attrib] [= explist]` with support for `<const>` (compile-time constant folding) and `<close>` (to-be-closed variables).

#### `globalstatfunc(LexState *ls, int line)` (static)

Dispatches `global` to either `globalfunc` or `globalstat`.

#### `globalfunc(LexState *ls, int line)` (static)

Parses `global function NAME body`: declares the global, compiles the body, stores the closure, with a nil-check to prevent redeclaration.

#### `globalstat(LexState *ls)` (static)

Parses global variable declarations (name list or `*` catch-all) with optional attributes.

#### `body(LexState *ls, expdesc *e, int ismethod, int line)` (static)

Parses a function body: creates a new `Proto`, opens a `FuncState`, optionally adds `self`, parses parameters and statements, emits the closure.

#### `gotostat(LexState *ls, int line)` (static)

Parses `goto NAME`.

#### `breakstat(LexState *ls, int line)` (static)

Parses `break`: validates it is inside a loop, marks the block as having pending breaks, creates a goto entry for the synthetic `break` label.

#### `labelstat(LexState *ls, TString *name, int line)` (static)

Parses `::NAME::`: skips intervening no-ops, checks for repeated labels, creates the label entry.

### Entry Point

#### `luaY_parser(lua_State *L, ZIO *z, Table *anchor, Mbuffer *buff, Dyndata *dyd, const char *name, int firstchar)`

The top-level compilation entry point. Creates the main `LClosure` with one upvalue (`_ENV`), initializes `LexState` and `FuncState`, calls `mainfunc` to parse the entire chunk, and returns the compiled closure. Asserts all dynamic data structures are empty after parsing.
