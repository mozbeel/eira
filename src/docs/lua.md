# lua.c — Stand-alone Lua interpreter (Eira dialect)

> **AI-Generated Documentation**

## Overview

`lua.c` is the entry point for the Eira interpreter's command-line executable. It implements the complete lifecycle of a standalone Lua session: creating a `lua_State`, parsing CLI options, opening standard libraries, running initialization scripts, executing user scripts, and — when appropriate — entering an interactive Read-Eval-Print Loop (REPL).

The file is structured in two halves. The first half contains error-reporting utilities, signal handling, argument parsing, and helper routines for loading and running code. The second half is the REPL proper, handling multi-line input, expression shortcutting (`return <line>;`), and prompt management. The top-level `main` function ties everything together by calling `pmain` in protected mode.

CLI options supported include `-e` (execute a string), `-l` (require a library), `-i` (force interactive mode), `-v` (print version), `-E` (ignore environment variables), and `-W` (enable warnings). The `LUA_INIT` / `LUA_INIT_<version>` environment variable is consulted for startup initialization.

Signal handling is POSIX-aware: `SIGINT` is caught and translated into a Lua hook that interrupts the running chunk, rather than attempting to touch Lua state directly from a C signal handler. This two-stage approach (C handler → Lua hook → error) is necessary because `SIGINT` handlers run asynchronously and cannot safely call most Lua API functions.

A bitmask-based argument parsing scheme (`collectargs`) allows the interpreter to determine which options need processing before any Lua code runs (`-e`, `-l`, `-W`, `-E`, `-v`) versus which happen afterward (the main script, the REPL).

## Signal Handling

The interpreter uses a two-stage approach for `SIGINT` handling. The C signal handler `laction` cannot safely modify Lua state directly (it runs asynchronously with no synchronization guarantee), so it instead installs `lstop` as a Lua VM hook. `lstop` fires at the next instruction boundary and raises an `"interrupted!"` error via `luaL_error`, cleanly aborting the running chunk.

```c
static void laction (int i) {
  int flag = LUA_MASKCALL | LUA_MASKRET | LUA_MASKLINE | LUA_MASKCOUNT;
  setsignal(i, SIG_DFL);
  lua_sethook(globalL, lstop, flag, 1);
}
```

The `setsignal` function uses POSIX `sigaction` with no signals masked when available, falling back to `signal` on other platforms. After arming the hook, `SIGINT` is reset to `SIG_DFL` so that a second interrupt terminates the process immediately.

The `globalL` pointer is a file-scope variable set by `docall` before every `lua_pcall`, ensuring the signal handler always has access to the active Lua state. This is safe because only one state exists in the standalone interpreter.

## REPL Architecture

The REPL implements a read-eval-print cycle with several sophisticated features:

