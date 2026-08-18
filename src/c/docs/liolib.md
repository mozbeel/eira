# liolib.c — Eira Standard I/O Library

> **AI-Generated Documentation**

## Overview

This file implements the **I/O library** for the Eira Lua 5.5 dialect, exposed as the `io` table. It provides a comprehensive interface to C standard I/O (`FILE *`) operations, including file opening, reading, writing, seeking, flushing, and closing, as well as subprocess piping and temporary file creation. The library operates on `LStream` userdata (a `luaL_Stream` containing a `FILE *` and a close function pointer) with a metatable named `LUA_FILEHANDLE`.

The library offers two APIs: a **module-level API** (`io.open`, `io.read`, `io.write`, etc.) that operates on default input/output streams, and a **method-level API** (`file:read`, `file:write`, `file:seek`, etc.) that operates on individual file handles. The default input and output streams are stored in the registry under keys `_IO_input` and `_IO_output` and can be reassigned via `io.input()` and `io.output()`.

A significant portion of the file handles buffered I/O configuration and the `lines` iteration pattern. The `lines` function creates a closure (`io_readline`) with upvalues holding the file handle, argument count, close flag, and read format specifications. This closure reads one iteration's worth of data per call and automatically closes the file (when created by `io.lines`) at EOF. The read subsystem supports multiple format specifiers: `*n` (number with locale-aware decimal point and hex/exponent scanning), `*l` (line), `*L` (line with newline), `*a` (entire file), and numeric byte counts.

Platform-specific abstractions cover `popen`/`pclose` (POSIX vs Windows vs unsupported), file locking (`flockfile`/`funlockfile` on POSIX, no-ops elsewhere), character reading (`getc_unlocked` vs `getc`), and large-file seeking (`fseeko`/`ftello` on POSIX, `_fseeki64`/`_ftelli64` on MSVC, `fseek`/`ftell` as fallback). The file handle metatable defines `__gc` and `__close` (both point to `f_gc` which silently closes open files) and `__tostring`.

## Functions

### `l_checkmode(const char *mode)`

Validates an `fopen` mode string. Accepts one of `r`/`w`/`a`, an optional `+`, and optional binary extension characters (defined by `L_MODEEXT`, default `"b"`). Returns nonzero if the mode is valid.

### `io_type(lua_State *L)`

Implements `io.type(obj)`. Returns `"file"` for open file handles, `"closed file"` for closed handles, or `nil` for non-file values. Uses `luaL_testudata` to safely check the userdata type.

### `f_tostring(lua_State *L)`

`__tostring` metamethod for file handles. Returns `"file (closed)"` for closed handles or `"file (0xADDRESS)"` for open ones.

### `tofile(lua_State *L)`

Helper: returns the underlying `FILE *` from a file handle userdata, raising an error if the handle is closed. Used by virtually every file method.

### `newprefile(lua_State *L)`

Creates a file handle userdata pre-marked as closed (`closef = NULL`). This ensures that if a subsequent `fopen` fails due to memory exhaustion, the handle is in a consistent state for garbage collection.

### `aux_close(lua_State *L)`

Closes a file handle by calling its stored `closef` function pointer. Sets `closef` to `NULL` before calling to prevent double-close from `__gc`. Uses a `volatile` local to work around a Clang compiler bug.

```c
volatile lua_CFunction cf = p->closef;
p->closef = NULL;
return (*cf)(L);
```

### `f_close(lua_State *L)`

Implements `file:close()`. Verifies the handle is open via `tofile`, then delegates to `aux_close`.

### `io_close(lua_State *L)`

Implements `io.close([file])`. When no argument is given, closes the default output file. Delegates to `f_close`.

### `f_gc(lua_State *L)`

`__gc` and `__close` metamethod for file handles. Silently closes a file that is still open. Ignores already-closed or incompletely-initialized handles (where `f` is `NULL`).

### `io_fclose(lua_State *L)`

`closef` function for regular files. Calls `fclose` and returns the result via `luaL_fileresult`, reporting `errno` on failure.

### `newfile(lua_State *L)`

Helper: creates a file handle via `newprefile`, sets `closef` to `io_fclose`, and returns it. Used before `fopen` attempts.

