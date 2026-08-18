# loadlib.c — Dynamic library loader and require implementation

> **AI-Generated Documentation**

## Overview

`loadlib.c` implements the `package` standard library and the global `require` function for Eira. It provides platform-specific dynamic loading via three compilation paths: POSIX `dlopen`/`dlclose`/`dlsym` (`LUA_USE_DLOPEN`), Windows `LoadLibraryExA`/`FreeLibrary`/`GetProcAddress` (`LUA_DL_DLL`), and a stub fallback that reports "dynamic libraries not enabled" on unsupported platforms.

The `require` mechanism is built on four searcher functions stored in `package.searchers`: (1) `searcher_preload` — looks up pre-registered loaders in `package.preload`; (2) `searcher_Lua` — finds `.lua` files on `package.path`; (3) `searcher_C` — finds shared libraries on `package.cpath` and resolves their `luaopen_*` entry point; (4) `searcher_Croot` — for dotted module names like `a.b`, loads the root library `a` from `package.cpath` then resolves `luaopen_a_b` within it.

Loaded C libraries are tracked in `registry._CLIBS` using lightuserdata handles. Each loaded library also keeps an external string with a deallocation callback (`freelib`) so that when the string is garbage-collected, the DLL is automatically unloaded — ensuring DLLs outlive any strings they produce. The `setpath` function handles environment variable configuration (`LUA_PATH` / `LUA_CPATH`), versioned variable names, and the `;;` default-path insertion idiom.

The file is 816 lines and exposes two Lua function tables: `pk_funcs` (`package.loadlib`, `package.searchpath`, plus placeholders for `path`, `cpath`, `searchers`, `loaded`, `preload`) and `ll_funcs` (the global `require`).

## Functions

### lsys_unloadlib(lib)

Platform-specific: unloads a dynamically loaded library. Uses `dlclose` on POSIX, `FreeLibrary` on Windows, and is a no-op on unsupported platforms.

### lsys_load(L, path, seeglb)

Platform-specific: opens a shared library at `path`. On POSIX, uses `dlopen` with `RTLD_NOW`, setting `RTLD_GLOBAL` when `seeglb` is true (used when loading with `*` symbol). On Windows, uses `LoadLibraryExA`; the `seeglb` parameter is ignored because Windows symbols are global by default. Pushes an error string on failure.

### lsys_sym(L, lib, sym)

Platform-specific: resolves a C function symbol `sym` in the loaded library `lib`. Uses `dlsym` on POSIX and `GetProcAddress` on Windows. Pushes an error string on failure.

### setprogdir(L) (Windows only)

Replaces every occurrence of `LUA_EXEC_DIR` in the path string on top of the stack with the directory of the running executable, determined via `GetModuleFileNameA`.

### pusherror(L) (Windows only)

Pushes the human-readable Windows error text for the last failed system call, obtained via `FormatMessageA`. Falls back to a numeric error code if formatting fails.

### noenv(L)

Reads `registry.LUA_NOENV` as a boolean. When true, environment variables are ignored and compiled-in default paths are used unconditionally.

### setpath(L, fieldname, envname, dft)

Sets `package.path` or `package.cpath`. Checks the versioned environment variable first (`LUA_PATH_5.5`), then the plain one (`LUA_PATH`), falling back to the compiled-in default `dft`. A `;;` in the environment value is replaced with the default path. On Windows, `LUA_EXEC_DIR` is expanded via `setprogdir`.

### checkclib(L, path)

Looks up `registry._CLIBS[path]` and returns the library handle (lightuserdata) if already loaded, or `NULL` if not yet cached.

### freelib(ud, ptr, osize, nsize)

Deallocation callback for library strings created by `createlibstr`. When garbage collection reclaims the string, this callback calls `lsys_unloadlib` to unload the associated DLL. The string content itself is irrelevant.

### createlibstr(L, plib)

Creates an external string whose deallocation callback (`freelib`) will unload the DLL `plib`. This keeps the DLL alive as long as any of its strings remain reachable in Lua.

### addtoclib(L, path, plib)

