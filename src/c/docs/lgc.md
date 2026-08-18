# lgc.c — Incremental and Generational Garbage Collector

> **AI-Generated Documentation**

## Overview

`lgc.c` implements the garbage collector (GC) for the Eira Lua 5.5 dialect runtime. It supports two collection modes: **incremental** (mark-sweep, one step at a time) and **generational** (young-generation minor collections with periodic major collections). The collector uses a **tri-color invariant** — objects are white (unvisited), gray (visited but children untraversed), or black (fully traversed) — and enforces the rule that a black object never points directly to a white one.

The file is organized into four logical sections: **generic helpers** (object sizing, linking, barriers, allocation), **mark functions** (root marking, gray-list management, ephemeron convergence), **sweep functions** (dead-object reclamation, weak-table clearing), and **GC control** (state machine stepping, mode switching, full collections). The state machine progresses through states defined in `lgc.h`: `GCSpause` → `GCSpropagate` → `GCSenteratomic` → `GCSatomic` → `GCSswpallgc` → `GCSswpfinobj` → `GCSswptobefnz` → `GCSswpend` → `GCScallfin` → back to `GCSpause`.

Key data structures include `global_State` (holds all GC lists — `allgc`, `finobj`, `tobefnz`, `fixedgc`, gray lists, generational bookmarks like `survival`, `old1`, `reallyold`), `GCObject` (common header with `marked` bits and `next` pointer), and the generational age field encoded in the lowest 3 bits of `marked` (`G_NEW`, `G_SURVIVAL`, `G_OLD0`, `G_OLD1`, `G_OLD`, `G_TOUCHED1`, `G_TOUCHED2`).

## Key Types / Macros

- **`GCSpropagate` .. `GCSpause`** — The nine GC states; `issweepphase(g)` covers `GCSswpallgc` through `GCSswpend`.
- **`G_NEW`..`G_TOUCHED2`** — Object ages in generational mode, stored in the low 3 bits of `marked`.
- **`WHITE0BIT`, `WHITE1BIT`, `BLACKBIT`** — Bit positions in `marked`; white is actually *two* bits so the collector can flip "current white" each cycle.
- **`maskcolors`, `maskgcbits`** — Bitmasks covering the color and age bits respectively.
- **`makewhite(g,x)`** — Erase all color bits and set the current white.
- **`set2gray(x)`, `set2black(x)`** — Paint an object gray or black from any starting color.
- **`CWUFIN`** — Work-unit cost charged for running one finalizer (10 units).
- **`GCSWEEPMAX`** — Maximum objects swept per single step (20).

## Functions

### objsize(GCObject *o)

Returns the memory size (in bytes) of a collectable object, dispatching by type. Used for GC accounting (`GCmarked` counters). Each type has its own sizing formula — e.g. `sizeLclosure(nupvalues)` for Lua closures, `sizeudata(nuvalue, len)` for userdata, `luaS_sizelngstr` for long strings.

### getgclist(GCObject *o)

Returns the address of the `gclist` field for object types that participate in gray lists (tables, closures, threads, protos, userdata). This allows generic gray-list operations without knowing the concrete type.

### linkgclist_(GCObject *o, GCObject **pnext, GCObject **list)

Prepends object `o` to a gray list and paints it gray. Asserts the object is not already gray. The macro wrappers `linkgclist` and `linkobjgclist` provide type-safe access.

### clearkey(Node *n)

For an empty table entry, marks the key as dead so the collectable key object can be reclaimed while the slot (with its chain position) remains intact.

### iscleared(global_State *g, const GCObject *o)

Determines whether a GC object should be removed from a weak table. Strings and NULL are never cleared (strings behave as values); other objects are cleared when they are white.

### luaC_barrier_(lua_State *L, GCObject *o, GCObject *v)

**Forward barrier**: called when a black object `o` acquires a reference to a white object `v`. During the mark phase it marks `v` (aging it `G_OLD0` in generational mode); during sweep it repaints `o` white to avoid repeated barrier hits.

### luaC_barrierback_(lua_State *L, GCObject *o)

**Backward barrier**: called when a black object `o` has been mutated to reference a new white value. Paints `o` gray and links it to `grayagain` for re-traversal. In generational mode, ages `o` to `G_TOUCHED1`.

### luaC_fix(lua_State *L, GCObject *o)

Pins object `o` by removing it from `allgc` and moving it to the `fixedgc` list. The object becomes permanently gray and old. Must be called when `o` is the head of `allgc`.

### luaC_newobjdt(lua_State *L, lu_byte tt, size_t sz, size_t offset)

Allocates a collectable object of type `tt` with byte size `sz`. The object is painted white and prepended to the `allgc` list. The `offset` parameter skips bytes so that the returned pointer points inside the allocation block (used when header data precedes the object).

### luaC_newobj(lua_State *L, lu_byte tt, size_t sz)

Convenience wrapper: same as `luaC_newobjdt` with offset 0.

