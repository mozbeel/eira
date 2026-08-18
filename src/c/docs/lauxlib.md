# lauxlib.c — Auxiliary library for building Lua/Eira C extensions

> **AI-Generated Documentation**

## Overview

`lauxlib.c` implements the auxiliary library declared in `lauxlib.h`. This library sits *above* the public `lua.h` C API—it is built exclusively using official API calls and provides the higher-level convenience functions that library authors rely on: error formatting with source locations, argument type-checking helpers, the generic `luaL_Buffer` string-building system, the reference table mechanism, chunk loading from files and strings, metatable management, module registration, and the default state constructor (`luaL_newstate`).

The file is organized into clearly delimited sections: traceback construction, error-report functions, userdata metatable manipulation, argument-check routines, the generic buffer subsystem, the reference system (`luaL_ref` / `luaL_unref`), file and string chunk loaders, metamethod and module helpers, string substitution, the default allocator/panic/warning functions, seed generation, and state creation.

Every function in this file is intentionally written against the public API only. The file's header comment makes this explicit: *"Any function declared here could be written as an application function."* This means `lauxlib.c` serves as both a reference implementation and a validation suite for the `lua.h` surface.

For Eira specifically, this file preserves all standard Lua 5.4+ auxiliary conventions (buffer boxes with `__close` support, external string handoff in `luaL_pushresult`, the `__name` metafield in type errors) while adding no Eira-specific extensions—making it a drop-in compatible auxiliary library.

## Key Types / Macros

| Name | Location | Purpose |
|---|---|---|
| `LEVELS1` / `LEVELS2` | `lauxlib.c:42-43` | Stack frames shown at start/end of truncated tracebacks (10 / 11) |
| `UBox` | `lauxlib.c:534` | Internal struct: `void *box` + `size_t bsize` for heap-allocated buffer storage |
| `buffonstack(B)` | `lauxlib.c:607` | True when the buffer has outgrown its static area and uses a `UBox` |
| `LoadF` | `lauxlib.c:831` | Reader state for file loading: `FILE*`, pre-read count, and a `BUFSIZ` buffer |
| `LoadS` | `lauxlib.c:955` | Reader state for in-memory loading: pointer and remaining size |
| `LUA_ERRFILE` | `lauxlib.h:27` | Extra error code for file-load failures (`LUA_ERRERR + 1`) |
| `LUA_LOADED_TABLE` | `lauxlib.h:31` | Registry key `"_LOADED"` for the table of loaded modules |
| `LUA_PRELOAD_TABLE` | `lauxlib.h:34` | Registry key `"_PRELOAD"` for preloaded loaders |
| `LUALIB_API` | `lauxlib.h` | Export macro for auxiliary library functions |
| `luaL_Buffer` | `lauxlib.h:185` | Generic buffer: union of static `LUAL_BUFFERSIZE` area and heap `UBox` |
| `luaL_Reg` | `lauxlib.h:38` | Name→function mapping used by `luaL_setfuncs` and `luaL_newlib` |
| `luaL_pushfail(L)` | `lauxlib.h:174` | Pushes `false` or `nil` depending on `LUA_FAILISFALSE` |

## Functions

### findfield(lua_State *L, int objidx, int level)
(`lauxlib.c:51`) Searches the table at stack top for `objidx`, recursing into sub-tables up to `level` depth. Concatenates found names with dots. Returns 1 and pushes the dotted name on success, or 0 if not found. Used by `pushglobalfuncname` to produce readable function names for tracebacks.

### pushglobalfuncname(lua_State *L, lua_Debug *ar)
(`lauxlib.c:81`) Pushes a global name for the function described by debug info `ar`. Scans `_LOADED` tables and `_G` via `findfield`. Strips the `"_G."` prefix. Returns 1 if a name is found, 0 otherwise.

### pushfuncname(lua_State *L, lua_Debug *ar)
(`lauxlib.c:105`) Pushes a human-readable function name for `ar`. Priority order: declared name (`ar->namewhat`), `"main chunk"`, found global name, `<file:line>` for Lua functions, or `"?"` as fallback.

### lastlevel(lua_State *L)
(`lauxlib.c:123`) Returns the highest valid call-stack level. Uses exponential doubling then binary search for an exact bound.

### luaL_traceback(lua_State *L, lua_State *L1, const char *msg, int level)
(`lauxlib.c:141`) Builds and pushes a multi-line `"stack traceback:"` string for thread `L1` starting at `level`. Deep stacks (more than `LEVELS1 + LEVELS2` frames) print only the first `LEVELS1` and last `LEVELS2` frames with a skip notice. Uses `luaL_Buffer` for efficient string assembly.

