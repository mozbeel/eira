# lmathlib.c — Standard mathematical library for Eira

> **AI-Generated Documentation**

## Overview

`lmathlib.c` implements the `math` standard library, exposing trigonometric functions, rounding, number-theoretic utilities, and a pseudo-random number generator to Eira scripts. The library is registered as a single module via `luaopen_math`, which populates constants (`math.pi`, `math.huge`, `math.maxinteger`, `math.mininteger`) and installs closures for `math.random` / `math.randomseed` that share a single `RanState` upvalue.

The PRNG is based on the **xoshiro256\*\*** algorithm, operating on four 64-bit words of state. The implementation provides both a native 64-bit path (using `unsigned long` or `unsigned long long`) and a portable fallback that emulates 64-bit arithmetic via a two-word struct (`{h, l}`). Projection of raw random bits into a user-requested interval uses Mersenne-number masking with rejection sampling (`project`), ensuring uniformity even when the interval size is not a power of two.

Functions that deal with both integers and floats (e.g. `math.floor`, `math.ceil`, `math.modf`) carefully distinguish the two Lua numeric types: integers pass through unchanged for unary operations, while floats are converted through C math macros prefixed with `l_mathop`. The `math.abs` function negates integers through an unsigned cast to avoid undefined behaviour on `LUA_MININTEGER`.

Conditional compilation (`LUA_COMPAT_MATHLIB`) keeps deprecated functions (`math.pow`, `math.log10`, hyperbolic trig) available for backward compatibility. The entire library is self-contained at 841 lines with no dependencies beyond the standard C library.

## Functions

### math_abs(n)

Implements `math.abs`. For integer arguments, negation is performed through an unsigned intermediate (`0u - (lua_Unsigned)n`) so that `LUA_MININTEGER` does not overflow. Float arguments delegate to `fabs` via the `l_mathop` macro.

### math_sin(x)

Implements `math.sin`. Pushes the sine of its numeric argument, computed by the platform's `sin` function through `l_mathop`.

### math_cos(x)

Implements `math.cos`. Pushes the cosine of its numeric argument.

### math_tan(x)

Implements `math.tan`. Pushes the tangent of its numeric argument.

### math_asin(x)

Implements `math.asin`. Pushes the arc sine of its numeric argument.

### math_acos(x)

Implements `math.acos`. Pushes the arc cosine of its numeric argument.

### math_atan(y [, x])

Implements `math.atan`. When called with one argument, behaves as `atan(y, 1)` (matching the old single-argument `math.atan`). The two-argument form delegates to `atan2(y, x)`.

### math_toint(v)

Implements `math.tointeger`. Attempts to convert its argument to an integer via `lua_tointegerx`. Returns the integer on success; returns `fail` (nil) when the value is not convertible. This is a lossless check — no rounding is performed.

### math_floor(n)

Implements `math.floor`. Integers are returned as-is. Floats are floored via C's `floor` and then pushed as an integer when the result fits, using the `pushnumint` helper.

### math_ceil(n)

Implements `math.ceil`. Symmetric to `math_floor`: integers pass through, floats are ceiled and converted to integer when possible.

### math_fmod(m, n)

Implements `math.fmod`. When both operands are integers, it uses the `%` operator with special-case handling for divisors of 0 (error) and -1 (returns 0 to avoid overflow on `LUA_MININTEGER`). Float operands delegate to C's `fmod`.

### math_modf(n)

Implements `math.modf`. Returns two values: the integer part (truncated toward zero) and the fractional part. Integers return themselves plus `0.0`. The integer part is computed via `ceil` for negatives and `floor` for positives rather than using `modf`, avoiding pointer-type issues when `lua_Number` is not `double`.

### math_sqrt(x)

Implements `math.sqrt`. Pushes the square root of its numeric argument.

### math_ult(m, n)

Implements `math.ult`. Performs an unsigned less-than comparison of two integers, pushing a boolean result. This is useful for ordering values where signed comparison would be incorrect.

### math_log(x [, base])

Implements `math.log`. Returns the natural logarithm when `base` is absent. Provides fast paths for base 2 (`log2`) and base 10 (`log10`) when the platform supports them. Other bases use the change-of-base formula `log(x) / log(base)`.

### math_exp(x)

Implements `math.exp`. Pushes *e* raised to the power of its argument.

### math_deg(x)

Implements `math.deg`. Converts radians to degrees by multiplying by `180 / pi`.

### math_rad(x)

Implements `math.rad`. Converts degrees to radians by multiplying by `pi / 180`.

### math_frexp(x)

Implements `math.frexp`. Returns two values: the mantissa (in the range [0.5, 1)) and the binary exponent such that `x == mantissa * 2^exponent`.

### math_ldexp(m, e)

Implements `math.ldexp`. Returns `m * 2^e`, the inverse of `frexp`.

### math_min(...)

Implements `math.min`. Returns the smallest of its arguments. Requires at least one argument. Uses `lua_compare` with `LUA_OPLT`, so it honours `__lt` metamethods.

### math_max(...)

Implements `math.max`. Returns the largest of its arguments. Requires at least one argument. Uses `lua_compare` with `LUA_OPLT`.

### math_type(x)

Implements `math.type`. Returns the string `"integer"` or `"float"` for number arguments, or `fail` for non-numeric values.

### rotl(x, n) / nextrand(state)

Internal helpers for the xoshiro256\*\* PRNG. `rotl` rotates a 64-bit word left by *n* bits. `nextrand` advances the four-word state array and returns the scrambled output. Two complete implementations exist: one for native 64-bit types and a portable two-32-bit-word struct fallback.

### I2d(x)

Converts the top `FIGS` random bits into a `lua_Number` in the half-open interval [0, 1). The 64-bit path uses a signed intermediate for compatibility with older compilers; the two-word path combines bits from both halves.

### project(ran, n, state)

Projects a random unsigned integer into the closed interval [0, n] using Mersenne-number masking and rejection sampling. Computes the smallest Mersenne number ≥ *n*, masks the random bits, and re-draws when the result falls outside the valid range. This guarantees uniformity.

### math_random([low [, up]])

Implements `math.random`. With no arguments, returns a float in [0, 1). With one argument *n*, returns an integer in [1, *n*] (or a full-range random integer if *n* == 0). With two arguments, returns an integer in [*low*, *up*] via `project`.

### setseed(L, state, n1, n2)

Initialises the 4-word PRNG state from two seeds. State word 1 is set to `0xff` to avoid a zero state. After seeding, 16 outputs are discarded to spread the entropy. Pushes the two seed values as return values.

### math_randomseed([x [, y]])

Implements `math.randomseed`. Without arguments, generates a time-based seed via `luaL_makeseed`. With one or two integer arguments, uses those as the seed. Returns the two seed values used.

### setrandfunc(L)

Allocates a `RanState` as Lua userdata, seeds it with `luaL_makeseed`, and registers `math.random` and `math.randomseed` as closures sharing that state via upvalue index 1.

### luaopen_math(L)

Opens the `math` library. Creates the function table, fills the placeholder constants (`pi`, `huge`, `maxinteger`, `mininteger`), and calls `setrandfunc` to install the PRNG functions. Returns the library table.
