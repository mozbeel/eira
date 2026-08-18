# lobject.c — Generic operations over Lua objects

> **AI-Generated Documentation**

## Overview

`lobject.c` is the foundation layer for value manipulation in Eira's Lua 5.5 dialect. It provides arithmetic dispatch (both raw and metamethod-aware), number parsing (decimal and hexadecimal), number-to-string formatting, the `luaO_pushvfstring` printf-like message builder, and source-chunk identification for error messages.

The file operates heavily on `TValue` — Eira's tagged-value union — and relies on the `intop` / `luai_num*` macro families so that platform-specific overflow and rounding behavior can be injected at compile time. Number conversion goes through a two-pass strategy: try integer first (via `l_str2int`), then float (via `l_str2d`), preferring the integer result.

The parameter-byte encoding (`luaO_codeparam` / `luaO_applyparam`) is a compact IEEE-754-inspired format used by several VM operations (e.g., table resize hints) to transmit a percentage as a single byte with excess-7 exponent and 4-bit mantissa.

Finally, the `BuffFS` subsystem implements a growable buffer that starts on the C stack and dynamically allocates only when the formatted string exceeds `BUFVFS` (≈ `LUA_IDSIZE + LUA_N2SBUFFSZ + 95` bytes).

## Key Types / Macros

| Identifier | Purpose |
|---|---|
| `TValue` | Tagged value: a `Value` union plus a `lu_byte tt_` type tag. Every Lua value flows through this. |
| `BuffFS` | Stack-local formatting buffer with dynamic overflow for `luaO_pushvfstring`. |
| `LUA_N2SBUFFSZ` | Size of the buffer used for number-to-string conversion. Must hold both integer and float formats. |
| `UTF8BUFFSZ` (8) | Maximum bytes needed for `luaO_utf8esc` to encode a single Unicode codepoint. |
| `MAXBY10` / `MAXLASTD` | Derived constants used to detect integer overflow during `l_str2int`. |

## Functions

### `luaO_ceillog2(unsigned int x)`

Returns `ceil(log₂(x))` — the smallest `n` such that `x ≤ 2ⁿ`. Uses a 256-entry lookup table and processes the argument in 8-bit chunks for efficiency. The result feeds into `luaO_codeparam`'s exponent calculation.

### `luaO_codeparam(unsigned int p)`

Encodes a percentage `p` into a single floating-point byte with format `(eeee xxxx)`: a 4-bit excess-7 exponent and 4-bit mantissa. Normalizes when possible (implicit leading 1, mimicking IEEE 754), returns `0xFF` on overflow, and handles subnormals when the mantissa fits in 4 bits without an exponent.

### `luaO_applyparam(lu_byte p, l_mem x)`

Decodes a parameter byte (produced by `luaO_codeparam`) and multiplies `x` by that percentage. For positive exponents it checks overflow before shifting; for negative exponents it prefers to multiply first (preserving precision) and falls back to shift-first if the multiplication would overflow. Returns `MAX_LMEM` on saturation.

### `intarith(lua_State *L, int op, lua_Integer v1, lua_Integer v2)` (static)

Dispatches raw integer arithmetic: `+`, `-`, `*`, `%`, `//`, `&`, `|`, `^`, `<<`, `>>`, unary `-`, and `~`. Binary results wrap via `intop` macros. Modulo and integer division delegate to `luaV_mod` / `luaV_idiv` for correct rounding.

### `numarith(lua_State *L, int op, lua_Number v1, lua_Number v2)` (static)

Dispatches raw floating-point arithmetic through the configurable `luai_num*` macros. Covers `+`, `-`, `*`, `/`, `^`, `//`, unary `-`, and `%` (via `luaV_modf`).

### `luaO_rawarith(lua_State *L, int op, const TValue *p1, const TValue *p2, TValue *res)`

Performs a metamethod-free arithmetic operation. Bitwise ops require both operands to be integers; `/` and `^` coerce to floats; other ops try integer first, then float. Returns 1 on success with the result in `res`, or 0 if the types are incompatible.

### `luaO_arith(lua_State *L, int op, const TValue *p1, const TValue *p2, StkId res)`

Full arithmetic: tries `luaO_rawarith` first, then falls back to the binary metamethod via `luaT_trybinTM`. The metamethod event is computed as `(op - LUA_OPADD) + TM_ADD`.

