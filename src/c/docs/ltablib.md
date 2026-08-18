# ltablib.c — Library for Table Manipulation

> **AI-Generated Documentation**

## Overview

`ltablib.c` implements the `table` standard library, providing Eira scripts with functions to create, insert, remove, move, concatenate, pack, unpack, and sort tables. The library is registered via `luaopen_table`, which simply creates the function table — no constants or extra state are required.

The sorting implementation is a quicksort based on Robert Sedgewick's algorithms, featuring randomized pivot selection for large arrays to avoid quadratic behaviour on sorted input. The partition step uses the stack to hold the pivot, and the recursive function (`auxsort`) always recurses on the smaller partition first, limiting stack depth to O(log n). When an imbalance ratio of 128:1 is detected, the pivot randomization seed is refreshed.

Most functions accept table-like objects (tables with `__index`, `__newindex`, and `__len` metamethods), validated by the `checktab` helper. The `aux_getn` macro combines metamethod validation with `luaL_len` to get the effective sequence length. The file is 463 lines.

## Functions

### checkfield(L, key, n)

Internal helper: raw-gets `key` from the table at stack index `n`, returning whether the result is non-nil. Used by `checktab` to probe for required metamethods (`__index`, `__newindex`, `__len`).

### checktab(L, arg, what)

Validates that argument `arg` is either a table or a table-like object with the required metamethods specified by the `what` bitmask (`TAB_R` for `__index`, `TAB_W` for `__newindex`, `TAB_L` for `__len`). Strings are exempt from the `__len` check since they have an intrinsic length. Raises a type error if the check fails.

### tcreate(n [, r])

Implements `table.create`. Pre-allocates a new table with room for `n` sequence elements and `r` other (hash) elements, capping both at `INT_MAX`. Returns the new empty table.

### tinsert(list [, pos], v)

Implements `table.insert`. With two arguments (list, value), appends the value at the end of the sequence. With three arguments (list, pos, value), shifts elements from `pos` to `#list` up by one position to make room, then stores the value. Raises if `pos` is outside the range `[1, #list + 1]`.

### tremove(list [, pos])

Implements `table.remove`. Removes the element at position `pos` (default: `#list`, the last element) by shifting all subsequent elements down by one. Returns the removed element. Raises if `pos` is outside the valid range.

### tmove(a1, f, e, t [, a2])

Implements `table.move`. Copies elements `a1[f]` through `a1[e]` into `a2[t], a2[t+1], ...`. When `a2` is omitted, copies within the same table. When the source and destination ranges do not overlap (or are in different tables), copies forward for better rehashing; copies backward when they overlap in the same direction.

### addfield(L, b, i)

Internal helper: gets element `t[i]` and appends it to the buffer `b`. Raises an error if the element is not a string (or coercible to one). Used by `tconcat`.

### tconcat(list [, sep [, i [, j]]])

Implements `table.concat`. Concatenates elements `list[i]` through `list[j]` (default: `i = 1`, `j = #list`) with the separator string `sep` (default: `""`) between them. Returns the resulting string. All elements in the range must be strings.

### tpack(...)

Implements `table.pack`. Creates a new table containing all arguments as elements `1..n` and sets the field `n` to the count of arguments. Returns the table.

### tunpack(list [, i [, j]])

Implements `table.unpack`. Returns elements `list[i]` through `list[j]` (default: `i = 1`, `j = #list`) as multiple return values. Checks that the stack can hold all results, raising an error for excessively large ranges.

### set2(L, i, j)

Internal helper for quicksort: swaps the elements at indices `i` and `j` using the stack top as temporary storage.

### sort_comp(L, a, b)

Internal helper: compares two values on the stack. With no comparator function (index 2 is nil), uses Lua's `<` operator. With a comparator, calls it with the two values and returns the boolean result. Compensates stack indices for the pushed function.

### partition(L, lo, up)

Internal quicksort helper: partitions the range `[lo..up]` around the pivot (kept on the stack top). Returns the final index of the pivot. Raises an error if the order function is inconsistent (e.g. `a < b` and `b < a` both true).

### choosePivot(lo, up, rnd)

Internal quicksort helper: selects a pivot in the middle quarters of `[lo, up]`, pseudo-randomized by `rnd` (XORed with the bounds). This avoids worst-case O(n^2) behaviour on already-sorted input.

### auxsort(L, lo, up, rnd)

Internal recursive quicksort. Sorts `[lo..up]` with tail-call optimization (looping instead of recursing for the larger partition). Sorts the two- and three-element base cases directly. For larger ranges, picks a pivot via `choosePivot`, partitions, then recurses on the smaller half. When a partition is too imbalanced (ratio > 128:1), re-randomizes the pivot seed.

### sort(list [, comp])

Implements `table.sort`. Validates the optional comparator function, then quicksorts the sequence `list[1..#list]` in place. Returns nothing. Raises if the array is too large (≥ `INT_MAX`) or the comparator is not a function.