### reallymarkobject(global_State *g, GCObject *o)

The core mark routine. Charges `objsize(o)` to `GCmarked`, then: strings and closed upvalues are blackened immediately; open upvalues stay gray; userdata with user values, closures, tables, threads, and prototypes are linked to the `gray` list for later traversal.

### markmt(global_State *g)

Marks all basic-type metatables (roots of the GC).

### markbeingfnz(global_State *g)

Marks every object on the `tobefnz` list so they survive the current cycle.

### remarkupvals(global_State *g)

For each thread in the `twups` list, simulates a barrier between its open upvalues and their values. Prunes threads that are already marked or have no open upvalues.

### cleargraylists(global_State *g)

Resets all five gray lists (`gray`, `grayagain`, `weak`, `allweak`, `ephemeron`) to NULL at the start of a cycle.

### restartcollection(global_State *g)

Seeds a new collection: clears gray lists, resets `GCmarked` to 0, and marks the root set (main thread, registry, metatables, finalizable leftovers).

### genlink(global_State *g, GCObject *o)

Generational bookkeeping for a black object: `G_TOUCHED1` objects rejoin `grayagain`; `G_TOUCHED2` objects advance to `G_OLD`. No-op in incremental mode.

### traverseweakvalue(global_State *g, Table *h)

Visits a weak-values table: marks keys, detects white values for later clearing, and links the table to `grayagain` (propagate phase) or `weak`/`genlink` (atomic phase).

### traversearray(global_State *g, Table *h)

Marks collectable values in a table's array part. Returns whether anything was marked (drives ephemeron convergence).

### traverseephemeron(global_State *g, Table *h, int inv)

Visits an ephemeron table (weak keys). Values are only marked after their keys are marked. The `inv` flag alternates traversal direction to speed convergence on chains. Returns true if any value was newly marked.

### traversestrongtable(global_State *g, Table *h)

Marks every key and value in a strong (non-weak) table.

### getmode(global_State *g, Table *h)

Decodes the `__mode` metafield: returns a bitmask where bit 0 = weak values, bit 1 = weak keys.

### traversetable(global_State *g, Table *h)

Dispatches table traversal by weakness mode and returns a work estimate (nodes + array size).

### traverseudata(global_State *g, Udata *u)

Marks a userdata's metatable and user values; returns a work estimate.

### traverseproto(global_State *g, Proto *f)

Marks a prototype's source string, constants, upvalue names, nested prototypes, and local-variable names.

### traverseCclosure(global_State *g, CClosure *cl)

Marks the upvalues of a C closure.

### traverseLclosure(global_State *g, LClosure *cl)

Marks a Lua closure's prototype and upvalue objects.

### traversethread(global_State *g, lua_State *th)

Marks a coroutine's live stack slice and open upvalues. In the atomic phase, nils out the dead stack tail and may re-link the thread to `twups`. Returns a work estimate.

### propagatemark(global_State *g)

Pops one object from the `gray` list, blackens it, and dispatches to the appropriate traverse function. Returns a work estimate.

### propagateall(global_State *g)

Drains the entire `gray` list by repeatedly calling `propagatemark`.

### convergeephemerons(global_State *g)

Iterates all ephemeron tables, alternating traversal direction, propagating key→value marks until a full pass marks nothing new.

### clearbykeys(global_State *g, GCObject *l)

Drops entries with collected (white) keys from every table in a weak-key list.

### clearbyvalues(global_State *g, GCObject *l, GCObject *f)

Drops entries with collected values from weak-value tables in the range `[l, f)`.

### freeupval(lua_State *L, UpVal *uv)

Frees an upvalue, unlinking it from its thread's open-upvalue chain if still open.

### freeobj(lua_State *L, GCObject *o)

Frees one object by type, running per-type cleanup (e.g. short strings remove themselves from the string table; long strings with external allocators call `falloc` to release).

### sweeplist(lua_State *L, GCObject **p, l_mem countin)

Sweeps up to `countin` objects: dead (old-white) objects are freed; survivors are repainted with the new white and age `G_NEW`. Returns a pointer to resume from, or NULL when the list is exhausted.

### sweeptolive(lua_State *L, GCObject **p)

Sweeps one object at a time until `p` points at a live object or NULL.

### checkSizes(lua_State *L, global_State *g)

Shrinks the string table when `nuse < size / 4`.

### udata2finalize(global_State *g)

Takes the next userdata off `tobefnz`, returns it to `allgc`, clears `FINALIZEDBIT`, and (during sweep) repaints it white.

### dothecall(lua_State *L, void *ud)

Trampoline that calls `luaD_callnoyield` for a finalizer.

### GCTM(lua_State *L)

Calls one `__gc` metamethod in a protected, isolated call with GC steps and hooks disabled. Errors are reported as warnings.

### callallpendingfinalizers(lua_State *L)

Runs every pending finalizer to completion.

### findlast(GCObject **p)

Returns the address of the trailing `next` field (append point) of a linked list.

