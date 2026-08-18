# lutf8lib.c — Standard library for UTF-8 manipulation

> **AI-Generated Documentation**

## Overview

`lutf8lib.c` implements the `utf8` standard library, providing Eira scripts with functions for counting, iterating, and converting UTF-8 encoded strings. The library is registered via `luaopen_utf8`, which also populates the `utf8.charpattern` placeholder with a Lua pattern that matches a single UTF-8 character (`[\0-\x7F\xC2-\xFD][\x80-\xBF]*`).

The core of the library is the `utf8_decode` function, a stateless decoder that reads one UTF-8 sequence from a byte buffer and produces a Unicode code point (as `l_uint32`). It validates continuation bytes, checks minimum values per sequence length to reject overlong encodings, and in strict mode also rejects surrogates (`U+D800..U+DFFF`) and code points above `U+10FFFF`. A "lax" mode (`lax = 1`) relaxes these checks, allowing the iterator to skip over malformed bytes.

The file is 320 lines, making it one of the smallest standard library modules. All functions that accept byte positions support relative (negative) indices via `u_posrelat`.

## Functions

### u_posrelat(pos, len)

Internal helper: translates a relative string position to an absolute 0-based byte offset. Negative positions count from the end; values that would go before the start are clipped to 0. Used by `utflen`, `codepoint`, and `byteoffset`.

### utf8_decode(s, val, strict)

Core UTF-8 decoder. Reads one UTF-8 sequence starting at `s` and writes the decoded code point to `*val` (if non-NULL). Returns a pointer to the byte after the sequence, or `NULL` on invalid input. In strict mode (`strict = 1`), rejects overlong encodings, surrogates (`U+D800..U+DFFF`), and code points above `U+10FFFF`. The `limits[]` table stores the minimum valid code point for each sequence length (1–5 continuation bytes), enforcing overlong rejection. Supports up to 6-byte sequences (though valid Unicode only requires 4).

### utflen(s [, i [, j [, lax]]])

Implements `utf8.len`. Counts the number of UTF-8 characters whose starting bytes fall in the byte range `[i, j]` (default: `i = 1`, `j = -1`, the full string). Returns the count as an integer on success. On malformed input (in strict mode), returns `nil` (fail) and the 1-based byte position of the first error. The optional `lax` parameter, when true, disables strict validation.

### codepoint(s [, i [, j [, lax]]])

Implements `utf8.codepoint`. Pushes the Unicode code point (as an integer) of every character whose starting byte falls in `[i, j]` (default: `i = 1`, `j = i`, a single character). Each code point is a separate return value. Raises an error on malformed input in strict mode.

### pushutfchar(L, arg)

Internal helper: reads an integer code point from stack argument `arg`, validates it is ≤ `MAXUTF` (`0x7FFFFFFF`), and pushes its UTF-8 encoding as a string using `lua_pushfstring` with the `%U` format.

### utfchar(...)

Implements `utf8.char`. Concatenates the UTF-8 encoding of each argument (each must be a valid code point ≤ `MAXUTF`) into a single string. Optimizes the common single-character case.

### byteoffset(s, n, [i])

Implements `utf8.offset`. Returns the byte positions (1-based start and end) where the `n`-th character counting from position `i` starts and ends. When `n == 0`, returns the boundaries of the character that contains position `i` (walking backward over continuation bytes to find the start). Positive `n` counts forward, negative `n` counts backward. Returns `nil` when the requested character does not exist. Raises if `i` lands on a continuation byte.

### iter_aux(L, strict)

Internal helper: one step of the `utf8.codes` iterator. Skips continuation bytes from the current position, decodes one UTF-8 sequence, and returns the 1-based byte position and the code point. Returns 0 (no more values) when the string is exhausted. In strict mode, raises on malformed input.

### iter_auxstrict(L)

Strict variant of the codes iterator: calls `iter_aux` with `strict = 1`, rejecting malformed UTF-8.

### iter_auxlax(L)

Lax variant of the codes iterator: calls `iter_aux` with `strict = 0`, allowing the iterator to skip over invalid bytes.

### iter_codes(s [, lax])

Implements `utf8.codes`. Returns the standard iterator triple `(function, string, 0)` for iterating over the code points of `s`. The iterator function is `iter_auxstrict` or `iter_auxlax` depending on the `lax` flag. Validates that `s` does not start with a continuation byte.

### luaopen_utf8(L)

Opens the `utf8` library. Creates the function table, then sets the `charpattern` field to the pre-defined pattern `UTF8PATT` (`[\0-\x7F\xC2-\xFD][\x80-\xBF]*`), which matches a single non-ASCII UTF-8 character. Returns the library table.
