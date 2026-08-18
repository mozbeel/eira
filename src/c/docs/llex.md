# llex.c — Lexical Analyzer / Tokenizer

> **AI-Generated Documentation**

## Overview

`llex.c` implements the **lexical analyzer** (scanner/tokenizer) for the Eira compiler. It transforms the raw character stream (provided via a `ZIO` buffered reader) into a stream of **tokens** consumed on demand by the recursive-descent parser (`lparser.c`). The lexer is a single-pass, demand-driven scanner: the parser calls `luaX_next()` whenever it needs the next token, and the lexer scans exactly one token per call.

The file is the first stage of the compilation pipeline (lexing → parsing → code generation → bytecode → VM). Tokens are represented by the `Token` struct, which pairs an integer token tag (from the `RESERVED` enum in `llex.h`) with a `SemInfo` union carrying semantic information — a `lua_Integer`, `lua_Number`, or `TString *` depending on the token type.

Beyond simple tokenization, the lexer also handles **string interning and anchoring**: every string created during lexing is pinned in a scanner table (`LexState.h`) so the garbage collector cannot collect it mid-compilation. Reserved words are created once during `luaX_init()` and permanently fixed in the GC, using the `extra` field of `TString` to store their token number for fast lookup via `isreserved()`.

The `LexState` structure (defined in `llex.h`) is shared between the lexer and parser. It holds the current character, line number, current and look-ahead tokens, the input stream, a token buffer (`Mbuffer`), the scanner table, and pointers to dynamic data structures (`Dyndata`) used by the parser. A one-token look-ahead is supported for constructs that need to disambiguate (e.g., record fields in table constructors).

## Key Types / Macros

| Name | Defined In | Description |
|------|-----------|-------------|
| `LexState` | `llex.h:64` | Combined lexer/parser state: current char, line numbers, current/lookahead tokens, input stream `ZIO *z`, buffer `Mbuffer *buff`, scanner table `Table *h`, `FuncState *fs`, source name, and fixed `TString` pointers for `_ENV`, `break`, `global`. |
| `Token` | `llex.h:56` | Pairs an `int token` tag with a `SemInfo seminfo` union. |
| `SemInfo` | `llex.h:49` | Union of `lua_Number r`, `lua_Integer i`, and `TString *ts`. |
| `RESERVED` | `llex.h:32` | Enum of all reserved-word and special tokens, starting at `FIRST_RESERVED` (= `UCHAR_MAX + 1`). |
| `FIRST_RESERVED` | `llex.h:20` | Threshold separating single-char tokens (ASCII) from multi-char/reserved tokens. |
| `NUM_RESERVED` | `llex.h:46` | Count of reserved words (`TK_WHILE - FIRST_RESERVED + 1`). |
| `LUA_ENV` | `llex.h:24` | The string `"__ENV"`, used as the environment variable name. |
| `next(ls)` | `llex.c:36` | Macro: reads the next character from `ZIO` into `ls->current`. |
| `currIsNewline(ls)` | `llex.c:45` | Macro: tests if the current character is `\n` or `\r`. |
| `save_and_next(ls)` | `llex.c:60` | Macro: appends the current character to the token buffer, then advances. |
| `LUA_MINBUFFER` | `llex.c:41` | Minimum initial size of the token buffer (32 bytes). |

## Functions

### `luaX_init(lua_State *L)`

One-time initialization: creates the `"_ENV"` name and all reserved-word `TString` objects, fixing them in the GC so they are never collected. Each reserved word's `extra` field is set to its token number + 1, enabling `isreserved()` to map a string to its token tag in O(1).

### `luaX_setinput(lua_State *L, LexState *ls, ZIO *z, TString *source, int firstchar)`

Binds the lexer to an input stream. Initializes all `LexState` fields: resets line counter to 1, clears look-ahead (`TK_EOS`), sets the `FuncState` pointer to NULL, pre-loads the first character, and resolves the fixed `_ENV`, `break`, and (in compat mode) `global` strings. Resizes the token buffer to `LUA_MINBUFFER`.

### `luaX_next(LexState *ls)`

The parser's primary entry point for consuming tokens. If a look-ahead token exists, it is moved into `ls->t` and the look-ahead is discharged; otherwise `llex()` is called to scan a fresh token. Records `lastline` before advancing for error messages.

### `luaX_lookahead(LexState *ls)`

Scans and stores one token of look-ahead without consuming the current token. Used by the parser to disambiguate constructs like table constructor fields (`NAME '='` vs. plain expression). Only one look-ahead is permitted at a time (asserted).

### `llex(LexState *ls, SemInfo *seminfo)` (static)

The core scanning state machine. Resets the token buffer and enters an infinite loop, dispatching on the current character:

- **Whitespace / newlines**: consumed, line counter incremented.
- **`-`**: if followed by `-`, enters comment handling (short comment to end of line, or long comment via `skip_sep` + `read_long_string` with `seminfo=NULL`).
- **`[`**: uses `skip_sep` to detect long strings (`[[...]]` with matching `=` separators); otherwise returns `'['`.
- **Multi-char operators**: `==`, `~=`, `<=`, `>=`, `<<`, `>>`, `//`, `..`, `...`, `::` — each consumes the second character when matched.
- **String literals**: `"` or `'` delimiters, dispatched to `read_string()`.
- **Numbers**: digits or leading `.` followed by digit, dispatched to `read_numeral()`.
- **Identifiers / reserved words**: alphabetic or `_` start, accumulated into the buffer, then checked via `isreserved()`. Reserved words return their token tag; identifiers are interned via `anchorstr()` and returned as `TK_NAME`.
- **Single-char tokens**: any other character is returned as-is.
- **EOF**: returns `TK_EOS`.

