# lstrlib.c — Standard string library and pattern matching engine

> **AI-Generated Documentation**

## Overview

`lstrlib.c` is the largest standard library file (2056 lines), implementing the `string` module and its powerful pattern-matching engine. It provides functions for basic string manipulation (`byte`, `char`, `sub`, `rep`, `reverse`, `lower`, `upper`, `len`, `dump`), search and substitution (`find`, `match`, `gmatch`, `gsub`), formatted output (`format`), and binary serialization (`pack`, `unpack`, `packsize`).

The pattern matcher is a recursive backtracking engine (`match`) with a fixed recursion depth limit (`MAXCCALLS = 200`) to prevent stack overflow. It supports Lua's full pattern syntax: character classes (`%a`, `%d`, `%s`, etc.), bracket sets (`[a-z]`), anchors (`^`, `$`), captures (`()`), backreferences (`%n`), balanced strings (`%bxy`), frontiers (`%f[...]`), and quantifiers (`*`, `+`, `-`, `?`). The `MatchState` struct tracks capture levels and source/pattern boundaries. A `goto init` pattern replaces tail calls for efficiency.

The `string` library also installs a metatable on strings with arithmetic metamethods (`__add`, `__sub`, `__mul`, `__mod`, `__pow`, `__div`, `__idiv`, `__unm`). These attempt numeric coercion on both operands; if both are numbers, the operation proceeds directly; otherwise, the fallback `trymt` delegates to the second operand's metamethod.

The binary serialization subsystem (`pack`/`unpack`) supports explicit endianness control (`<`, `>`, `=`), alignment (`!n`, `X`), and a rich set of type options for integers of various sizes, floating-point types, fixed-length strings, length-prefixed strings, and zero-terminated strings.

## Functions

### str_len(s)

Implements `string.len`. Returns the byte length of the string argument.

### posrelatI(pos, len)

Internal helper: converts a relative 1-based string position to an absolute 0-based index. Negative positions count from the end; zero is treated as 1. The result is clipped to `[0, len]`.

### getendpos(L, arg, def, len)

Internal helper: reads an optional end position from a stack argument with a default value. Negative values count from the end. Result is clipped to `[0, len]`.

### str_sub(s, i [, j])

Implements `string.sub`. Returns the substring from position `i` to `j` (default: -1, the end). Supports negative indices. Returns `""` when the interval is empty.

### str_reverse(s)

Implements `string.reverse`. Returns the string with its bytes reversed. Uses a `luaL_Buffer` for efficient allocation.

### str_lower(s)

Implements `string.lower`. Maps each byte to its lowercase equivalent via C's `tolower`. Locale-dependent.

### str_upper(s)

Implements `string.upper`. Maps each byte to its uppercase equivalent via C's `toupper`. Locale-dependent.

### str_rep(s, n [, sep])

Implements `string.rep`. Concatenates `n` copies of `s` with the separator `sep` between them. Raises an error if the result would exceed size limits. The total length is computed as `n * (len + lsep) - lsep`.

### str_byte(s [, i [, j]])

Implements `string.byte`. Returns the numerical byte values of characters in the range `[i, j]` (default: `i = j = 1`). Returns nothing for an empty interval. Each result is a separate return value.

### str_char(...)

Implements `string.char`. Builds a string from the given byte values (each must be in `[0, 255]`). Returns one string.

### writer(L, b, size, ud)

Internal callback for `lua_dump`. Accumulates binary chunks into a `luaL_Buffer` held in a `str_Writer` struct. On the final call (`b == NULL`), pushes the complete result onto the stack.

### str_dump(f [, strip])

Implements `string.dump`. Serializes a Lua function (not a C function) into its binary representation. When `strip` is true, debug information is omitted. Returns the binary string.

### tonum(L, arg)

Internal helper for string metamethods: attempts to coerce the stack value at `arg` to a number. Returns 1 (and pushes the number) on success, 0 on failure.

### trymt(L, mtkey, opname)

Internal helper for arithmetic metamethods: if the second operand is not a string and has the requested metamethod, calls it. Otherwise raises an "attempt to perform arithmetic" error.

### arith(L, op, mtname)

Internal shared implementation for all arithmetic metamethods: tries numeric coercion on both operands; if either fails, delegates to `trymt`.

### arith_add / arith_sub / arith_mul / arith_mod / arith_pow / arith_div / arith_idiv / arith_unm

Metamethod implementations for the corresponding arithmetic operators on strings. Each delegates to the shared `arith` helper.

### check_capture(ms, l)

Internal pattern-matching helper: validates a `%n` backreference index, ensuring it refers to an already-closed capture.

### capture_to_close(ms)

Internal pattern-matching helper: finds the index of the innermost still-open capture (the one whose closing `)` is being matched).

