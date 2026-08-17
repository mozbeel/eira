/*
** $Id: ldump.c $
** save precompiled Lua chunks
** See Copyright Notice in lua.h
*/

#define ldump_c
#define LUA_CORE

#include "lprefix.h"


#include <limits.h>
#include <stddef.h>

#include "lua.h"

#include "lapi.h"
#include "lgc.h"
#include "lobject.h"
#include "lstate.h"
#include "ltable.h"
#include "lundump.h"


typedef struct {
  lua_State *L;
  lua_Writer writer;
  void *data;
  size_t offset;  /* current position relative to beginning of dump */
  int strip;
  int status;
  Table *h;  /* table to track saved strings */
  lua_Unsigned nstr;  /* counter for counting saved strings */
} DumpState;


// AI: dumpVector is the single choke point for all data, so endianness can be changed in just this one macro.

/*
** All high-level dumps go through dumpVector; you can change it to
** change the endianness of the result
*/
#define dumpVector(D,v,n)	dumpBlock(D,v,(n)*sizeof((v)[0]))

#define dumpLiteral(D, s)	dumpBlock(D,s,sizeof(s) - sizeof(char))


// AI: Writes 'size' bytes through the writer, tracking 'offset'; after any error it stops writing. The final NULL/0 call signals end-of-dump.

/*
** Dump the block of memory pointed by 'b' with given 'size'.
** 'b' should not be NULL, except for the last call signaling the end
** of the dump.
*/
static void dumpBlock (DumpState *D, const void *b, size_t size) {
  if (D->status == 0) {  /* do not write anything after an error */
    lua_unlock(D->L);
    D->status = (*D->writer)(D->L, b, size, D->data);
    lua_lock(D->L);
    D->offset += size;
  }
}


// AI: Pads with zeros so the dump offset is a multiple of 'align' (used before aligned code/struct arrays).

/*
** Dump enough zeros to ensure that current position is a multiple of
** 'align'.
*/
static void dumpAlign (DumpState *D, unsigned align) {
  unsigned padding = align - cast_uint(D->offset % align);
  if (padding < align) {  /* padding == align means no padding */
    static lua_Integer paddingContent = 0;
    lua_assert(align <= sizeof(lua_Integer));
    dumpBlock(D, &paddingContent, padding);
  }
  lua_assert(D->offset % align == 0);
}


#define dumpVar(D,x)		dumpVector(D,&x,1)


// AI: Dumps a single byte (any value 0-255 passed as an int).
static void dumpByte (DumpState *D, int y) {
  lu_byte x = (lu_byte)y;
  dumpVar(D, x);
}


/*
** size for 'dumpVarint' buffer: each byte can store up to 7 bits.
** (The "+6" rounds up the division.)
*/
#define DIBS    ((l_numbits(lua_Unsigned) + 6) / 7)

// AI: MSB-varint: 7 payload bits per byte, high bit marks "more bytes", stored most-significant byte first.

/*
** Dumps an unsigned integer using the MSB Varint encoding
*/
static void dumpVarint (DumpState *D, lua_Unsigned x) {
  lu_byte buff[DIBS];
  unsigned n = 1;
  buff[DIBS - 1] = x & 0x7f;  /* fill least-significant byte */
  while ((x >>= 7) != 0)  /* fill other bytes in reverse order */
    buff[DIBS - (++n)] = cast_byte((x & 0x7f) | 0x80);
  dumpVector(D, buff + DIBS - n, n);
}


// AI: Dumps a size_t as a varint.
static void dumpSize (DumpState *D, size_t sz) {
  dumpVarint(D, cast(lua_Unsigned, sz));
}


// AI: Dumps a non-negative int as a varint.
static void dumpInt (DumpState *D, int x) {
  lua_assert(x >= 0);
  dumpVarint(D, cast_uint(x));
}


// AI: Dumps a float in raw native memory format (the header records its size for the loader).
static void dumpNumber (DumpState *D, lua_Number x) {
  dumpVar(D, x);
}