### luaL_argerror(lua_State *L, int arg, const char *extramsg)
(`lauxlib.c:190`) Raises `"bad argument #N to 'name' (extramsg)"`. Accounts for extra vararg arguments and method-colon `self` parameters using the calling frame's debug info. Returns a value only to allow `return luaL_argerror(...)`.

### luaL_typeerror(lua_State *L, int arg, const char *tname)
(`lauxlib.c:218`) Raises a type-mismatch error. Uses the `__name` metafield for the actual type when available, `"light userdata"` as a special case, or `luaL_typename` otherwise.

### tag_error(lua_State *L, int arg, int tag)
(`lauxlib.c:233`) Static helper: raises a type error for `arg` using the built-in type name for `tag`.

### luaL_where(lua_State *L, int level)
(`lauxlib.c:244`) Pushes a `"source:line: "` prefix string for the call at `level`, or an empty string when no info is available. Used to prefix error messages.

### luaL_error(lua_State *L, const char *fmt, ...)
(`lauxlib.c:264`) Formats a `"source:line: message"` error and raises it via `lua_error`. Never returns. Uses `lua_pushvfstring` so it does not need reserved stack space.

### luaL_fileresult(lua_State *L, int stat, const char *fname)
(`lauxlib.c:278`) Converts a C file-operation result into Lua values. On success: pushes `true` (1 result). On failure: pushes `fail`, `"fname: strerror"`, and the errno code (3 results). Captures `errno` before any API calls that might overwrite it.

### luaL_execresult(lua_State *L, int stat)
(`lauxlib.c:323`) Converts an `exec`/`close` status into Lua return values. Forwards to `luaL_fileresult` when `errno` was set. Otherwise returns `true/false`, `"exit"/"signal"`, and the numeric status code.

### luaL_newmetatable(lua_State *L, const char *tname)
(`lauxlib.c:352`) Creates a new metatable named `tname`, registers it in the registry with `tname` as key, and sets `__name = tname`. Returns 1 if newly created, 0 if the name was already in use (leaving the existing metatable on the stack).

### luaL_setmetatable(lua_State *L, const char *tname)
(`lauxlib.c:367`) Looks up the named metatable in the registry and sets it on the value just below the stack top (popping the metatable).

### luaL_testudata(lua_State *L, int ud, const char *tname)
(`lauxlib.c:376`) Returns the userdata pointer at `ud` if it is a full userdata whose metatable matches the named registry metatable. Returns NULL otherwise without raising an error. Stack is left unchanged.

### luaL_checkudata(lua_State *L, int ud, const char *tname)
(`lauxlib.c:393`) Like `luaL_testudata`, but raises a type error on failure instead of returning NULL.

### luaL_checkoption(lua_State *L, int arg, const char *def, const char *const lst[])
(`lauxlib.c:411`) Checks that the string at `arg` (or `def` if absent) matches one of the null-terminated options in `lst`. Returns the match index or raises an `"invalid option"` error.

### luaL_checkstack(lua_State *L, int space, const char *msg)
(`lauxlib.c:433`) Ensures at least `space` extra stack slots. Raises `"stack overflow"` (with optional `msg` suffix) if `lua_checkstack` fails.

### luaL_checktype(lua_State *L, int arg, int t)
(`lauxlib.c:444`) Raises a type error if the value at `arg` does not have type `t`.

### luaL_checkany(lua_State *L, int arg)
(`lauxlib.c:451`) Raises `"value expected"` if there is no value at `arg` (type is `LUA_TNONE`).

### luaL_checklstring(lua_State *L, int arg, size_t *len)
(`lauxlib.c:459`) Returns the string at `arg` (converting numbers), setting `*len` to its length. Raises a string type error for non-string values.

### luaL_optlstring(lua_State *L, int arg, const char *def, size_t *len)
(`lauxlib.c:468`) Like `luaL_checklstring`, but returns `def` (with `strlen(def)` or 0 for NULL) when the argument is nil or absent.

### luaL_checknumber(lua_State *L, int arg)
(`lauxlib.c:481`) Returns the number at `arg` (converting numeric strings). Raises a number type error on failure.

### luaL_optnumber(lua_State *L, int arg, lua_Number def)
(`lauxlib.c:491`) Like `luaL_checknumber`, but returns `def` when the argument is absent.

### interror(lua_State *L, int arg)
(`lauxlib.c:498`) Static helper for integer-check errors. Distinguishes "number has no integer representation" from a plain type error.

### luaL_checkinteger(lua_State *L, int arg)
(`lauxlib.c:508`) Returns the integer at `arg` (converting numeric strings). Delegates to `interror` on failure.

### luaL_optinteger(lua_State *L, int arg, lua_Integer def)
(`lauxlib.c:519`) Like `luaL_checkinteger`, but returns `def` when the argument is absent.

