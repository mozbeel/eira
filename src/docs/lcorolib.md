# lcorolib.c — Eira Coroutine Library

> **AI-Generated Documentation**

## Overview

This file implements the **coroutine library** for the Eira Lua 5.5 dialect, exposed as the `coroutine` table. Coroutines are a core feature of Lua that enable cooperative multitasking: each coroutine has its own execution stack and can be suspended and resumed independently. The coroutine library provides functions to create, resume, yield, inspect, and close coroutines.

The library exposes eight functions: `coroutine.create`, `coroutine.resume`, `coroutine.yield`, `coroutine.wrap`, `coroutine.status`, `coroutine.isyieldable`, `coroutine.running`, and `coroutine.close`. The implementation centers around a core resume helper (`auxresume`) that marshals arguments and results between the calling thread and the coroutine thread, and a state classification helper (`auxstatus`) that maps the internal `lua_status` plus stack inspection into one of four observable states.

A notable design aspect is the `coroutine.wrap` function, which returns a C closure instead of a thread object. This closure resumes the hidden coroutine on each call and turns errors into Lua exceptions (unlike `coroutine.resume`, which returns a boolean + message pair). The `coroutine.close` function runs to-be-closed variables on a suspended or dead coroutine, with special handling to prevent closing the main thread or a coroutine that is currently running (since it closes itself via a longjmp).

## Functions

### `getco(lua_State *L)`

Helper that fetches the coroutine (thread) given as the first stack argument. Raises a type error if the argument is not a thread. Returns the `lua_State *` of the coroutine.

### `auxresume(lua_State *L, lua_State *co, int narg)`

Core resume machinery. Moves `narg` values from the caller's stack `L` to the coroutine `co`, resumes it with `lua_resume`, then moves the results back to `L`. Returns the number of results on success, or `-1` with the error message left on `L`'s stack on failure.

Checks `lua_checkstack` on both the coroutine (for arguments) and the caller (for results) to prevent stack overflow. This is critical because `L1` can be in any state and its stack space is not guaranteed.

```c
lua_xmove(L, co, narg);
status = lua_resume(co, L, narg, &nres);
if (l_likely(status == LUA_OK || status == LUA_YIELD)) {
```

### `luaB_coresume(lua_State *L)`

Implements `coroutine.resume(co, ...)`. Calls `auxresume` and prepends a boolean to the outcome: `true` plus the results on success, or `false` plus the error message on failure. This is the safe, non-throwing interface for resuming coroutines.

### `luaB_auxwrap(lua_State *L)`

The C closure returned by `coroutine.wrap`. Retrieves the coroutine from its first upvalue, calls `auxresume`, and on error: if the coroutine died, it closes its to-be-closed variables via `lua_closethread`, adds source location info if the error is a string, and then propagates the error via `lua_error`. On success, returns the yielded values directly.

```c
lua_State *co = lua_tothread(L, lua_upvalueindex(1));
int r = auxresume(L, co, lua_gettop(L));
if (l_unlikely(r < 0)) {
  int stat = lua_status(co);
```

### `luaB_cocreate(lua_State *L)`

Implements `coroutine.create(f)`. Creates a new thread via `lua_newthread` and moves the given function `f` from the caller's stack into the new thread as its body. Returns the new thread.

### `luaB_cowrap(lua_State *L)`

Implements `coroutine.wrap(f)`. Creates a coroutine via `luaB_cocreate`, then wraps it in a C closure (`luaB_auxwrap`) with the thread as its sole upvalue. Returns the closure. Errors from this closure propagate as exceptions rather than boolean pairs.

### `luaB_yield(lua_State *L)`

Implements `coroutine.yield(...)`. Suspends the running coroutine and passes all arguments as the results of the matching `coroutine.resume` call. Delegates directly to `lua_yield`.

### `auxstatus(lua_State *L, lua_State *co)`

Classifies a coroutine into one of four states: `COS_RUN` (running), `COS_DEAD` (dead), `COS_YIELD` (suspended), or `COS_NORM` (normal — alive but not the current running thread). Distinguishes "normal" from "suspended" by checking `lua_getstack` to see if the coroutine has active stack frames, and distinguishes "dead" from "suspended" (initial state) by checking if the stack is empty.

```c
if (L == co) return COS_RUN;
else {
  switch (lua_status(co)) {
    case LUA_YIELD: return COS_YIELD;
    case LUA_OK: {
      lua_Debug ar;
      if (lua_getstack(co, 0, &ar)) return COS_NORM;
```

### `luaB_costatus(lua_State *L)`

Implements `coroutine.status(co)`. Returns the state of the given coroutine as one of the strings `"running"`, `"dead"`, `"suspended"`, or `"normal"`.

### `getoptco(lua_State *L)`

Helper that returns the thread given as the first argument, or the current thread `L` when no argument is provided. Used by `isyieldable`, `running`, and `close`.

### `luaB_yieldable(lua_State *L)`

Implements `coroutine.isyieldable([co])`. Returns a boolean indicating whether yielding from the given coroutine (or the current one) is allowed.

### `luaB_corunning(lua_State *L)`

Implements `coroutine.running()`. Returns the currently running thread and a boolean indicating whether it is the main thread.

### `luaB_close(lua_State *L)`

Implements `coroutine.close([co])`. Runs the to-be-closed variables of a suspended or dead coroutine via `lua_closethread`. On success returns `true`; on failure returns `false` plus the error message. Rejects closing a "normal" coroutine (one that is suspended in a nested call chain) and the main thread. When called on the running coroutine (self-close), it delegates to `lua_closethread` which performs the close via longjmp.

```c
case COS_DEAD: case COS_YIELD: {
  status = lua_closethread(co, L);
  if (status == LUA_OK) {
    lua_pushboolean(L, 1);
    return 1;
  }
```

### `luaopen_coroutine(lua_State *L)`

Opens the coroutine library. Creates and returns the `coroutine` table containing all eight library functions.