### `opencheck(lua_State *L, const char *fname, const char *mode)`

Helper: creates a handle and opens the file immediately, raising a Lua error with the errno message if `fopen` fails.

### `io_open(lua_State *L)`

Implements `io.open(filename, [mode])`. Validates the mode via `l_checkmode`, opens the file with `fopen`, and returns the file handle on success or `(nil, message, errno)` on failure.

```c
LStream *p = newfile(L);
p->f = fopen(filename, mode);
return (p->f == NULL) ? luaL_fileresult(L, 0, filename) : 1;
```

### `io_pclose(lua_State *L)`

`closef` function for `popen` files. Waits for the child process via `l_pclose` and returns the execution result via `luaL_execresult`.

### `io_popen(lua_State *L)`

Implements `io.popen(command, [mode])`. Spawns a subprocess connected to a file handle using `l_popen` (`popen` on POSIX, `_popen` on Windows). The mode must be `"r"` or `"w"`. Returns `(nil, message, errno)` on failure.

### `io_tmpfile(lua_State *L)`

Implements `io.tmpfile()`. Opens an anonymous temporary file via `tmpfile()` that is automatically deleted when closed. Returns the handle or `(nil, message, errno)`.

### `getiofile(lua_State *L, const char *findex)`

Helper: fetches the default input or output `LStream` from the registry. Raises an error if the default file is closed.

### `g_iofile(lua_State *L, const char *f, const char *mode)`

Shared implementation for `io.input` and `io.output`. When given a filename string, opens the file in the specified mode. When given a file handle, validates it. When called with no arguments, returns the current default. Stores the new default in the registry.

### `io_input(lua_State *L)`

Implements `io.input([file])`. Gets or sets the default input file. When given a filename, opens it in read mode.

### `io_output(lua_State *L)`

Implements `io.output([file])`. Gets or sets the default output file. When given a filename, opens it in write mode.

### `aux_lines(lua_State *L, int toclose)`

Builds the closure used by both `file:lines()` and `io.lines()`. Creates a closure of `io_readline` with upvalues: the file handle, the argument count, a boolean close flag, and the read format arguments. The maximum number of format arguments is `MAXARGLINE` (250).

### `f_lines(lua_State *L)`

Implements `file:lines(...)`. Returns an iterator closure that reads lines (or formatted values) from the handle without closing it when iteration ends.

### `io_lines(lua_State *L)`

Implements `io.lines([filename, ...])`. When called with a filename, opens the file and returns a 4-result iterator triplet plus the file as a to-be-closed variable. When called without arguments, iterates over the default input without closing it.

```c
if (toclose) {
  lua_pushnil(L);  /* state */
  lua_pushnil(L);  /* control */
  lua_pushvalue(L, 1);  /* file as to-be-closed variable */
  return 4;
}
```

### `nextc(RN *rn)`

Number scanner step for the `*n` read format. Saves the look-ahead character into a buffer and reads the next one. Returns 0 on buffer overflow (invalidating the result).

### `test2(RN *rn, const char *set)`

Number scanner helper: consumes the current character if it matches one of the two characters in `set`. Used for optional signs, decimal points, and exponent markers.

### `readdigits(RN *rn, int hex)`

Number scanner: consumes a sequence of decimal or hexadecimal digits and returns how many were read.

### `read_number(lua_State *L, FILE *f)`

Reads a number in the `*n` format. Scans a numeral with one-character look-ahead, supporting optional sign, hex prefix (`0x`), decimal point (locale-aware), and exponent (`e`/`p`). Validates the complete numeral via `lua_stringtonumber`. Locks the file during scanning.

```c
do { rn.c = l_getc(rn.f); } while (isspace(rn.c));
test2(&rn, "-+");
if (test2(&rn, "00")) {
  if (test2(&rn, "xX")) hex = 1;
```

### `test_eof(lua_State *L, FILE *f)`

Tests for end-of-file without consuming input. Peeks at the next character via `getc`/`ungetc`. Pushes `""` and returns `true` if a character is available, `false` at EOF.

### `read_line(lua_State *L, FILE *f, int chop)`