### separatetobefnz(global_State *g, int all)

Moves white (or, with `all`, every) finalizable object from `finobj` to `tobefnz`.

### checkpointer(GCObject **p, GCObject *o)

If bookmark `*p` points at `o`, advances it before `o` is unlinked.

### correctpointers(global_State *g, GCObject *o)

Keeps generational bookmarks (`survival`, `old1`, `reallyold`, `firstold1`) valid when an object leaves `allgc`.

### luaC_checkfinalizer(lua_State *L, GCObject *o, Table *mt)

Registers `o` for finalization: moves it from `allgc` to `finobj` if its metatable has a `__gc` entry. During sweep, adjusts `sweepgc` to avoid dangling pointers.

### setpause(global_State *g)

Arms the GC debt so the next cycle starts after bytes grow by roughly `GCmarked * pause / 100`.

### sweep2old(lua_State *L, GCObject **p)

Sweeps for entering generational mode: frees whites, ages survivors to `G_OLD`, keeps threads and open upvalues on gray lists.

### sweepgen(lua_State *L, global_State *g, GCObject **p, GCObject *limit, GCObject **pfirstold1, l_mem *paddedold)

Minor-cycle sweep: frees dead objects, advances survivor ages per the `nextage` table, tallies bytes becoming `OLD1`.

### correctgraylist(GCObject **p)

Post-sweep fix-up of one gray list: drops white objects, advances `TOUCHED1`/`TOUCHED2`, keeps threads. Returns the tail element.

### correctgraylists(global_State *g)

Fixes up and merges every gray list (`grayagain`, `weak`, `allweak`, `ephemeron`) into `grayagain`.

### markold(global_State *g, GCObject *from, GCObject *to)

Promotes `OLD1` objects in a list range to `OLD`, re-marking any black ones so their references are revisited.

### finishgencycle(lua_State *L, global_State *g)

Finishes a minor collection: fixes gray lists, shrinks the string table, runs pending finalizers if the stack allows.

### minor2inc(lua_State *L, global_State *g, lu_byte kind)

Switches from minor to major (incremental) collections, saving the live byte count and resetting generational bookmarks.

### checkminormajor(global_State *g)

Returns true when enough bytes became old to justify switching from minor to major collections.

### youngcollection(lua_State *L, global_State *g)

Runs a full young (minor) collection: promotes `OLD1` objects, does the atomic step, sweeps nursery/survival lists, then decides whether to go major.

### atomic2gen(lua_State *L, global_State *g)

After the atomic phase, sweeps everything into the old generation and enters minor mode.

### setminordebt(global_State *g)

Arms the debt so the next minor cycle fires after `GENMINORMUL%` growth relative to the last major collection.

### entergen(lua_State *L, global_State *g)

Reaches a clean atomic state via incremental cycles, then makes all live objects old and starts minor collections.

### luaC_changemode(lua_State *L, int newmode)

Switches the collector between incremental and generational modes.

### fullgen(lua_State *L, global_State *g)

Full collection in generational mode: drops to incremental, runs a full cycle, re-enters generational.

### checkmajorminor(lua_State *L, global_State *g)

After an incremental major step, returns to minor collections if the collectable bytes exceed the `majorminor` threshold.

### entersweep(lua_State *L)

Enters the first sweep state, advancing `sweepgc` to the first live object.

### deletelist(lua_State *L, GCObject *p, GCObject *limit)

Frees every object from `p` up to (not including) `limit`.

### luaC_freeallobjects(lua_State *L)

Shuts down the collector: stops new finalizers, runs all pending ones, frees everything except the main thread and fixed objects.

### atomic(lua_State *L)

The stop-the-world atomic phase: re-marks roots, propagates all gray objects, resolves ephemerons, clears weak tables, separates finalizable objects, resurrects them, and flips the current white color.

### sweepstep(lua_State *L, global_State *g, lu_byte nextstate, GCObject **nextlist, int fast)

One sweep step: `GCSWEEPMAX` objects at a time (or the whole list when `fast`), advancing to `nextstate` once the current list is exhausted.

### singlestep(lua_State *L, int fast)

Executes one unit of GC work in the current state. Returns work done or sentinels (`step2pause`, `step2minor`, `atomicstep`).

### luaC_runtilstate(lua_State *L, int state, int fast)

Keeps stepping until the collector reaches `state`. Used to drive full collections to specific phases.

### incstep(lua_State *L, global_State *g)

One incremental step: converts the byte budget into work units, runs single steps until the budget is spent or a cycle ends, then sets the next debt.

### luaC_step(lua_State *L)

Public GC tick called after allocations. No-op when stopped; otherwise runs one incremental or minor step.

### fullinc(lua_State *L, global_State *g)

Full incremental collection: normalizes colors if needed, then runs pause→finalizers→pause.

### luaC_fullgc(lua_State *L, int isemergency)

Runs a complete collection in the current mode. In emergency mode, finalizers and shrinking are skipped.