### resizebox(lua_State *L, int idx, size_t newsize)
(`lauxlib.c:545`) Resizes the buffer inside a `UBox` userdata. Optimizes for the common no-change case. Uses the state's allocator directly.

### boxgc(lua_State *L)
(`lauxlib.c:565`) `__gc` / `__close` metamethod for buffer boxes: frees the boxed buffer by resizing to 0.

### getBoxMT(lua_State *L)
(`lauxlib.c:581`) Ensures the shared box metatable (`"_UBOX*"` in registry) exists, creating it on first access with `__gc` and `__close` entries.

### newbox(lua_State *L)
(`lauxlib.c:594`) Pushes a new `UBox` full userdata with the shared box metatable installed.

### newbuffsize(luaL_Buffer *B, size_t sz)
(`lauxlib.c:623`) Computes the new buffer size: grows by 1.5× or to `B->n + sz + 1`, whichever is larger. Raises an error on overflow.

### prepbuffsize(luaL_Buffer *B, size_t sz, int boxidx)
(`lauxlib.c:641`) Ensures at least `sz` free bytes in the buffer. Creates a `UBox` and copies data from the static area on first overflow. `boxidx` is the stack position of the box/placeholder.

### luaL_prepbuffsize(luaL_Buffer *B, size_t sz)
(`lauxlib.c:670`) Public entry to `prepbuffsize` with the box at stack position `-1`.

### luaL_addlstring(luaL_Buffer *B, const char *s, size_t l)
(`lauxlib.c:676`) Appends `l` bytes of `s` to the buffer, growing as needed.

### luaL_addstring(luaL_Buffer *B, const char *s)
(`lauxlib.c:686`) Appends the null-terminated string `s` to the buffer.

### luaL_pushresult(luaL_Buffer *B)
(`lauxlib.c:694`) Finalizes the buffer and pushes its contents as a Lua string. When the buffer grew into a `UBox`, the boxed memory is handed to Lua via `lua_pushexternalstring` (zero-copy transfer of ownership), and the box is closed via `lua_closeslot`. A GC step proportional to the result size is triggered.

### luaL_pushresultsize(luaL_Buffer *B, size_t sz)
(`lauxlib.c:722`) Declares `sz` additional bytes written, then calls `luaL_pushresult`.

### luaL_addvalue(luaL_Buffer *B)
(`lauxlib.c:737`) Appends the Lua string at the stack top to the buffer. Notably, the box is at `-2` (not `-1`) because the string being added sits above it.

### luaL_buffinit(lua_State *L, luaL_Buffer *B)
(`lauxlib.c:750`) Initializes buffer `B` with its internal static area and pushes a light userdata placeholder (the buffer address) onto the stack.

### luaL_buffinitsize(lua_State *L, luaL_Buffer *B, size_t sz)
(`lauxlib.c:761`) Initializes the buffer and returns a write area of at least `sz` bytes, creating a box immediately if the static area is too small.

### luaL_ref(lua_State *L, int t)
(`lauxlib.c:780`) Creates a reference in table `t` for the value at stack top. Uses a free-list linked through `t[1]`: freed reference slots point to the next free slot. Returns `LUA_REFNIL` for nil values, or a positive integer reference. Pushes nothing.

### luaL_unref(lua_State *L, int t, int ref)
(`lauxlib.c:809`) Releases reference `ref` in table `t`, inserting it at the head of the free list. Negative references are silently ignored.

### getF(lua_State *L, void *ud, size_t *size)
(`lauxlib.c:840`) `lua_Reader` for files: returns pre-read characters first, then reads `BUFSIZ`-sized blocks. Returns NULL at EOF.

### errfile(lua_State *L, const char *what, int fnameindex)
(`lauxlib.c:860`) Pushes a `"cannot what filename: strerror"` message and returns `LUA_ERRFILE`.

### skipBOM(FILE *f)
(`lauxlib.c:878`) Skips an optional UTF-8 BOM (`EF BB BF`) at file start. Returns the first non-BOM character.

### skipcomment(FILE *f, int *cp)
(`lauxlib.c:894`) Skips a BOM and an optional `#!` shebang line. Sets `*cp` to the first meaningful character.

### luaL_loadfilex(lua_State *L, const char *filename, const char *mode)
(`lauxlib.c:910`) Loads a chunk from a file (or stdin if `filename` is NULL). Handles BOM/`#!` skipping, reopens binary files in `"rb"` mode, and pushes the compiled function. Returns `LUA_OK` or `LUA_ERR*`/`LUA_ERRFILE`.

### getS(lua_State *L, void *ud, size_t *size)
(`lauxlib.c:963`) `lua_Reader` for in-memory buffers: returns the entire buffer in one call, then NULL.