### `luaO_hexavalue(int c)`

Maps an ASCII hex digit (`0-9`, `a-f`, `A-F`) to its numeric value 0–15. Case-insensitive.

### `lua_strx2number(const char *s, char **endptr)` (static)

C99-style `strtod` for hexadecimal floating-point literals (`0x…`). Accumulates base-16 digits (capping significant digits at `MAXSIGDIG = 30`), tracks a decimal-point exponent correction, then combines with an optional `p`-exponent via `ldexp`.

### `l_str2dloc(const char *s, lua_Number *result, int mode)` (static)

Converts a string to a `lua_Number` using either `lua_strx2number` (mode `'x'`) or the system `strtod`. Only accepts strings that end cleanly at `'\0'` after optional trailing whitespace.

### `l_str2d(const char *s, lua_Number *result)` (static)

Locale-tolerant float conversion: if the default parse fails and the string contains a `'.'`, it retries on a copy with `'.'` replaced by the locale's radix character. Rejects `'inf'` and `'nan'`.

### `l_str2int(const char *s, lua_Integer *result)` (static)

Parses a decimal or `0x`-prefixed hex integer literal, detecting overflow against `LUA_MAXINTEGER`. Returns the position past the numeral or `NULL` on failure/overflow.

### `luaO_str2num(const char *s, TValue *o)`

Converts a string to a number `TValue`, preferring integer form (`l_str2int`) over float (`l_str2d`). Returns the length of the consumed prefix + 1 on success, or 0 on failure.

### `luaO_utf8esc(char *buff, l_uint32 x)`

Writes the UTF-8 encoding of codepoint `x` **backwards** into `buff` (which must be `UTF8BUFFSZ` bytes). Returns the number of bytes written (1–4). The caller reads from `buff + UTF8BUFFSZ - n`.

### `tostringbuffFloat(lua_Number n, char *buff)` (static)

Formats a float using `LUA_NUMBER_FMT`. If re-parsing the result gives a different value, it retries with `LUA_NUMBER_FMT_N` (more digits). Appends `".0"` when the numeral looks like an integer to prevent silent round-trip loss.

### `luaO_tostringbuff(const TValue *obj, char *buff)`

Renders any number (integer or float) into `buff` and returns the string length. Integers use `lua_integer2str`; floats go through `tostringbuffFloat`.

### `luaO_tostring(lua_State *L, TValue *obj)`

Converts the number at `obj` into an interned Lua string, replacing the value in-place via `luaS_newlstr`.

### `initbuff(lua_State *L, BuffFS *buff)` (static)

Initializes a `BuffFS` to use its inline `space` buffer (`BUFVFS` bytes).

### `pushbuff(lua_State *L, void *ud)` (static)

Final step of `luaO_pushvfstring`: pushes the accumulated buffer as a `TString` on the stack. Appends `"…"` on overflow; raises `LUA_ERRMEM` on allocation failure.

### `clearbuff(BuffFS *buff)` (static)

Calls `pushbuff` under `luaD_rawrunprotected`, frees any dynamic buffer, and returns a pointer to the resulting string (or `NULL` on error).

### `addstr2buff(BuffFS *buff, const char *str, size_t slen)` (static)

Appends `slen` bytes to the buffer, growing/reallocating from the static space to heap when needed. Sets `buff->err` to 1 (memory) or 2 (overflow) on failure.

### `addnum2buff(BuffFS *buff, TValue *num)` (static)

Converts a number to text via `luaO_tostringbuff` and appends it to the message buffer.

### `luaO_pushvfstring(lua_State *L, const char *fmt, va_list argp)`

Printf-like formatter supporting `%s`, `%c`, `%d`, `%I` (lua_Integer), `%f`, `%p`, `%U` (UTF-8 codepoint), and `%%`. Builds the result in a `BuffFS`, then pushes it as a Lua string. May raise on memory error.

### `luaO_pushfstring(lua_State *L, const char *fmt, ...)`

Varargs wrapper around `luaO_pushvfstring`. Throws `LUA_ERRMEM` if the message construction failed.

### `luaO_chunkid(char *out, const char *source, size_t srclen)`

Formats a chunk source identifier for error messages into `out` (max `LUA_IDSIZE`). Handles three cases: `=literal` (verbatim), `@file` (truncated with `"…"`), and string sources (wrapped in `[string "..."]`, cut at the first newline).