1. **Expression shortcut**: Every input line is first tried as `"return <line>;"` so that expressions print their results automatically.
2. **Multi-line support**: If a line is incomplete (syntax error ending in `<eof>`), continuation lines are read with the `">> "` prompt and concatenated.
3. **Local warning**: Lines beginning with `local` trigger a warning that locals don't persist across REPL lines.
4. **History**: Non-empty lines are saved via `lua_saveline` (readline's `add_history` or a no-op).
5. **Prompt customization**: `_PROMPT` and `_PROMPT2` globals control the prompt strings, with `tostring` applied.

Readline integration is multi-layered: the build may statically link readline, dynamically load it via `dlopen` (controlled by `LUA_READLINELIB` environment variable), or fall back to plain `fgets` over stdout.

## Functions

### `setsignal(int sig, void (*handler)(int))`

Installs a C signal handler. On POSIX platforms, uses `sigaction` with an empty signal mask so that the handler runs immediately without blocking other signals. On non-POSIX builds, falls back to the standard `signal` call.

```c
struct sigaction sa;
sa.sa_handler = handler;
sa.sa_flags = 0;
sigemptyset(&sa.sa_mask);
sigaction(sig, &sa, NULL);
```

### `lstop(lua_State *L, lua_Debug *ar)`

Lua hook callback armed by `laction`. Resets the hook via `lua_sethook(L, NULL, 0, 0)` and raises an `"interrupted!"` error via `luaL_error`. This is the safe way to abort a running chunk from a signal — the Lua state mutation happens here, inside the VM's instruction dispatch, not in the C signal handler.

### `laction(int i)`

C-level `SIGINT` handler. It cannot safely touch Lua state, so it arms `lstop` as a Lua hook (masking all event types) with a count of 1, ensuring the next instruction dispatch will trigger the abort. After arming the hook, it resets `SIGINT` to `SIG_DFL` so a second interrupt kills the process.

### `print_usage(const char *badoption)`

Writes a usage message to stderr. Distinguishes between a missing argument for `-e`/`-l` (which takes a concatenated or next-argv argument) and a completely unrecognized option.

### `l_message(const char *pname, const char *msg)`

Prints `"pname: msg\n"` to stderr using `lua_writestringerror`. Used by all error-reporting paths; if `pname` is NULL the prefix is omitted.

### `report(lua_State *L, int status)`

If `status` is not `LUA_OK`, pops and prints the error message from the stack top. Always returns `status` so callers can chain:

```c
return report(L, docall(L, narg, nres));
```

If the error object is not a string, prints `"(error message not a string)"`.

### `msghandler(lua_State *L)`

Protected-call message handler pushed under every `lua_pcall`. For non-string errors, attempts `__tostring` metamethod first, then falls back to `"(error object is a <type> value)"`. Appends a standard traceback via `luaL_traceback`.

### `docall(lua_State *L, int narg, int nres)`

Wraps `lua_pcall` with two additions: (1) the `msghandler` is inserted below the function and arguments on the stack, and (2) a temporary `SIGINT` → `laction` handler is active during execution. Saves and restores the signal handler, removes `msghandler` from the stack afterward.

```c
int base = lua_gettop(L) - narg;
lua_pushcfunction(L, msghandler);
lua_insert(L, base);
globalL = L;
setsignal(SIGINT, laction);
status = lua_pcall(L, narg, nres, base);
```

### `print_version(void)`

Prints `LUA_COPYRIGHT` followed by a newline via `lua_writestring` and `lua_writeline`.

### `createargtable(lua_State *L, char **argv, int argc, int script)`

Builds the global `arg` table. Index 0 holds `argv[script]` (the script name). Arguments after the script go to positive indices; arguments before it go to negative indices. The table is created with capacity for `narg` array entries and `script + 1` hash entries.

### `dochunk(lua_State *L, int status)`

If `status == LUA_OK`, calls `docall(L, 0, 0)` to execute the loaded chunk. Then reports any error via `report`. This is the shared tail of `dofile`, `dostring`, and other load-and-run paths.

### `dofile(lua_State *L, const char *name)`

Loads a file (binary or text, via mode `"bt"`) using `luaL_loadfilex` and runs it via `dochunk`. Used for `LUA_INIT` files and the main script.

### `dostring(lua_State *L, const char *s, const char *name)`

Loads a string as a text chunk (mode `"t"`) using `luaL_loadbufferx` and runs it via `dochunk`. Used for `-e` arguments and `LUA_INIT` string values.

### `dolibrary(lua_State *L, char *globname)`

Parses `globname[=modname]`: if `=` is present, the left side is the global name and the right is the module name. Otherwise `require(modname)` is assigned to `globname`. A suffix mark (from `LUA_IGMARK`, typically `-`) after the global name is stripped before assignment, so `mylib-2.0` becomes global `mylib` requiring module `mylib-2.0`.

```c
modname = strchr(globname, '=');
if (modname == NULL) {
  modname = globname;
  suffix = strchr(modname, *LUA_IGMARK);
}
```

### `pushargs(lua_State *L)`

Pushes all values from the global `arg` table (indices 1 through `#arg`) onto the stack and returns their count. Uses `luaL_checkstack` to ensure room. Used to forward script arguments to the main script's function call.

### `handle_script(lua_State *L, char **argv)`

Loads `argv[0]` as a file (`"-"` means stdin unless preceded by `--`), pushes script arguments via `pushargs`, and runs the chunk with `docall`. Reports any error.

### `collectargs(char **argv, int *first)`

Scans all `argv` entries, building a bitmask of `has_*` flags. Sets `*first` to the script index, 0 if no script, or -1 if no program name.

```c
#define has_error	1
#define has_i		2
#define has_v		4
#define has_e		8
#define has_E		16
```

The `-i` flag implicitly sets `has_v` (version is always shown in interactive mode). `-e` and `-l` consume their argument from the next `argv` entry if not concatenated.

### `runargs(lua_State *L, char **argv, int n)`

Iterates options before the script. For `-e`, executes the string via `dostring`; for `-l`, loads the library via `dolibrary`; for `-W`, enables warnings via `lua_warning(L, "@on", 0)`. Disables all warnings at the start with `lua_warning(L, "@off", 0)`.

### `no_getenv(const char *name)`

Always returns NULL. Used as a function pointer replacement for `getenv` when `-E` is active, causing `handle_luainit` to skip environment-based initialization. The `LUA_NOENV` registry key is also set so libraries can check it.

### `handle_luainit(lua_State *L)`

Checks `LUA_INIT_<version>` (e.g., `LUA_INIT_5.5`) then `LUA_INIT` in the environment. If the value starts with `@`, loads the named file via `dofile`; otherwise evaluates the value as a Lua string via `dostring`. Returns `LUA_OK` if neither variable is set.

### `get_prompt(lua_State *L, int firstline)`

Returns the REPL prompt string by reading `_PROMPT` (first line) or `_PROMPT2` (continuation lines) from globals. Falls back to `"> "` / `">> "` if nil. Applies `luaL_tolstring` (which invokes `tostring` metamethod) if the value is non-nil. The value is kept anchored on the stack.

### `incomplete(lua_State *L, int status)`

Returns 1 if `status` is `LUA_ERRSYNTAX` and the error message ends with `"<eof>"` (the `EOFMARK`), indicating an incomplete statement that needs more input.

### `pushline(lua_State *L, int firstline)`

Displays the prompt via `get_prompt`, reads one line (up to `LUA_MAXINPUT` = 512 chars) via `lua_readline`, strips the trailing newline, and pushes the result. Returns 0 on EOF (NULL from readline).

### `addreturn(lua_State *L)`

Tries to compile the line on the stack as `"return <line>;"`. On success, removes the original line and leaves the compiled chunk. On failure, pops both the result and the modified line, leaving the stack unchanged.

### `checklocal(const char *line)`

Warns to stderr when a REPL line begins with `local` (after skipping whitespace), since locals don't survive across lines in interactive mode.

### `multiline(lua_State *L)`

Reads continuation lines (with `">> "` prompt) and concatenates them with `"\n"` separators until the accumulated text compiles as a complete statement or a non-incomplete error occurs. The first line is already on the stack at index 1.

### `loadline(lua_State *L)`

One complete REPL input cycle: clears the stack, pushes the first line, tries `addreturn`, falls back to `multiline`, saves non-empty lines to history, and returns the load status. Returns -1 on EOF.

### `l_print(lua_State *L)`

Calls the global Lua `print` function with all values currently on the stack. Uses `lua_pcall` so errors in `print` are caught and reported via `l_message` rather than aborting the REPL.

### `doREPL(lua_State *L)`

The main REPL loop. Suppresses the program name in error messages by setting `progname = NULL`. Calls `lua_initreadline`, then loops: `loadline` → `docall` → `l_print`. Clears the stack and writes a final newline on exit.

### `pmain(lua_State *L)`

Protected-mode main body. The execution order is:
1. Parse args via `collectargs`
2. Print version if `-v`
3. Set `l_getenv` based on `-E`
4. Open standard libraries via `luai_openlibs`
5. Build the `arg` table
6. Start GC in generational mode
7. Run `LUA_INIT`
8. Run `-e`/`-l`/`-W` options
9. Run the main script
10. Enter REPL if `-i`, or if stdin is a tty and no script was given

Returns 1 (boolean true) on success; 0 on any error.

### `main(int argc, char **argv)`

Creates a fresh `lua_State` with `luaL_newstate`. Disables GC during state construction. Calls `pmain` in protected mode via `lua_pcall`, passing `argc`/`argv` as a lightuserdata pair. Reports errors and returns `EXIT_SUCCESS` or `EXIT_FAILURE`.
