// A header which imports the all symbols of the quickjs header but also exports
// the static atoms.

#ifdef QJSJIT_BINDINGS_ONLY
#define QUICKJS_H
typedef struct JSRuntime JSRuntime;
#include "quickjs-jit.h"
#else
#include "quickjs.h"

#ifdef CONFIG_JIT_ABI
#include "quickjs-jit.h"
#endif

#if !defined(EMSCRIPTEN) && !defined(_MSC_VER)
#define CONFIG_ATOMICS
#endif

enum {
  __JS_ATOM_NULL = JS_ATOM_NULL,
#define DEF(name, str) JS_ATOM_##name,
#include "quickjs-atom.h"
#undef DEF
  JS_ATOM_END,
};
#endif
