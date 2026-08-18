/*
** $Id: lopnames.h $
** Opcode names
** See Copyright Notice in lua.h
*/

// AI: lopnames.h holds the human-readable names of the opcodes, used by the
// AI: disassembler/debugger and by error messages. It is included directly by
// AI: lvm.c and lcode.c (which pulls it in just before luaK_finish).
#if !defined(lopnames_h)
#define lopnames_h

#include <stddef.h>


/* ORDER OP */

// AI: 'opnames' must stay in the same order as the OpCode enum in lopcodes.h
// AI: (grep "ORDER OP"); the index is the numeric opcode. NULL marks the end
// AI: of the list and is used as a fallback when printing an invalid opcode.
static const char *const opnames[] = {
  "MOVE",
  "LOADI",
  "LOADF",
  "LOADK",
  "LOADKX",
  "LOADFALSE",
  "LFALSESKIP",
  "LOADTRUE",
  "LOADNIL",
  "GETUPVAL",
  "SETUPVAL",
  "GETTABUP",
  "GETTABLE",
  "GETI",
  "GETFIELD",
  "SETTABUP",
  "SETTABLE",
  "SETI",
  "SETFIELD",
  "NEWTABLE",
  "SELF",
  "ADDI",
  "ADDK",
  "SUBK",
  "MULK",
  "MODK",
  "POWK",
  "DIVK",
  "IDIVK",
  "BANDK",
  "BORK",
  "BXORK",
  "SHLI",
  "SHRI",
  "ADD",
  "SUB",
  "MUL",
  "MOD",
  "POW",
  "DIV",
  "IDIV",
  "BAND",
  "BOR",
  "BXOR",
  "SHL",
  "SHR",
  "MMBIN",
  "MMBINI",
  "MMBINK",
  "UNM",
  "BNOT",
  "NOT",
  "LEN",
  "CONCAT",
  "CLOSE",
  "TBC",
  "JMP",
  "EQ",
  "LT",
  "LE",
  "EQK",
  "EQI",
  "LTI",
  "LEI",
  "GTI",
  "GEI",
  "TEST",
  "TESTSET",
  "CALL",
  "TAILCALL",
  "RETURN",
  "RETURN0",
  "RETURN1",
  "FORLOOP",
  "FORPREP",
  "TFORPREP",
  "TFORCALL",
  "TFORLOOP",
  "SETLIST",
  "CLOSURE",
  "VARARG",
  "GETVARG",
  "ERRNNIL",
  "VARARGPREP",
  "EXTRAARG",
  NULL
};

#endif