### classend(ms, p)

Internal pattern-matching helper: advances past one pattern item (a plain character, an escaped character, or a `[...]` set) and returns a pointer to what follows — the position where an optional repetition suffix would be.

### match_class(c, cl)

Internal pattern-matching helper: tests a character `c` against a `%class` letter. Uppercase class letters negate the result (e.g. `%A` = "not alpha"). Supports `a`, `c`, `d`, `g`, `l`, `p`, `s`, `u`, `w`, `x`, `z`.

### matchbracketclass(c, p, ec)

Internal pattern-matching helper: tests character `c` against a `[...]` bracket set spanning `p..ec`. Supports character ranges (`a-z`), escaped classes (`%d`), and negation (`^` as first character).

### singlematch(ms, s, p, ep)

Internal pattern-matching helper: tests whether one subject character matches the pattern item at `p`. Handles `.` (any), `%class`, `[...]`, and plain characters.

### matchbalance(ms, s, p)

Internal pattern-matching helper: implements `%bxy` — matches a balanced run starting with `x` and ending with `y`, tracking nesting depth. Returns the position after the closing character, or NULL on failure.

### max_expand(ms, s, p, ep)

Internal pattern-matching helper: handles greedy quantifiers `*` and `+`. Consumes as many repetitions as possible, then backtracks one at a time until the rest of the pattern matches.

### min_expand(ms, s, p, ep)

Internal pattern-matching helper: handles the lazy quantifier `-`. Tries matching the rest of the pattern with zero repetitions first, then grows one character at a time only when the rest fails.

### start_capture(ms, s, p, what)

Internal pattern-matching helper: opens a capture at `(`. Records the start position and increments the capture level. Undoes the capture if the rest of the pattern fails.

### end_capture(ms, s, p)

Internal pattern-matching helper: closes the innermost capture at `)`, recording its length. Reverts to `CAP_UNFINISHED` if the remainder of the pattern fails.

### match_capture(ms, s, l)

Internal pattern-matching helper: implements `%n` backreferences. Matches whatever text the n-th capture previously captured (exact byte comparison via `memcmp`).

### match(ms, s, p)

Core recursive pattern-matching engine. Dispatches on the current pattern character: captures (`(`, `)`), anchors (`^`, `$`), balanced strings (`%b`), frontiers (`%f`), backreferences (`%n`), character classes with quantifiers (`*`, `+`, `-`, `?`). Uses `goto init` to convert tail calls into loops. Recursion depth is bounded by `MAXCCALLS` (200).

### lmemfind(s1, l1, s2, l2)

Plain substring search: uses `memchr` to locate candidate first bytes, then `memcmp` to confirm. Returns the position of the first match, or NULL. Efficient for patterns with no special characters.

### get_onecapture(ms, i, s, e, cap)

Internal helper: retrieves capture `i` (where `i == 0` is the whole match). For string captures, sets `*cap` and returns the length. For position captures, pushes the 1-based position as an integer and returns `CAP_POSITION`.

### push_onecapture(ms, i, s, e)

Internal helper: pushes capture `i` onto the stack as a string (position captures are already pushed by `get_onecapture`).

### push_captures(ms, s, e)

Internal helper: pushes all captures of a match. With no captures, pushes the whole matched text as the single result.

### nospecials(p, l)

Fast path: returns true when the pattern contains no magic characters (`^$*+?.([%-`), allowing a plain literal search.

### prepstate(ms, L, s, ls, p, lp)

Internal helper: initializes the fixed fields of a `MatchState` (source and pattern boundaries).

### reprepstate(ms)

Internal helper: resets per-attempt fields — the recursion budget (`MAXCCALLS`) and the capture counter.

### str_find_aux(L, find)

Shared engine for `string.find` and `string.match`. Honours a leading `^` anchor. Uses `lmemfind` for plain searches (explicit flag or no special characters). Otherwise iterates `match` at every position. `find` returns start/end positions plus captures; `match` returns just captures.

### str_find(s, p [, init [, plain]])

Implements `string.find`. Returns the start and end positions of the first match, plus any captures. With `plain` true (or a no-specials pattern), does a literal substring search.

### str_match(s, p [, init])

Implements `string.match`. Returns the captures of the first match (or the whole match if there are no captures). Returns nil on no match.

### gmatch_aux(L)

Iterator step for `string.gmatch`. Finds the next match strictly after the end of the previous one, preventing zero-length matches from repeating in place. Returns the captures.

### gmatch(s, p [, init])

Implements `string.gmatch`. Creates and returns an iterator function (closure) that yields all matches and their captures over the string. The iterator holds a `GMatchState` as a userdata upvalue containing the source, pattern, and last-match position.

### add_s(ms, b, s, e)