Reads one line into a growing `luaL_Buffer`. When `chop` is false, the trailing newline is kept. Uses file locking during character reads. Returns whether anything was read (true unless EOF with no data).

### `read_all(lua_State *L, FILE *f)`

Reads the entire file contents in `LUAL_BUFFERSIZE` chunks into a single string using `luaL_Buffer`.

### `read_chars(lua_State *L, FILE *f, size_t n)`

Reads up to `n` characters into a buffer. Returns whether any characters were actually read.

### `g_read(lua_State *L, FILE *f, int first)`

Core implementation of `io.read` and `file:read`. Dispatches each format argument: numeric arguments read that many bytes, `*n` reads a number, `*l`/`*L` read a line (with/without newline), `*a` reads the entire file. Returns all values read, or `(nil, message, errno)` on error. Clears the error indicator with `clearerr` before reading.

### `io_read(lua_State *L)`

Implements `io.read(...)`. Reads from the default input file. Delegates to `g_read`.

### `f_read(lua_State *L)`

Implements `file:read(...)`. Reads from the method's file handle. Delegates to `g_read`.

### `io_readline(lua_State *L)`

The iteration function for `lines`. Reads the next chunk(s) using `g_read` with the format arguments stored as upvalues. On EOF, closes the file (if it was opened by `io.lines`) and returns 0 to end the loop. On read error, raises a Lua error.

```c
n = g_read(L, p->f, 2);
if (lua_toboolean(L, -n))
  return n;
else {
  if (lua_toboolean(L, lua_upvalueindex(3))) {
    lua_settop(L, 0);
    lua_pushvalue(L, lua_upvalueindex(1));
    aux_close(L);
  }
```

### `g_write(lua_State *L, FILE *f, int arg)`

Core implementation of `io.write` and `file:write`. Converts numbers to strings via `lua_numbertocstring`, writes each argument with `fwrite`, and tracks total bytes written. On a partial write, returns `(nil, message, error_code, bytes_written)`. On success, returns the file handle.

### `io_write(lua_State *L)`

Implements `io.write(...)`. Writes to the default output file. Delegates to `g_write`.

### `f_write(lua_State *L)`

Implements `file:write(...)`. Writes to the method's file handle and returns the handle itself for chaining.

### `f_seek(lua_State *L)`

Implements `file:seek([whence, offset])`. Moves the file position according to `"set"`, `"cur"`, or `"end"` (default `"cur"`) plus an offset (default 0). Returns the new absolute position. Uses platform-appropriate large-file seek functions.

### `f_setvbuf(lua_State *L)`

Implements `file:setvbuf(mode, [size])`. Configures the buffering mode: `"no"` (unbuffered), `"full"` (fully buffered), or `"line"` (line-buffered), with an optional buffer size defaulting to `LUAL_BUFFERSIZE`.

### `aux_flush(lua_State *L, FILE *f)`

Helper: flushes the given stream and returns the result via `luaL_fileresult`.

### `f_flush(lua_State *L)`

Implements `file:flush()`. Flushes the file handle's output stream.

### `io_flush(lua_State *L)`

Implements `io.flush()`. Flushes the default output file.

### `createmeta(lua_State *L)`

Builds the `LUA_FILEHANDLE` metatable. Registers `__gc`, `__close`, and `__tostring` metamethods, then creates a separate method table with all file methods and assigns it to `__index`.

### `io_noclose(lua_State *L)`

`closef` for the standard streams (`stdin`, `stdout`, `stderr`). Refuses to close them, returns `(nil, "cannot close standard file")`, and restores `closef` to itself so the handle remains usable.

### `createstdfile(lua_State *L, FILE *f, const char *k, const char *fname)`

Creates a handle for a standard stream, wiring it to `io_noclose`. When `k` is non-NULL, also registers it as the default input/output in the registry. Sets the handle as a field of the `io` table (e.g. `io.stdin`).

### `luaopen_io(lua_State *L)`

Opens the I/O library. Creates the `io` table, builds the file handle metatable via `createmeta`, and creates default handles for `stdin` (registered as default input), `stdout` (registered as default output), and `stderr`.