Records a newly loaded library handle in `registry._CLIBS[path]` and creates a library string (via `createlibstr`) referenced from that table, ensuring the DLL is unloaded only when `registry._CLIBS` itself is collected.

### lookforfunc(L, path, sym)

Loads a C library at `path` (or reuses the cached handle from `checkclib`) and resolves symbol `sym`. A `sym` of `"*"` means load-only: returns `true` with global symbols. Otherwise pushes the resolved `lua_CFunction`. Returns 0 on success, `ERRLIB` if the library could not be loaded, or `ERRFUNC` if the symbol was not found.

### ll_loadlib(L)

Implements `package.loadlib(path, init)`. Calls `lookforfunc` to load and resolve. Returns the function on success, or `fail` + error message + reason (`"open"` or `"init"`) on failure.

### readable(filename)

Tests whether a file exists and can be opened for reading. Returns 1 if readable, 0 otherwise. Used by the path searchers.

### getnextfilename(path, end)

Iterates over a `';'`-separated path list in place, returning each file name in turn. Replaces separators with `'\0'` and restores them on subsequent calls. Returns `NULL` when the list is exhausted.

### pusherrornotfound(L, path)

Builds a multi-line error message of the form `"no file 'X'\n\tno file 'Y'"` listing every entry in a `';'`-separated path that was searched.

### searchpath(L, name, path, sep, dirsep)

Implements the core path-search logic used by both `package.searchpath` and the Lua/C searchers. Substitutes the module `name` for every `?` in `path`, replacing `sep` with `dirsep`, then tests each candidate with `readable`. Returns the first readable file, or pushes a `"no file"` error listing all tried entries.

### ll_searchpath(L)

Implements `package.searchpath(name, path [, sep [, rep]])`. Delegates to `searchpath` and returns the found path, or `fail` + error message.

### findfile(L, name, pname, dirsep)

Reads `package[pname]` (either `"path"` or `"cpath"`) from the upvalue table and calls `searchpath` with `'.'` as the name separator.

### checkload(L, stat, filename)

Converts a loader result into `(open_function, filename)` when `stat` is true. On failure, raises a detailed error including the searcher's message.

### searcher_Lua(L)

Searcher #2 for `require`: finds `name` on `package.path` via `findfile`, loads it as a bytecode chunk with `luaL_loadfilex`, and returns the loader function plus the filename.

### loadfunc(L, filename, modname)

Finds the C open function for a module: converts dots to underscores, and for names with an ignore mark (`X-Y`), first tries `luaopen_X` then `luaopen_Y`. Calls `lookforfunc` to resolve the symbol.

### searcher_C(L)

Searcher #3 for `require`: finds a C library on `package.cpath` and resolves its `luaopen_*` function via `loadfunc`.

### searcher_Croot(L)

Searcher #4 for `require`: for dotted names like `a.b`, first loads the root library `a` from `package.cpath`, then resolves `luaopen_a_b` within that library. Handles the case where the root exists but the sub-module function is missing.

### searcher_preload(L)

Searcher #1 for `require`: looks up `name` in `package.preload`. Returns the pre-registered loader function and the string `":preload:"` as loader data, or an error message if not found.

### findloader(L, name)

Iterates over `package.searchers`, calling each with the module name until one returns a loader function. Concatenates all error messages for the final "module not found" error. Raises if no searcher succeeds.

### ll_require(L)

Implements the global `require(name)`. Consults `registry._LOADED[name]` for a cached result. If not found, calls `findloader` to obtain a loader, invokes it with the module name and loader data, caches the return value (or `true` if nil), and returns the module plus loader data.

### createsearcherstable(L)

Populates `package.searchers` with the four built-in searchers: `searcher_preload`, `searcher_Lua`, `searcher_C`, `searcher_Croot`. Each is a closure with the `package` table as its upvalue, so they can read `path`/`cpath`/`searchers`.

### luaopen_package(L)

Opens the `package` library. Creates `registry._CLIBS`, the `package` table (with `loadlib`, `searchpath`, and placeholders), sets `path`/`cpath`/`config`/`loaded`/`preload`, installs the four searchers, and registers the global `require` function. Returns the `package` table.