### `read_numeral(LexState *ls, SemInfo *seminfo)` (static)

Reads a numeric literal (decimal, hexadecimal with `0x`/`0X`, with optional exponent `Ee`/`Pp`). The scanning is intentionally liberal; `luaO_str2num()` performs the actual validation. A trailing alphabetic character is deliberately consumed to force a "malformed number" error rather than splitting the token. Returns `TK_INT` or `TK_FLT` with the parsed value stored in `seminfo`.

### `skip_sep(LexState *ls)` (static)

Reads a `[` or `]` followed by a run of `=` characters. Returns the separator length + 2 for a matched pair (e.g., `[[` → 2, `[==[` → 4), 1 for a lone bracket, or 0 for an unfinished `[==...` sequence.

### `read_long_string(LexState *ls, SemInfo *seminfo, size_t sep)` (static)

Reads a long string or long comment until the matching `]=*]` with the same separator length. Newlines are normalized to `\n` and counted. When `seminfo` is NULL (comment mode), text is discarded without buffering. When reading a string, builds `seminfo->ts` from the buffer minus the delimiter characters.

### `read_string(LexState *ls, int del, SemInfo *seminfo)` (static)

Reads a short string literal delimited by `del` (`"` or `'`). Handles all escape sequences: `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v` (C escapes); `\xXX` (hexadecimal); `\u{XXXX}` (UTF-8 codepoint); `\ddd` (decimal); `\z` (span-whitespace); `\\`, `\"`, `\'` (literal); and newline escapes (normalized to `\n`). Delimiters are kept in the buffer for error context, then the interned string is created via `luaX_newstring()`.

### `readhexaesc(LexState *ls)` (static)

Reads exactly two hexadecimal digits after `\x`, returning the byte value. Removes the digit characters from the token buffer.

### `readutf8esc(LexState *ls)` (static)

Reads a `\u{XXXX...}` escape sequence. Enforces a 31-bit maximum value. Returns the Unicode codepoint; the caller (`utf8esc`) encodes it into 1–4 UTF-8 bytes.

### `utf8esc(LexState *ls)` (static)

Wrapper that calls `readutf8esc()`, then encodes the resulting codepoint into UTF-8 bytes via `luaO_utf8esc()` and appends them to the token buffer.

### `readdecesc(LexState *ls)` (static)

Reads up to 3 decimal digits as a `\ddd` escape, erroring if the value exceeds `UCHAR_MAX`. Removes the consumed digits from the buffer.

### `esccheck(LexState *ls, int c, const char *msg)` (static)

Raises a string-escape error if condition `c` is false. Optionally saves the current character into the buffer for error context before reporting.

### `gethexa(LexState *ls)` (static)

Consumes and validates one hexadecimal digit, returning its numeric value (0–15).

### `anchorstr(LexState *ls, TString *ts)` (static)

Internalizes a string in the scanner table. On a hit, reuses the stored `TString` (pointer equality suffices because all strings are unified). On a miss, pins the string via a temporary stack slot so the GC keeps it alive until compilation ends.

### `luaX_newstring(LexState *ls, const char *str, size_t l)`

Public wrapper: creates a `TString` from `str`/`l` via `luaS_newlstr()`, then anchors it via `anchorstr()`.

### `inclinenumber(LexState *ls)` (static)

Advances past a newline sequence (`\n`, `\r`, `\n\r`, or `\r\n`) and increments `ls->linenumber`. Raises a lexical error if the line count overflows `INT_MAX`.

### `save(LexState *ls, int c)` (static)

Appends character `c` to the token buffer, growing it by 1.5× when full. Raises "lexical element too long" if the buffer exceeds `MAX_SIZE * 2/3`.

### `check_next1(LexState *ls, int c)` (static)

If the current character is `c`, consumes it and returns 1; otherwise returns 0 without side effects.

### `check_next2(LexState *ls, const char *set)` (static)

If the current character matches either character in the 2-char string `set`, saves it to the buffer and advances, returning 1; otherwise returns 0.

### `luaX_token2str(LexState *ls, int token)`

Returns a printable representation of `token` for error messages: quotes single-char symbols and reserved words, verbatim for names/strings/numerals.

### `txtToken(LexState *ls, int token)` (static)

Builds the "near \<token\>" text for syntax errors. For name/string/number tokens, null-terminates the buffer and formats its contents; for other tokens, delegates to `luaX_token2str`.

### `lexerror(LexState *ls, const char *msg, int token)` (static)

Reports a lexical error with source:line info and optional "near \<token\>" suffix. Throws `LUA_ERRSYNTAX` and never returns.

### `luaX_syntaxerror(LexState *ls, const char *msg)`

Public entry point for parser-raised syntax errors. Anchors the error message on the current token (`ls->t.token`) and delegates to `lexerror`.