Internal helper for `gsub`: expands a replacement template. `%%` produces `%`, `%0` the whole match, `%n` capture `n` (position captures become numbers). Other escape sequences raise an error.

### add_value(ms, b, s, e, tr)

Internal helper for `gsub`: appends one replacement. A function is called with all captures. A table is indexed by the first capture. A string/number template is expanded via `add_s`. `nil`/`false` results keep the original text.

### str_gsub(s, p, repl [, n])

Implements `string.gsub`. Scans for matches (anchored when `p` starts with `^`), replacing each via `add_value`. Returns the new string (or original if unchanged) and the substitution count. The optional `n` limits the maximum number of replacements.

### addquoted(b, s, len)

Internal helper for `format`: serializes a string as a quoted Lua literal, escaping quotes, backslashes, newlines, and control characters (`\ddd` form when the next character is a digit).

### quotefloat(L, buff, n)

Internal helper: serializes a float so Lua can parse it back. Uses hex format for normal numbers (preserving precision). Inf becomes `1e9999`, -inf becomes `-1e9999`, and NaN becomes `(0/0)`.

### addliteral(L, b, arg)

Internal helper for `%q` format: appends a literal form of the argument. Strings are quoted, floats use `quotefloat`, integers use `%d` or hex, nil/boolean use `tostring`.

### get2digits(s)

Internal helper: skips at most two digit characters. Used when parsing width and precision fields (limited to two digits each).

### checkformat(L, form, flags, precision)

Validates a format specification: the flags, width, and precision must match the allowed set for the conversion specifier. Raises on invalid specifications.

### getformat(L, strfrmt, form)

Extracts the current format item (`%` + flags + width + precision + specifier) into `form` and returns a pointer to the last character.

### addlenmod(form, lenmod)

Inserts a length modifier string (e.g. `"l"` or the platform's `LUA_INTEGER_FRMLEN`) right before the final conversion character in the format string.

### str_format(fmt, ...)

Implements `string.format`. Walks the format string, copying plain text and dispatching each `%` item to the matching C formatter. Validates each specification and argument type. Handles `c`, `d`, `i`, `u`, `o`, `x`, `X`, `a`, `A`, `f`, `e`, `E`, `g`, `G`, `p`, `q`, and `s` conversions.

### digit(c)

Internal helper: returns true for ASCII decimal digits.

### getnum(fmt, df)

Internal helper: reads an optional decimal numeral from the format string, returning the default `df` when absent. Used by pack/unpack for size fields.

### getnumlimit(h, fmt, df)

Internal helper: reads a size numeral and rejects values outside `[1, MAXINTSIZE]`.

### initheader(L, h)

Internal helper: initializes the pack/unpack `Header` struct with native endianness and a default maximum alignment of 1.

### getoption(h, fmt, size)

Internal helper: reads and classifies one pack/unpack format option. Returns a `KOption` enum (`Kint`, `Kuint`, `Kfloat`, `Knumber`, `Kdouble`, `Kchar`, `Kstring`, `Kzstr`, `Kpadding`, `Kpaddalign`, `Knop`) and sets `*size`. Handles endianness controls (`<`, `>`, `=`) and alignment limits (`!`).

### getdetails(h, totalsize, fmt, psize, ntoalign)

Internal helper: completes `getoption` by computing alignment padding. Enforces the `!` maximum-alignment limit. `X` reads its alignment from the next format option.

### packint(b, n, islittle, size, neg)

Internal helper: writes an integer as `size` bytes in the requested endianness into a buffer. When `size` exceeds `sizeof(lua_Integer)`, negative values are sign-extended.

### copywithendian(dest, src, size, islittle)

Internal helper: copies `size` bytes, reversing byte order when the requested endianness differs from native. Uses `memcpy` when they agree.

### str_pack(fmt, ...)

Implements `string.pack`. Serializes arguments according to the format string into a single binary string. Handles alignment, endianness, and all type options. Returns the packed string.

### str_packsize(fmt)

Implements `string.packsize`. Returns the fixed byte size required by the format string. Rejects variable-length options (`s`, `z`).

### unpackint(L, str, islittle, size, issigned)

Internal helper: reads an integer of `size` bytes with the given endianness. Performs sign extension for short signed types. Validates that extra bytes (for sizes larger than `lua_Integer`) do not cause overflow.

### str_unpack(fmt, s [, pos])

Implements `string.unpack`. Decodes values from string `s` starting at position `pos` (default: 1) according to the format. Returns all decoded values plus the next position.

### createmetatable(L)

Internal helper: installs the string metatable, setting `__index` to the string library table. This enables method-style calls like `s:len()`.

### luaopen_string(L)

Opens the `string` library, creates the function table, and installs the string metatable via `createmetatable`. Returns the library table.