// AI: Zig-zag style coding: n>=0 -> 2n, n<0 -> -2n-1, so small integers (including -1) stay small after varint encoding.

/*
** Signed integers are coded to keep small values small. (Coding -1 as
** 0xfff...fff would use too many bytes to save a quite common value.)
** A non-negative x is coded as 2x; a negative x is coded as -2x - 1.
** (0 => 0; -1 => 1; 1 => 2; -2 => 3; 2 => 4; ...)
*/
static void dumpInteger (DumpState *D, lua_Integer x) {
  lua_Unsigned cx = (x >= 0) ? 2u * l_castS2U(x)
                             : (2u * ~l_castS2U(x)) + 1;
  dumpVarint(D, cx);
}


// AI: Dumps a string, deduplicating: size 0 + index reuses a previously dumped string (index 0 = NULL); else it dumps size+1 bytes and records it.

/*
** Dump a String. First dump its "size":
** size==0 is followed by an index and means "reuse saved string with
** that index"; index==0 means NULL.
** size>=1 is followed by the string contents with real size==size-1 and
** means that string, which will be saved with the next available index.
** The real size does not include the ending '\0' (which is not dumped),
** so adding 1 to it cannot overflow a size_t.
*/
static void dumpString (DumpState *D, TString *ts) {
  if (ts == NULL) {
    dumpVarint(D, 0);  /* will "reuse" NULL */
    dumpVarint(D, 0);  /* special index for NULL */
  }
  else {
    TValue idx;
    int tag = luaH_getstr(D->h, ts, &idx);
    if (!tagisempty(tag)) {  /* string already saved? */
      dumpVarint(D, 0);  /* reuse a saved string */
      dumpVarint(D, l_castS2U(ivalue(&idx)));  /* index of saved string */
    }
    else {  /* must write and save the string */
      TValue key, value;  /* to save the string in the hash */
      size_t size;
      const char *s = getlstr(ts, size);
      dumpSize(D, size + 1);
      dumpVector(D, s, size + 1);  /* include ending '\0' */
      D->nstr++;  /* one more saved string */
      setsvalue(D->L, &key, ts);  /* the string is the key */
      setivalue(&value, l_castU2S(D->nstr));  /* its index is the value */
      luaH_set(D->L, D->h, &key, &value);  /* h[ts] = nstr */
      /* integer value does not need barrier */
    }
  }
}


// AI: Dumps the instruction array, aligned so the raw instructions can be read back natively.
static void dumpCode (DumpState *D, const Proto *f) {
  dumpInt(D, f->sizecode);
  dumpAlign(D, sizeof(f->code[0]));
  lua_assert(f->code != NULL);
  dumpVector(D, f->code, cast_uint(f->sizecode));
}


static void dumpFunction (DumpState *D, const Proto *f);

// AI: Dumps all constants: count, then per constant a type byte and its value (nil/false/true carry no payload).
static void dumpConstants (DumpState *D, const Proto *f) {
  int i;
  int n = f->sizek;
  dumpInt(D, n);
  for (i = 0; i < n; i++) {
    const TValue *o = &f->k[i];
    int tt = ttypetag(o);
    dumpByte(D, tt);
    switch (tt) {
      case LUA_VNUMFLT:
        dumpNumber(D, fltvalue(o));
        break;
      case LUA_VNUMINT:
        dumpInteger(D, ivalue(o));
        break;
      case LUA_VSHRSTR:
      case LUA_VLNGSTR:
        dumpString(D, tsvalue(o));
        break;
      default:
        lua_assert(tt == LUA_VNIL || tt == LUA_VFALSE || tt == LUA_VTRUE);
    }
  }
}


// AI: Dumps the count of nested prototypes and each of them recursively.
static void dumpProtos (DumpState *D, const Proto *f) {
  int i;
  int n = f->sizep;
  dumpInt(D, n);
  for (i = 0; i < n; i++)
    dumpFunction(D, f->p[i]);
}