### luaL_loadbufferx(lua_State *L, const char *buff, size_t size, const char *name, const char *mode)
(`lauxlib.c:975`) Loads a chunk from `size` bytes in `buff`, named `name`. Delegates to `lua_load` via `getS`.

### luaL_loadstring(lua_State *L, const char *s)
(`lauxlib.c:986`) Loads the null-terminated string `s` as a text chunk (mode `"t"`).

### luaL_getmetafield(lua_State *L, int obj, const char *event)
(`lauxlib.c:997`) Pushes the metafield `event` of the value at `obj`. Returns the field's type, or `LUA_TNIL` if no metatable or the field is nil (nothing pushed in that case).

### luaL_callmeta(lua_State *L, int obj, const char *event)
(`lauxlib.c:1015`) Calls the `event` metamethod of the value at `obj` with `obj` as argument. Returns 1 and leaves the result on the stack, or 0 if no metafield exists.

### luaL_len(lua_State *L, int idx)
(`lauxlib.c:1027`) Returns the integer length of the value at `idx` (via `__len`). Raises an error if the length is not an integer.

### luaL_tolstring(lua_State *L, int idx, size_t *len)
(`lauxlib.c:1042`) Converts the value at `idx` to a readable string. Tries `__tostring` first, then uses built-in forms for numbers/strings/booleans/nil, and falls back to `"type: 0xaddr"`. Returns the string pointer.

### luaL_setfuncs(lua_State *L, const luaL_Reg *l, int nup)
(`lauxlib.c:1085`) Registers the functions in `l` into the table at stack top, each as a closure with `nup` shared upvalues (popped from the top). Placeholder entries (`func == NULL`) become `false`.

### luaL_getsubtable(lua_State *L, int idx, const char *fname)
(`lauxlib.c:1106`) Ensures `stack[idx][fname]` is a table (creating it if absent) and pushes it. Returns 1 if it existed, 0 if newly created.

### luaL_requiref(lua_State *L, const char *modname, lua_CFunction openf, int glb)
(`lauxlib.c:1126`) Module loading: checks `_LOADED[modname]`, calls `openf` if not yet loaded, stores the result in `_LOADED`, and optionally registers it in `_G`. Leaves the module on the stack.

### luaL_addgsub(luaL_Buffer *b, const char *s, const char *p, const char *r)
(`lauxlib.c:1148`) Appends `s` to buffer `b`, replacing every occurrence of pattern `p` with replacement `r` (literal substitution, no pattern semantics).

### luaL_gsub(lua_State *L, const char *s, const char *p, const char *r)
(`lauxlib.c:1163`) Returns a new string with every `p` in `s` replaced by `r`, leaving the result on the stack.

### luaL_alloc(void *ud, void *ptr, size_t osize, size_t nsize)
(`lauxlib.c:1175`) Default `lua_Alloc`: thin wrapper over `realloc`. Frees when `nsize == 0`, ignores `ud` and `osize`.

### panic(lua_State *L)
(`lauxlib.c:1190`) Standard panic function: prints `"PANIC: unprotected error in call to Lua API"` to stderr. Returns 0 to abort.

### checkcontrol(lua_State *L, const char *message, int tocont)
(`lauxlib.c:1215`) Detects and handles warning control messages (`"@off"`, `"@on"`). Returns 1 if the message was a control message.

### warnfoff(void *ud, const char *message, int tocont)
(`lauxlib.c:1230`) Warning function when the system is off: only reacts to `@on`/`@off` control messages.

### warnfcont(void *ud, const char *message, int tocont)
(`lauxlib.c:1239`) Continuation warning function: writes the message fragment and transitions back to `warnfon` at the final part.

### warnfon(void *ud, const char *message, int tocont)
(`lauxlib.c:1253`) Warning function for new messages: handles control messages, prefixes `"Lua warning: "`, and delegates to `warnfcont`.

### luai_makeseed(void)
(`lauxlib.c:1285`) Generates a per-run pseudo-random seed from stack addresses (ASLR) and the current time. XOR-folds the buffer into an `unsigned int`.

### luaL_makeseed(lua_State *L)
(`lauxlib.c:1306`) Public wrapper around `luai_makeseed`. The `L` parameter is unused.

### luaL_newstate(void)
(`lauxlib.c:1318`) Creates a new Lua state with the default allocator (`luaL_alloc`), installs the standard `panic` handler and `warnfon` warning function. Returns NULL if memory allocation fails.

### luaL_checkversion_(lua_State *L, lua_Number ver, size_t sz)
(`lauxlib.c:1331`) Verifies the library was built against a compatible Lua core by checking `LUAL_NUMSIZES` and `lua_version`. Raises an error on mismatch. Called through the `luaL_checkversion` macro at library load time.
