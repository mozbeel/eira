# loslib.c — Standard Operating System library

> **AI-Generated Documentation**

## Overview

`loslib.c` implements the `os` standard library, providing Eira scripts with access to time/date manipulation, environment variables, file operations, process control, and locale settings. The library is registered as a single module via `luaopen_os`, which simply creates the function table — no extra constants or state are needed.

Time and date functions use `struct tm` and C's `mktime`/`gmtime`/`localtime` (or their POSIX `_r` variants when available). Date tables in Lua use a human-friendly field layout (`year`, `month`, `day`, `hour`, `min`, `sec`, `wday`, `yday`, `isdst`) with deltas applied to convert from C's zero-based fields (e.g. `tm_year + 1900`). The `os.date` function supports both `strftime`-style format strings (each specifier validated against `LUA_STRFTIMEOPTIONS`) and the `*t` table form. The `!` prefix forces UTC via `gmtime`.

Platform abstractions are handled through preprocessor macros: `LUA_NUMTIME` controls whether time values are `lua_Integer` or `lua_Number`; `lua_tmpnam` wraps `mkstemp` on POSIX and `tmpnam` elsewhere; `l_system` wraps `system()` but stubs it on iOS. The file is 474 lines and is one of the simplest standard library modules.

## Functions

### os_execute([cmd])

Implements `os.execute`. With an argument, runs the shell command via `l_system(cmd)` and returns the exit status through `luaL_execresult` (which produces `true/nil, "exit"/"signal", code`). Without arguments, returns a boolean indicating whether a shell is available.

### os_remove(filename)

Implements `os.remove`. Deletes the file at `filename`. Returns `true` on success, or `nil` + error message on failure via `luaL_fileresult`.

### os_rename(from, to)

Implements `os.rename`. Renames the file or directory from `from` to `to`. Returns `true` on success, or `nil` + error message on failure.

### os_tmpname()

Implements `os.tmpname`. Generates a unique temporary file name. On POSIX, uses `mkstemp` with the template `/tmp/lua_XXXXXX` (closed immediately). On non-POSIX systems, uses `tmpnam`. Raises an error if name generation fails.

### os_getenv(name)

Implements `os.getenv`. Returns the value of the environment variable `name` as a string, or `nil` if the variable is not set.

### os_clock()

Implements `os.clock`. Returns the CPU time used by the program in seconds as a floating-point number, computed as `clock() / CLOCKS_PER_SEC`.

### setfield(L, key, value, delta)

Internal helper: pushes `(lua_Integer)value + delta` onto the stack and sets it as field `key` in the table at the top. Includes an overflow check when `LUA_NUMTIME` is defined and integers are 32-bit.

### setboolfield(L, key, value)

Internal helper: sets a boolean field in the table. When `value` is negative (indicating an undefined C field like `tm_isdst`), the field is omitted rather than set.

### setallfields(L, stm)

Internal helper: writes every `struct tm` field into the Lua table on top of the stack, using `setfield` (with appropriate deltas) and `setboolfield`. Used by both `os.date` (table form) and `os.time` (to write back normalized values).

### getboolfield(L, key)

Internal helper: reads a boolean field from the table. Returns -1 when the field is `nil` (undefined), 0 for false, or 1 for true.

### getfield(L, key, d, delta)

Internal helper: reads an integer field from the table, applying the inverse delta. Uses the default `d` when the field is absent (raises if `d < 0`). Validates that the value is an integer and within range.

### checkoption(L, conv, convlen, buff)

Validates a single `strftime` conversion specifier against the platform's `LUA_STRFTIMEOPTIONS` string. Supports both single-character and (where available) two-character specifiers separated by `||`. Copies the validated specifier into `buff` and returns a pointer past it.

### l_checktime(L, arg)

Reads a time argument (integer or float per `LUA_NUMTIME`) and validates that it fits into a `time_t` without truncation.

### os_date([format [, time]])

Implements `os.date`. With the format `"*t"`, returns a table of broken-down date fields. With `"!*t"`, uses UTC. Any other format is expanded through `strftime`, with each conversion specifier individually validated by `checkoption`. The `!` prefix at the start forces UTC. Defaults to the current time when `time` is omitted.

### os_time([table])

Implements `os.time`. Without arguments, returns the current time. With a date table, builds a `struct tm` via `getfield`, calls `mktime` to normalize, then writes the normalized values back with `setallfields`. Validates that the result fits in the Lua time representation.

### os_difftime(t2, t1)

Implements `os.difftime`. Returns the difference `t1 - t2` in seconds as a floating-point number. Note: the parameter order is `(t1, t2)` but the documentation specifies `difftime(t2, t1)`.

### os_setlocale(locale [, category])

Implements `os.setlocale`. Sets the process locale for the given category (`"all"`, `"collate"`, `"ctype"`, `"monetary"`, `"numeric"`, `"time"`). Returns the resulting locale string, or `nil` on failure. Uses a static mapping from category names to `LC_*` constants.

### os_exit([code [, close]])

Implements `os.exit`. Terminates the process with the given exit code. A boolean code maps to `EXIT_SUCCESS`/`EXIT_FAILURE`. When the second argument is true, `lua_close` is called before `exit` to clean up the Lua state.

### luaopen_os(L)

Opens the `os` library by creating the function table via `luaL_newlib`. Returns the table.