// AI: Dumps each upvalue descriptor (instack flag, index, kind) as three bytes; their names go in the debug section.
static void dumpUpvalues (DumpState *D, const Proto *f) {
  int i, n = f->sizeupvalues;
  dumpInt(D, n);
  for (i = 0; i < n; i++) {
    dumpByte(D, f->upvalues[i].instack);
    dumpByte(D, f->upvalues[i].idx);
    dumpByte(D, f->upvalues[i].kind);
  }
}


// AI: Dumps debug info (line info, locals, upvalue names); with 'strip' all of it is replaced by zero counts.
static void dumpDebug (DumpState *D, const Proto *f) {
  int i, n;
  n = (D->strip) ? 0 : f->sizelineinfo;
  dumpInt(D, n);
  if (f->lineinfo != NULL)
    dumpVector(D, f->lineinfo, cast_uint(n));
  n = (D->strip) ? 0 : f->sizeabslineinfo;
  dumpInt(D, n);
  if (n > 0) {
    /* 'abslineinfo' is an array of structures of int's */
    dumpAlign(D, sizeof(int));
    dumpVector(D, f->abslineinfo, cast_uint(n));
  }
  n = (D->strip) ? 0 : f->sizelocvars;
  dumpInt(D, n);
  for (i = 0; i < n; i++) {
    dumpString(D, f->locvars[i].varname);
    dumpInt(D, f->locvars[i].startpc);
    dumpInt(D, f->locvars[i].endpc);
  }
  n = (D->strip) ? 0 : f->sizeupvalues;
  dumpInt(D, n);
  for (i = 0; i < n; i++)
    dumpString(D, f->upvalues[i].name);
}


// AI: Serializes one Proto: header fields, code, constants, upvalues, nested protos, source name, and debug info.
static void dumpFunction (DumpState *D, const Proto *f) {
  dumpInt(D, f->linedefined);
  dumpInt(D, f->lastlinedefined);
  dumpByte(D, f->numparams);
  dumpByte(D, f->flag);
  dumpByte(D, f->maxstacksize);
  dumpCode(D, f);
  dumpConstants(D, f);
  dumpUpvalues(D, f);
  dumpProtos(D, f);
  dumpString(D, D->strip ? NULL : f->source);
  dumpDebug(D, f);
}


// AI: Dumps a native type's size followed by a sample value, letting the loader verify its own ABI matches.
#define dumpNumInfo(D, tvar, value)  \
  { tvar i = value; dumpByte(D, sizeof(tvar)); dumpVar(D, i); }


// AI: Writes the chunk header: signature, version, format, LUAC_DATA marker, and native size/sample of each numeric type.
static void dumpHeader (DumpState *D) {
  dumpLiteral(D, LUA_SIGNATURE);
  dumpByte(D, LUAC_VERSION);
  dumpByte(D, LUAC_FORMAT);
  dumpLiteral(D, LUAC_DATA);
  dumpNumInfo(D, int, LUAC_INT);
  dumpNumInfo(D, Instruction, LUAC_INST);
  dumpNumInfo(D, lua_Integer, LUAC_INT);
  dumpNumInfo(D, lua_Number, LUAC_NUM);
}


// AI: Public entry: serializes Proto 'f' through writer 'w'; returns the writer status (nonzero = write error).

/*
** dump Lua function as precompiled chunk
*/
int luaU_dump (lua_State *L, const Proto *f, lua_Writer w, void *data,
               int strip) {
  DumpState D;
  // AI: 'h' maps dumped strings to indices for dedup references; it is anchored on the stack to keep it alive.
  D.h = luaH_new(L);  /* aux. table to keep strings already dumped */
  sethvalue2s(L, L->top.p, D.h);  /* anchor it */
  L->top.p++;
  D.L = L;
  D.writer = w;
  D.offset = 0;
  D.data = data;
  D.strip = strip;
  D.status = 0;
  D.nstr = 0;
  dumpHeader(&D);
  dumpByte(&D, f->sizeupvalues);
  dumpFunction(&D, f);
  dumpBlock(&D, NULL, 0);  /* signal end of dump */
  return D.status;
}

