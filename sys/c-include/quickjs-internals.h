// #pragma once
// obviously we can't generate warnings here, even if they exist
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wall"
#pragma GCC diagnostic ignored "-Wextra"

#ifndef JWST_QJS_INTERNALS
#define JWST_QJS_INTERNALS

// #define CONFIG_BIGNUM

/*
 * QuickJS Internal Definition. Stripped from quickjs.c.
 *
 * Copyright (c) 2017-2021 Fabrice Bellard
 * Copyright (c) 2017-2021 Charlie Gordon
 */

#include <assert.h>
#include <fenv.h>
#include <inttypes.h>
#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#if defined(__APPLE__)
#include <malloc/malloc.h>
#elif defined(__linux__)
#include <malloc.h>
#elif defined(__FreeBSD__)
#include <malloc_np.h>
#endif

#include "cutils.h"
#include "libregexp.h"
#include "list.h"
#include "quickjs.h"
#ifdef CONFIG_BIGNUM
#include "libbf.h"
#endif

#define OPTIMIZE 1
#define SHORT_OPCODES 1
#if defined(EMSCRIPTEN)
#define DIRECT_DISPATCH 0
#else
#define DIRECT_DISPATCH 1
#endif

#if defined(__APPLE__)
#define MALLOC_OVERHEAD 0
#else
#define MALLOC_OVERHEAD 8
#endif

#if !defined(_WIN32)
/* define it if printf uses the RNDN rounding mode instead of RNDNA */
#define CONFIG_PRINTF_RNDN
#endif

#if !defined(EMSCRIPTEN)
/* enable stack limitation */
#define CONFIG_STACK_CHECK
#endif

/* dump object free */
//#define DUMP_FREE
//#define DUMP_CLOSURE
/* dump the bytecode of the compiled functions: combination of bits
   1: dump pass 3 final byte code
   2: dump pass 2 code
   4: dump pass 1 code
   8: dump stdlib functions
  16: dump bytecode in hex
  32: dump line number table
 */
//#define DUMP_BYTECODE  (1)
/* dump the occurence of the automatic GC */
//#define DUMP_GC
/* dump objects freed by the garbage collector */
//#define DUMP_GC_FREE
/* dump objects leaking when freeing the runtime */
//#define DUMP_LEAKS  1
/* dump memory usage before running the garbage collector */
//#define DUMP_MEM
//#define DUMP_OBJECTS    /* dump objects in JS_FreeContext */
//#define DUMP_ATOMS      /* dump atoms in JS_FreeContext */
//#define DUMP_SHAPES     /* dump shapes in JS_FreeContext */
//#define DUMP_MODULE_RESOLVE
//#define DUMP_PROMISE
//#define DUMP_READ_OBJECT

/* test the GC by forcing it before each object allocation */
//#define FORCE_GC_AT_MALLOC

enum {
  /* classid tag        */ /* union usage   | properties */
  JS_CLASS_OBJECT = 1,     /* must be first */
  JS_CLASS_ARRAY,          /* u.array       | length */
  JS_CLASS_ERROR,
  JS_CLASS_NUMBER,           /* u.object_data */
  JS_CLASS_STRING,           /* u.object_data */
  JS_CLASS_BOOLEAN,          /* u.object_data */
  JS_CLASS_SYMBOL,           /* u.object_data */
  JS_CLASS_ARGUMENTS,        /* u.array       | length */
  JS_CLASS_MAPPED_ARGUMENTS, /*               | length */
  JS_CLASS_DATE,             /* u.object_data */
  JS_CLASS_MODULE_NS,
  JS_CLASS_C_FUNCTION,          /* u.cfunc */
  JS_CLASS_BYTECODE_FUNCTION,   /* u.func */
  JS_CLASS_BOUND_FUNCTION,      /* u.bound_function */
  JS_CLASS_C_FUNCTION_DATA,     /* u.c_function_data_record */
  JS_CLASS_GENERATOR_FUNCTION,  /* u.func */
  JS_CLASS_FOR_IN_ITERATOR,     /* u.for_in_iterator */
  JS_CLASS_REGEXP,              /* u.regexp */
  JS_CLASS_ARRAY_BUFFER,        /* u.array_buffer */
  JS_CLASS_SHARED_ARRAY_BUFFER, /* u.array_buffer */
  JS_CLASS_UINT8C_ARRAY,        /* u.array (typed_array) */
  JS_CLASS_INT8_ARRAY,          /* u.array (typed_array) */
  JS_CLASS_UINT8_ARRAY,         /* u.array (typed_array) */
  JS_CLASS_INT16_ARRAY,         /* u.array (typed_array) */
  JS_CLASS_UINT16_ARRAY,        /* u.array (typed_array) */
  JS_CLASS_INT32_ARRAY,         /* u.array (typed_array) */
  JS_CLASS_UINT32_ARRAY,        /* u.array (typed_array) */
#ifdef CONFIG_BIGNUM
  JS_CLASS_BIG_INT64_ARRAY,  /* u.array (typed_array) */
  JS_CLASS_BIG_UINT64_ARRAY, /* u.array (typed_array) */
#endif
  JS_CLASS_FLOAT32_ARRAY, /* u.array (typed_array) */
  JS_CLASS_FLOAT64_ARRAY, /* u.array (typed_array) */
  JS_CLASS_DATAVIEW,      /* u.typed_array */
#ifdef CONFIG_BIGNUM
  JS_CLASS_BIG_INT,      /* u.object_data */
  JS_CLASS_BIG_FLOAT,    /* u.object_data */
  JS_CLASS_FLOAT_ENV,    /* u.float_env */
  JS_CLASS_BIG_DECIMAL,  /* u.object_data */
  JS_CLASS_OPERATOR_SET, /* u.operator_set */
#endif
  JS_CLASS_MAP,                      /* u.map_state */
  JS_CLASS_SET,                      /* u.map_state */
  JS_CLASS_WEAKMAP,                  /* u.map_state */
  JS_CLASS_WEAKSET,                  /* u.map_state */
  JS_CLASS_MAP_ITERATOR,             /* u.map_iterator_data */
  JS_CLASS_SET_ITERATOR,             /* u.map_iterator_data */
  JS_CLASS_ARRAY_ITERATOR,           /* u.array_iterator_data */
  JS_CLASS_STRING_ITERATOR,          /* u.array_iterator_data */
  JS_CLASS_REGEXP_STRING_ITERATOR,   /* u.regexp_string_iterator_data */
  JS_CLASS_GENERATOR,                /* u.generator_data */
  JS_CLASS_PROXY,                    /* u.proxy_data */
  JS_CLASS_PROMISE,                  /* u.promise_data */
  JS_CLASS_PROMISE_RESOLVE_FUNCTION, /* u.promise_function_data */
  JS_CLASS_PROMISE_REJECT_FUNCTION,  /* u.promise_function_data */
  JS_CLASS_ASYNC_FUNCTION,           /* u.func */
  JS_CLASS_ASYNC_FUNCTION_RESOLVE,   /* u.async_function_data */
  JS_CLASS_ASYNC_FUNCTION_REJECT,    /* u.async_function_data */
  JS_CLASS_ASYNC_FROM_SYNC_ITERATOR, /* u.async_from_sync_iterator_data */
  JS_CLASS_ASYNC_GENERATOR_FUNCTION, /* u.func */
  JS_CLASS_ASYNC_GENERATOR,          /* u.async_generator_data */

  JS_CLASS_INIT_COUNT, /* last entry for predefined classes */
};

/* number of typed array types */
#define JS_TYPED_ARRAY_COUNT                                                   \
  (JS_CLASS_FLOAT64_ARRAY - JS_CLASS_UINT8C_ARRAY + 1)
static uint8_t const typed_array_size_log2[JS_TYPED_ARRAY_COUNT] = {};
#define typed_array_size_log2(classid)                                         \
  (typed_array_size_log2[(classid)-JS_CLASS_UINT8C_ARRAY])

typedef enum JSErrorEnum {
  JS_EVAL_ERROR,
  JS_RANGE_ERROR,
  JS_REFERENCE_ERROR,
  JS_SYNTAX_ERROR,
  JS_TYPE_ERROR,
  JS_URI_ERROR,
  JS_INTERNAL_ERROR,
  JS_AGGREGATE_ERROR,

  JS_NATIVE_ERROR_COUNT, /* number of different NativeError objects */
} JSErrorEnum;

#define JS_MAX_LOCAL_VARS 65536
#define JS_STACK_SIZE_MAX 65534
#define JS_STRING_LEN_MAX ((1 << 30) - 1)

#define __exception __attribute__((warn_unused_result))

typedef struct JSShape JSShape;
typedef struct JSString JSString;
typedef struct JSString JSAtomStruct;

enum OPCodeEnum {
#define FMT(f)
#define DEF(id, size, n_pop, n_push, f) OP_##id,
#define def(id, size, n_pop, n_push, f)
#include "quickjs-opcode.h"
#undef def
#undef DEF
#undef FMT
  OP_COUNT, /* excluding temporary opcodes */
  /* temporary opcodes : overlap with the short opcodes */
  OP_TEMP_START = OP_nop + 1,
  OP___dummy = OP_TEMP_START - 1,
#define FMT(f)
#define DEF(id, size, n_pop, n_push, f)
#define def(id, size, n_pop, n_push, f) OP_##id,
#include "quickjs-opcode.h"
#undef def
#undef DEF
#undef FMT
  OP_TEMP_END,
};

typedef enum {
  JS_GC_PHASE_NONE,
  JS_GC_PHASE_DECREF,
  JS_GC_PHASE_REMOVE_CYCLES,
} JSGCPhaseEnum;

#ifdef CONFIG_BIGNUM
/* function pointers are used for numeric operations so that it is
   possible to remove some numeric types */
typedef struct {
  JSValue (*to_string)(JSContext *ctx, JSValueConst val);
  JSValue (*from_string)(JSContext *ctx, const char *buf, int radix, int flags,
                         slimb_t *pexponent);
  int (*unary_arith)(JSContext *ctx, JSValue *pres, enum OPCodeEnum op,
                     JSValue op1);
  int (*binary_arith)(JSContext *ctx, enum OPCodeEnum op, JSValue *pres,
                      JSValue op1, JSValue op2);
  int (*compare)(JSContext *ctx, enum OPCodeEnum op, JSValue op1, JSValue op2);
  /* only for bigfloat: */
  JSValue (*mul_pow10_to_float64)(JSContext *ctx, const bf_t *a,
                                  int64_t exponent);
  int (*mul_pow10)(JSContext *ctx, JSValue *sp);
} JSNumericOperations;
#endif

struct JSRuntime {
  JSMallocFunctions mf;
  JSMallocState malloc_state;
  const char *rt_info;

  int atom_hash_size; /* power of two */
  int atom_count;
  int atom_size;
  int atom_count_resize; /* resize hash table at this count */
  uint32_t *atom_hash;
  JSAtomStruct **atom_array;
  int atom_free_index; /* 0 = none */

  int class_count; /* size of class_array */
  JSClass *class_array;

  struct list_head context_list; /* list of JSContext.link */
  /* list of JSGCObjectHeader.link. List of allocated GC objects (used
     by the garbage collector) */
  struct list_head gc_obj_list;
  /* list of JSGCObjectHeader.link. Used during JS_FreeValueRT() */
  struct list_head gc_zero_ref_count_list;
  struct list_head tmp_obj_list; /* used during GC */
  JSGCPhaseEnum gc_phase : 8;
  size_t malloc_gc_threshold;
#ifdef DUMP_LEAKS
  struct list_head string_list; /* list of JSString.link */
#endif
  /* stack limitation */
  uintptr_t stack_size; /* in bytes, 0 if no limit */
  uintptr_t stack_top;
  uintptr_t stack_limit; /* lower stack limit */

  JSValue current_exception;
  /* true if inside an out of memory error, to avoid recursing */
  BOOL in_out_of_memory : 8;

  struct JSStackFrame *current_stack_frame;

  JSInterruptHandler *interrupt_handler;
  void *interrupt_opaque;

  JSHostPromiseRejectionTracker *host_promise_rejection_tracker;
  void *host_promise_rejection_tracker_opaque;

  struct list_head job_list; /* list of JSJobEntry.link */

  JSModuleNormalizeFunc *module_normalize_func;
  JSModuleLoaderFunc *module_loader_func;
  void *module_loader_opaque;

  BOOL can_block : 8; /* TRUE if Atomics.wait can block */
  /* used to allocate, free and clone SharedArrayBuffers */
  JSSharedArrayBufferFunctions sab_funcs;

  /* Shape hash table */
  int shape_hash_bits;
  int shape_hash_size;
  int shape_hash_count; /* number of hashed shapes */
  JSShape **shape_hash;
#ifdef CONFIG_BIGNUM
  bf_context_t bf_ctx;
  JSNumericOperations bigint_ops;
  JSNumericOperations bigfloat_ops;
  JSNumericOperations bigdecimal_ops;
  uint32_t operator_count;
#endif
  void *user_opaque;
};

struct JSClass {
  uint32_t class_id; /* 0 means free entry */
  JSAtom class_name;
  JSClassFinalizer *finalizer;
  JSClassGCMark *gc_mark;
  JSClassCall *call;
  /* pointers for exotic behavior, can be NULL if none are present */
  const JSClassExoticMethods *exotic;
};

#define JS_MODE_STRICT (1 << 0)
#define JS_MODE_STRIP (1 << 1)
#define JS_MODE_MATH (1 << 2)

typedef struct JSStackFrame {
  struct JSStackFrame *prev_frame; /* NULL if first stack frame */
  JSValue
      cur_func; /* current function, JS_UNDEFINED if the frame is detached */
  JSValue *arg_buf;              /* arguments */
  JSValue *var_buf;              /* variables */
  struct list_head var_ref_list; /* list of JSVarRef.link */
  const uint8_t *cur_pc;         /* only used in bytecode functions : PC of the
                              instruction after the call */
  int arg_count;
  int js_mode; /* 0 or JS_MODE_MATH for C functions */
  /* only used in generators. Current stack pointer value. NULL if
     the function is running. */
  JSValue *cur_sp;
} JSStackFrame;

typedef enum {
  JS_GC_OBJ_TYPE_JS_OBJECT,
  JS_GC_OBJ_TYPE_FUNCTION_BYTECODE,
  JS_GC_OBJ_TYPE_SHAPE,
  JS_GC_OBJ_TYPE_VAR_REF,
  JS_GC_OBJ_TYPE_ASYNC_FUNCTION,
  JS_GC_OBJ_TYPE_JS_CONTEXT,
} JSGCObjectTypeEnum;

/* header for GC objects. GC objects are C data structures with a
   reference count that can reference other GC objects. JS Objects are
   a particular type of GC object. */
struct JSGCObjectHeader {
  int ref_count; /* must come first, 32-bit */
  JSGCObjectTypeEnum gc_obj_type : 4;
  uint8_t mark : 4; /* used by the GC */
  uint8_t dummy1;   /* not used by the GC */
  uint16_t dummy2;  /* not used by the GC */
  struct list_head link;
};

typedef struct JSVarRef {
  union {
    JSGCObjectHeader header; /* must come first */
    struct {
      int __gc_ref_count; /* corresponds to header.ref_count */
      uint8_t __gc_mark;  /* corresponds to header.mark/gc_obj_type */

      /* 0 : the JSVarRef is on the stack. header.link is an element
         of JSStackFrame.var_ref_list.
         1 : the JSVarRef is detached. header.link has the normal meanning
      */
      uint8_t is_detached : 1;
      uint8_t is_arg : 1;
      uint16_t var_idx; /* index of the corresponding function variable on
                           the stack */
    };
  };
  JSValue *pvalue; /* pointer to the value, either on the stack or
                      to 'value' */
  JSValue value;   /* used when the variable is no longer on the stack */
} JSVarRef;

#ifdef CONFIG_BIGNUM
typedef struct JSFloatEnv {
  limb_t prec;
  bf_flags_t flags;
  unsigned int status;
} JSFloatEnv;

/* the same structure is used for big integers and big floats. Big
   integers are never infinite or NaNs */
typedef struct JSBigFloat {
  JSRefCountHeader header; /* must come first, 32-bit */
  bf_t num;
} JSBigFloat;

typedef struct JSBigDecimal {
  JSRefCountHeader header; /* must come first, 32-bit */
  bfdec_t num;
} JSBigDecimal;
#endif

typedef enum {
  JS_AUTOINIT_ID_PROTOTYPE,
  JS_AUTOINIT_ID_MODULE_NS,
  JS_AUTOINIT_ID_PROP,
} JSAutoInitIDEnum;

/* must be large enough to have a negligible runtime cost and small
   enough to call the interrupt callback often. */
#define JS_INTERRUPT_COUNTER_INIT 10000

struct JSContext {
  JSGCObjectHeader header; /* must come first */
  JSRuntime *rt;
  struct list_head link;

  uint16_t binary_object_count;
  int binary_object_size;

  JSShape *array_shape; /* initial shape for Array objects */

  JSValue *class_proto;
  JSValue function_proto;
  JSValue function_ctor;
  JSValue array_ctor;
  JSValue regexp_ctor;
  JSValue promise_ctor;
  JSValue native_error_proto[JS_NATIVE_ERROR_COUNT];
  JSValue iterator_proto;
  JSValue async_iterator_proto;
  JSValue array_proto_values;
  JSValue throw_type_error;
  JSValue eval_obj;

  JSValue global_obj;     /* global object */
  JSValue global_var_obj; /* contains the global let/const definitions */

  uint64_t random_state;
#ifdef CONFIG_BIGNUM
  bf_context_t *bf_ctx; /* points to rt->bf_ctx, shared by all contexts */
  JSFloatEnv fp_env;    /* global FP environment */
  BOOL bignum_ext : 8;  /* enable math mode */
  BOOL allow_operator_overloading : 8;
#endif
  /* when the counter reaches zero, JSRutime.interrupt_handler is called */
  int interrupt_counter;
  BOOL is_error_property_enabled;

  struct list_head loaded_modules; /* list of JSModuleDef.link */

  /* if NULL, RegExp compilation is not supported */
  JSValue (*compile_regexp)(JSContext *ctx, JSValueConst pattern,
                            JSValueConst flags);
  /* if NULL, eval is not supported */
  JSValue (*eval_internal)(JSContext *ctx, JSValueConst this_obj,
                           const char *input, size_t input_len,
                           const char *filename, int flags, int scope_idx);
  void *user_opaque;
};

typedef union JSFloat64Union {
  double d;
  uint64_t u64;
  uint32_t u32[2];
} JSFloat64Union;

enum {
  JS_ATOM_TYPE_STRING = 1,
  JS_ATOM_TYPE_GLOBAL_SYMBOL,
  JS_ATOM_TYPE_SYMBOL,
  JS_ATOM_TYPE_PRIVATE,
};

enum {
  JS_ATOM_HASH_SYMBOL,
  JS_ATOM_HASH_PRIVATE,
};

typedef enum {
  JS_ATOM_KIND_STRING,
  JS_ATOM_KIND_SYMBOL,
  JS_ATOM_KIND_PRIVATE,
} JSAtomKindEnum;

#define JS_ATOM_HASH_MASK ((1 << 30) - 1)

struct JSString {
  JSRefCountHeader header; /* must come first, 32-bit */
  uint32_t len : 31;
  uint8_t is_wide_char : 1; /* 0 = 8 bits, 1 = 16 bits characters */
  /* for JS_ATOM_TYPE_SYMBOL: hash = 0, atom_type = 3,
     for JS_ATOM_TYPE_PRIVATE: hash = 1, atom_type = 3
     XXX: could change encoding to have one more bit in hash */
  uint32_t hash : 30;
  uint8_t atom_type : 2; /* != 0 if atom, JS_ATOM_TYPE_x */
  uint32_t hash_next;    /* atom_index for JS_ATOM_TYPE_SYMBOL */
#ifdef DUMP_LEAKS
  struct list_head link; /* string list */
#endif
  union {
    uint8_t str8[0]; /* 8 bit strings will get an extra null terminator */
    uint16_t str16[0];
  } u;
};

typedef struct JSClosureVar {
  uint8_t is_local : 1;
  uint8_t is_arg : 1;
  uint8_t is_const : 1;
  uint8_t is_lexical : 1;
  uint8_t var_kind : 4; /* see JSVarKindEnum */
  /* 8 bits available */
  uint16_t var_idx; /* is_local = TRUE: index to a normal variable of the
                  parent function. otherwise: index to a closure
                  variable of the parent function */
  JSAtom var_name;
} JSClosureVar;

#define ARG_SCOPE_INDEX 1
#define ARG_SCOPE_END (-2)

typedef struct JSVarScope {
  int parent; /* index into fd->scopes of the enclosing scope */
  int first;  /* index into fd->vars of the last variable in this scope */
} JSVarScope;

typedef enum {
  /* XXX: add more variable kinds here instead of using bit fields */
  JS_VAR_NORMAL,
  JS_VAR_FUNCTION_DECL,     /* lexical var with function declaration */
  JS_VAR_NEW_FUNCTION_DECL, /* lexical var with async/generator
                               function declaration */
  JS_VAR_CATCH,
  JS_VAR_FUNCTION_NAME, /* function expression name */
  JS_VAR_PRIVATE_FIELD,
  JS_VAR_PRIVATE_METHOD,
  JS_VAR_PRIVATE_GETTER,
  JS_VAR_PRIVATE_SETTER,        /* must come after JS_VAR_PRIVATE_GETTER */
  JS_VAR_PRIVATE_GETTER_SETTER, /* must come after JS_VAR_PRIVATE_SETTER */
} JSVarKindEnum;

/* XXX: could use a different structure in bytecode functions to save
   memory */
typedef struct JSVarDef {
  JSAtom var_name;
  /* index into fd->scopes of this variable lexical scope */
  int scope_level;
  /* during compilation:
      - if scope_level = 0: scope in which the variable is defined
      - if scope_level != 0: index into fd->vars of the next
        variable in the same or enclosing lexical scope
     in a bytecode function:
     index into fd->vars of the next
     variable in the same or enclosing lexical scope
  */
  int scope_next;
  uint8_t is_const : 1;
  uint8_t is_lexical : 1;
  uint8_t is_captured : 1;
  uint8_t var_kind : 4; /* see JSVarKindEnum */
  /* only used during compilation: function pool index for lexical
     variables with var_kind =
     JS_VAR_FUNCTION_DECL/JS_VAR_NEW_FUNCTION_DECL or scope level of
     the definition of the 'var' variables (they have scope_level =
     0) */
  int func_pool_idx : 24; /* only used during compilation : index in
                             the constant pool for hoisted function
                             definition */
} JSVarDef;

/* for the encoding of the pc2line table */
#define PC2LINE_BASE (-1)
#define PC2LINE_RANGE 5
#define PC2LINE_OP_FIRST 1
#define PC2LINE_DIFF_PC_MAX ((255 - PC2LINE_OP_FIRST) / PC2LINE_RANGE)

typedef enum JSFunctionKindEnum {
  JS_FUNC_NORMAL = 0,
  JS_FUNC_GENERATOR = (1 << 0),
  JS_FUNC_ASYNC = (1 << 1),
  JS_FUNC_ASYNC_GENERATOR = (JS_FUNC_GENERATOR | JS_FUNC_ASYNC),
} JSFunctionKindEnum;

typedef struct JSFunctionBytecode {
  JSGCObjectHeader header; /* must come first */
  uint8_t js_mode;
  uint8_t has_prototype : 1; /* true if a prototype field is necessary */
  uint8_t has_simple_parameter_list : 1;
  uint8_t is_derived_class_constructor : 1;
  /* true if home_object needs to be initialized */
  uint8_t need_home_object : 1;
  uint8_t func_kind : 2;
  uint8_t new_target_allowed : 1;
  uint8_t super_call_allowed : 1;
  uint8_t super_allowed : 1;
  uint8_t arguments_allowed : 1;
  uint8_t has_debug : 1;
  uint8_t backtrace_barrier : 1; /* stop backtrace on this function */
  uint8_t read_only_bytecode : 1;
  /* XXX: 4 bits available */
  uint8_t *byte_code_buf; /* (self pointer) */
  int byte_code_len;
  JSAtom func_name;
  JSVarDef *vardefs; /* arguments + local variables (arg_count + var_count)
                        (self pointer) */
  JSClosureVar
      *closure_var; /* list of variables in the closure (self pointer) */
  uint16_t arg_count;
  uint16_t var_count;
  uint16_t defined_arg_count; /* for length function property */
  uint16_t stack_size;        /* maximum stack size */
  JSContext *realm;           /* function realm */
  JSValue *cpool;             /* constant pool (self pointer) */
  int cpool_count;
  int closure_var_count;
  struct {
    /* debug info, move to separate structure to save memory? */
    JSAtom filename;
    int line_num;
    int source_len;
    int pc2line_len;
    uint8_t *pc2line_buf;
    char *source;
  } debug;
} JSFunctionBytecode;

typedef struct JSBoundFunction {
  JSValue func_obj;
  JSValue this_val;
  int argc;
  JSValue argv[0];
} JSBoundFunction;

typedef enum JSIteratorKindEnum {
  JS_ITERATOR_KIND_KEY,
  JS_ITERATOR_KIND_VALUE,
  JS_ITERATOR_KIND_KEY_AND_VALUE,
} JSIteratorKindEnum;

typedef struct JSForInIterator {
  JSValue obj;
  BOOL is_array;
  uint32_t array_length;
  uint32_t idx;
} JSForInIterator;

typedef struct JSRegExp {
  JSString *pattern;
  JSString *bytecode; /* also contains the flags */
} JSRegExp;

typedef struct JSProxyData {
  JSValue target;
  JSValue handler;
  uint8_t is_func;
  uint8_t is_revoked;
} JSProxyData;

typedef struct JSArrayBuffer {
  int byte_length; /* 0 if detached */
  uint8_t detached;
  uint8_t shared; /* if shared, the array buffer cannot be detached */
  uint8_t *data;  /* NULL if detached */
  struct list_head array_list;
  void *opaque;
  JSFreeArrayBufferDataFunc *free_func;
} JSArrayBuffer;

typedef struct JSTypedArray {
  struct list_head link; /* link to arraybuffer */
  JSObject *obj;         /* back pointer to the TypedArray/DataView object */
  JSObject *buffer;      /* based array buffer */
  uint32_t offset;       /* offset in the array buffer */
  uint32_t length;       /* length in the array buffer */
} JSTypedArray;

typedef struct JSAsyncFunctionState {
  JSValue this_val; /* 'this' generator argument */
  int argc;         /* number of function arguments */
  BOOL throw_flag;  /* used to throw an exception in JS_CallInternal() */
  JSStackFrame frame;
} JSAsyncFunctionState;

/* XXX: could use an object instead to avoid the
   JS_TAG_ASYNC_FUNCTION tag for the GC */
typedef struct JSAsyncFunctionData {
  JSGCObjectHeader header; /* must come first */
  JSValue resolving_funcs[2];
  BOOL is_active; /* true if the async function state is valid */
  JSAsyncFunctionState func_state;
} JSAsyncFunctionData;

typedef enum {
  /* binary operators */
  JS_OVOP_ADD,
  JS_OVOP_SUB,
  JS_OVOP_MUL,
  JS_OVOP_DIV,
  JS_OVOP_MOD,
  JS_OVOP_POW,
  JS_OVOP_OR,
  JS_OVOP_AND,
  JS_OVOP_XOR,
  JS_OVOP_SHL,
  JS_OVOP_SAR,
  JS_OVOP_SHR,
  JS_OVOP_EQ,
  JS_OVOP_LESS,

  JS_OVOP_BINARY_COUNT,
  /* unary operators */
  JS_OVOP_POS = JS_OVOP_BINARY_COUNT,
  JS_OVOP_NEG,
  JS_OVOP_INC,
  JS_OVOP_DEC,
  JS_OVOP_NOT,

  JS_OVOP_COUNT,
} JSOverloadableOperatorEnum;

typedef struct {
  uint32_t operator_index;
  JSObject *ops[JS_OVOP_BINARY_COUNT]; /* self operators */
} JSBinaryOperatorDefEntry;

typedef struct {
  int count;
  JSBinaryOperatorDefEntry *tab;
} JSBinaryOperatorDef;

typedef struct JSOperatorSetData {
  uint32_t operator_counter;
  BOOL is_primitive; /* OperatorSet for a primitive type */
  /* NULL if no operator is defined */
  JSObject *self_ops[JS_OVOP_COUNT]; /* self operators */
  JSBinaryOperatorDef left;
  JSBinaryOperatorDef right;
} JSOperatorSetData;

typedef struct JSReqModuleEntry {
  JSAtom module_name;
  JSModuleDef *module; /* used using resolution */
} JSReqModuleEntry;

typedef enum JSExportTypeEnum {
  JS_EXPORT_TYPE_LOCAL,
  JS_EXPORT_TYPE_INDIRECT,
} JSExportTypeEnum;

typedef struct JSExportEntry {
  union {
    struct {
      int var_idx;       /* closure variable index */
      JSVarRef *var_ref; /* if != NULL, reference to the variable */
    } local;             /* for local export */
    int req_module_idx;  /* module for indirect export */
  } u;
  JSExportTypeEnum export_type;
  JSAtom local_name;  /* '*' if export ns from. not used for local
                         export after compilation */
  JSAtom export_name; /* exported variable name */
} JSExportEntry;

typedef struct JSStarExportEntry {
  int req_module_idx; /* in req_module_entries */
} JSStarExportEntry;

typedef struct JSImportEntry {
  int var_idx; /* closure variable index */
  JSAtom import_name;
  int req_module_idx; /* in req_module_entries */
} JSImportEntry;

struct JSModuleDef {
  JSRefCountHeader header; /* must come first, 32-bit */
  JSAtom module_name;
  struct list_head link;

  JSReqModuleEntry *req_module_entries;
  int req_module_entries_count;
  int req_module_entries_size;

  JSExportEntry *export_entries;
  int export_entries_count;
  int export_entries_size;

  JSStarExportEntry *star_export_entries;
  int star_export_entries_count;
  int star_export_entries_size;

  JSImportEntry *import_entries;
  int import_entries_count;
  int import_entries_size;

  JSValue module_ns;
  JSValue func_obj;            /* only used for JS modules */
  JSModuleInitFunc *init_func; /* only used for C modules */
  BOOL resolved : 8;
  BOOL func_created : 8;
  BOOL instantiated : 8;
  BOOL evaluated : 8;
  BOOL eval_mark : 8; /* temporary use during js_evaluate_module() */
  /* true if evaluation yielded an exception. It is saved in
     eval_exception */
  BOOL eval_has_exception : 8;
  JSValue eval_exception;
  JSValue meta_obj; /* for import.meta */
};

typedef struct JSJobEntry {
  struct list_head link;
  JSContext *ctx;
  JSJobFunc *job_func;
  int argc;
  JSValue argv[0];
} JSJobEntry;

typedef struct JSProperty {
  union {
    JSValue value;      /* JS_PROP_NORMAL */
    struct {            /* JS_PROP_GETSET */
      JSObject *getter; /* NULL if undefined */
      JSObject *setter; /* NULL if undefined */
    } getset;
    JSVarRef *var_ref; /* JS_PROP_VARREF */
    struct {           /* JS_PROP_AUTOINIT */
      /* in order to use only 2 pointers, we compress the realm
         and the init function pointer */
      uintptr_t realm_and_id; /* realm and init_id (JS_AUTOINIT_ID_x)
                                 in the 2 low bits */
      void *opaque;
    } init;
  } u;
} JSProperty;

#define JS_PROP_INITIAL_SIZE 2
#define JS_PROP_INITIAL_HASH_SIZE 4 /* must be a power of two */
#define JS_ARRAY_INITIAL_SIZE 2

typedef struct JSShapeProperty {
  uint32_t hash_next : 26; /* 0 if last in list */
  uint32_t flags : 6;      /* JS_PROP_XXX */
  JSAtom atom;             /* JS_ATOM_NULL = free property entry */
} JSShapeProperty;

struct JSShape {
  /* hash table of size hash_mask + 1 before the start of the
     structure (see prop_hash_end()). */
  JSGCObjectHeader header;
  /* true if the shape is inserted in the shape hash table. If not,
     JSShape.hash is not valid */
  uint8_t is_hashed;
  /* If true, the shape may have small array index properties 'n' with 0
     <= n <= 2^31-1. If false, the shape is guaranteed not to have
     small array index properties */
  uint8_t has_small_array_index;
  uint32_t hash; /* current hash value */
  uint32_t prop_hash_mask;
  int prop_size;  /* allocated properties */
  int prop_count; /* include deleted properties */
  int deleted_prop_count;
  JSShape *shape_hash_next; /* in JSRuntime.shape_hash[h] list */
  JSObject *proto;
  JSShapeProperty prop[0]; /* prop_size elements */
};

struct JSObject {
  union {
    JSGCObjectHeader header;
    struct JSObjectBitfield {
      int __gc_ref_count; /* corresponds to header.ref_count */
      uint8_t __gc_mark;  /* corresponds to header.mark/gc_obj_type */

      uint8_t extensible : 1;
      uint8_t free_mark : 1;  /* only used when freeing objects with cycles */
      uint8_t is_exotic : 1;  /* TRUE if object has exotic property handlers */
      uint8_t fast_array : 1; /* TRUE if u.array is used for get/put (for
                                 JS_CLASS_ARRAY, JS_CLASS_ARGUMENTS and typed
                                 arrays) */
      uint8_t is_constructor : 1; /* TRUE if object is a constructor function */
      uint8_t is_uncatchable_error : 1; /* if TRUE, error is not catchable */
      uint8_t tmp_mark : 1;             /* used in JS_WriteObjectRec() */
      uint8_t is_HTMLDDA : 1; /* specific annex B IsHtmlDDA behavior */
      uint16_t class_id;      /* see JS_CLASS_x */
    } bitfield;
  } hdr;
  /* byte offsets: 16/24 */
  JSShape *shape;   /* prototype and property names + flag */
  JSProperty *prop; /* array of properties */
  /* byte offsets: 24/40 */
  struct JSMapRecord
      *first_weak_ref; /* XXX: use a bit and an external hash table? */
  /* byte offsets: 28/48 */
  union {
    void *opaque;
    struct JSBoundFunction *bound_function; /* JS_CLASS_BOUND_FUNCTION */
    struct JSCFunctionDataRecord
        *c_function_data_record;             /* JS_CLASS_C_FUNCTION_DATA */
    struct JSForInIterator *for_in_iterator; /* JS_CLASS_FOR_IN_ITERATOR */
    struct JSArrayBuffer
        *array_buffer; /* JS_CLASS_ARRAY_BUFFER, JS_CLASS_SHARED_ARRAY_BUFFER */
    struct JSTypedArray
        *typed_array; /* JS_CLASS_UINT8C_ARRAY..JS_CLASS_DATAVIEW */
#ifdef CONFIG_BIGNUM
    struct JSFloatEnv *float_env;           /* JS_CLASS_FLOAT_ENV */
    struct JSOperatorSetData *operator_set; /* JS_CLASS_OPERATOR_SET */
#endif
    struct JSMapState *map_state; /* JS_CLASS_MAP..JS_CLASS_WEAKSET */
    struct JSMapIteratorData
        *map_iterator_data; /* JS_CLASS_MAP_ITERATOR, JS_CLASS_SET_ITERATOR */
    struct JSArrayIteratorData
        *array_iterator_data; /* JS_CLASS_ARRAY_ITERATOR,
                                 JS_CLASS_STRING_ITERATOR */
    struct JSRegExpStringIteratorData
        *regexp_string_iterator_data; /* JS_CLASS_REGEXP_STRING_ITERATOR */
    struct JSGeneratorData *generator_data; /* JS_CLASS_GENERATOR */
    struct JSProxyData *proxy_data;         /* JS_CLASS_PROXY */
    struct JSPromiseData *promise_data;     /* JS_CLASS_PROMISE */
    struct JSPromiseFunctionData
        *promise_function_data; /* JS_CLASS_PROMISE_RESOLVE_FUNCTION,
                                   JS_CLASS_PROMISE_REJECT_FUNCTION */
    struct JSAsyncFunctionData
        *async_function_data; /* JS_CLASS_ASYNC_FUNCTION_RESOLVE,
                                 JS_CLASS_ASYNC_FUNCTION_REJECT */
    struct JSAsyncFromSyncIteratorData
        *async_from_sync_iterator_data; /* JS_CLASS_ASYNC_FROM_SYNC_ITERATOR */
    struct JSAsyncGeneratorData
        *async_generator_data; /* JS_CLASS_ASYNC_GENERATOR */
    struct JSBytecodeFunctionData {
      /* JS_CLASS_BYTECODE_FUNCTION: 12/24 bytes
       * also used by JS_CLASS_GENERATOR_FUNCTION, JS_CLASS_ASYNC_FUNCTION and
       * JS_CLASS_ASYNC_GENERATOR_FUNCTION */
      struct JSFunctionBytecode *function_bytecode;
      JSVarRef **var_refs;
      JSObject *home_object; /* for 'super' access */
    } func;
    struct JSCFuntionData { /* JS_CLASS_C_FUNCTION: 12/20 bytes */
      JSContext *realm;
      JSCFunctionType c_function;
      uint8_t length;
      uint8_t cproto;
      int16_t magic;
    } cfunc;
    /* array part for fast arrays and typed arrays */
    struct JSArrayData { /* JS_CLASS_ARRAY, JS_CLASS_ARGUMENTS,
                 JS_CLASS_UINT8C_ARRAY..JS_CLASS_FLOAT64_ARRAY */
      union {
        uint32_t size; /* JS_CLASS_ARRAY, JS_CLASS_ARGUMENTS */
        struct JSTypedArray
            *typed_array; /* JS_CLASS_UINT8C_ARRAY..JS_CLASS_FLOAT64_ARRAY */
      } u1;
      union {
        JSValue *values;    /* JS_CLASS_ARRAY, JS_CLASS_ARGUMENTS */
        void *ptr;          /* JS_CLASS_UINT8C_ARRAY..JS_CLASS_FLOAT64_ARRAY */
        int8_t *int8_ptr;   /* JS_CLASS_INT8_ARRAY */
        uint8_t *uint8_ptr; /* JS_CLASS_UINT8_ARRAY, JS_CLASS_UINT8C_ARRAY */
        int16_t *int16_ptr; /* JS_CLASS_INT16_ARRAY */
        uint16_t *uint16_ptr; /* JS_CLASS_UINT16_ARRAY */
        int32_t *int32_ptr;   /* JS_CLASS_INT32_ARRAY */
        uint32_t *uint32_ptr; /* JS_CLASS_UINT32_ARRAY */
        int64_t *int64_ptr;   /* JS_CLASS_INT64_ARRAY */
        uint64_t *uint64_ptr; /* JS_CLASS_UINT64_ARRAY */
        float *float_ptr;     /* JS_CLASS_FLOAT32_ARRAY */
        double *double_ptr;   /* JS_CLASS_FLOAT64_ARRAY */
      } u;
      uint32_t count;    /* <= 2^31-1. 0 for a detached typed array */
    } array;             /* 12/20 bytes */
    JSRegExp regexp;     /* JS_CLASS_REGEXP: 8/16 bytes */
    JSValue object_data; /* for JS_SetObjectData(): 8/16/16 bytes */
  } u;
  /* byte sizes: 40/48/72 */
};
enum {
  __JS_ATOM_NULL = JS_ATOM_NULL,
#define DEF(name, str) JS_ATOM_##name,
#include "quickjs-atom.h"
#undef DEF
  JS_ATOM_END,
};
#define JS_ATOM_LAST_KEYWORD JS_ATOM_super
#define JS_ATOM_LAST_STRICT_KEYWORD JS_ATOM_yield

static const char js_atom_init[] =
#define DEF(name, str) str "\0"
#include "quickjs-atom.h"
#undef DEF
    ;

typedef enum OPCodeFormat {
#define FMT(f) OP_FMT_##f,
#define DEF(id, size, n_pop, n_push, f)
#include "quickjs-opcode.h"
#undef DEF
#undef FMT
} OPCodeFormat;

int JS_InitAtoms(JSRuntime *rt);
JSAtom __JS_NewAtomInit(JSRuntime *rt, const char *str, int len, int atom_type);
void JS_FreeAtomStruct(JSRuntime *rt, JSAtomStruct *p);
void free_function_bytecode(JSRuntime *rt, JSFunctionBytecode *b);
JSValue js_call_c_function(JSContext *ctx, JSValueConst func_obj,
                           JSValueConst this_obj, int argc, JSValueConst *argv,
                           int flags);
JSValue js_call_bound_function(JSContext *ctx, JSValueConst func_obj,
                               JSValueConst this_obj, int argc,
                               JSValueConst *argv, int flags);
JSValue JS_CallInternal(JSContext *ctx, JSValueConst func_obj,
                        JSValueConst this_obj, JSValueConst new_target,
                        int argc, JSValue *argv, int flags);
JSValue JS_CallConstructorInternal(JSContext *ctx, JSValueConst func_obj,
                                   JSValueConst new_target, int argc,
                                   JSValue *argv, int flags);
JSValue JS_CallFree(JSContext *ctx, JSValue func_obj, JSValueConst this_obj,
                    int argc, JSValueConst *argv);
JSValue JS_InvokeFree(JSContext *ctx, JSValue this_val, JSAtom atom, int argc,
                      JSValueConst *argv);
__exception int JS_ToArrayLengthFree(JSContext *ctx, uint32_t *plen,
                                     JSValue val, BOOL is_array_ctor);
JSValue JS_EvalObject(JSContext *ctx, JSValueConst this_obj, JSValueConst val,
                      int flags, int scope_idx);
JSValue __attribute__((format(printf, 2, 3)))
JS_ThrowInternalError(JSContext *ctx, const char *fmt, ...);
__maybe_unused void JS_DumpAtoms(JSRuntime *rt);
__maybe_unused void JS_DumpString(JSRuntime *rt, const JSString *p);
__maybe_unused void JS_DumpObjectHeader(JSRuntime *rt);
__maybe_unused void JS_DumpObject(JSRuntime *rt, JSObject *p);
__maybe_unused void JS_DumpGCObject(JSRuntime *rt, JSGCObjectHeader *p);
__maybe_unused void JS_DumpValueShort(JSRuntime *rt, JSValueConst val);
__maybe_unused void JS_DumpValue(JSContext *ctx, JSValueConst val);
__maybe_unused void JS_PrintValue(JSContext *ctx, const char *str,
                                  JSValueConst val);
__maybe_unused void JS_DumpShapes(JSRuntime *rt);
JSValue js_function_apply(JSContext *ctx, JSValueConst this_val, int argc,
                          JSValueConst *argv, int magic);
void js_array_finalizer(JSRuntime *rt, JSValue val);
void js_array_mark(JSRuntime *rt, JSValueConst val, JS_MarkFunc *mark_func);
void js_object_data_finalizer(JSRuntime *rt, JSValue val);
void js_object_data_mark(JSRuntime *rt, JSValueConst val,
                         JS_MarkFunc *mark_func);
void js_c_function_finalizer(JSRuntime *rt, JSValue val);
void js_c_function_mark(JSRuntime *rt, JSValueConst val,
                        JS_MarkFunc *mark_func);
void js_bytecode_function_finalizer(JSRuntime *rt, JSValue val);
void js_bytecode_function_mark(JSRuntime *rt, JSValueConst val,
                               JS_MarkFunc *mark_func);
void js_bound_function_finalizer(JSRuntime *rt, JSValue val);
void js_bound_function_mark(JSRuntime *rt, JSValueConst val,
                            JS_MarkFunc *mark_func);
void js_for_in_iterator_finalizer(JSRuntime *rt, JSValue val);
void js_for_in_iterator_mark(JSRuntime *rt, JSValueConst val,
                             JS_MarkFunc *mark_func);
void js_regexp_finalizer(JSRuntime *rt, JSValue val);
void js_array_buffer_finalizer(JSRuntime *rt, JSValue val);
void js_typed_array_finalizer(JSRuntime *rt, JSValue val);
void js_typed_array_mark(JSRuntime *rt, JSValueConst val,
                         JS_MarkFunc *mark_func);
void js_proxy_finalizer(JSRuntime *rt, JSValue val);
void js_proxy_mark(JSRuntime *rt, JSValueConst val, JS_MarkFunc *mark_func);
void js_map_finalizer(JSRuntime *rt, JSValue val);
void js_map_mark(JSRuntime *rt, JSValueConst val, JS_MarkFunc *mark_func);
void js_map_iterator_finalizer(JSRuntime *rt, JSValue val);
void js_map_iterator_mark(JSRuntime *rt, JSValueConst val,
                          JS_MarkFunc *mark_func);
void js_array_iterator_finalizer(JSRuntime *rt, JSValue val);
void js_array_iterator_mark(JSRuntime *rt, JSValueConst val,
                            JS_MarkFunc *mark_func);
void js_regexp_string_iterator_finalizer(JSRuntime *rt, JSValue val);
void js_regexp_string_iterator_mark(JSRuntime *rt, JSValueConst val,
                                    JS_MarkFunc *mark_func);
void js_generator_finalizer(JSRuntime *rt, JSValue obj);
void js_generator_mark(JSRuntime *rt, JSValueConst val, JS_MarkFunc *mark_func);
void js_promise_finalizer(JSRuntime *rt, JSValue val);
void js_promise_mark(JSRuntime *rt, JSValueConst val, JS_MarkFunc *mark_func);
void js_promise_resolve_function_finalizer(JSRuntime *rt, JSValue val);
void js_promise_resolve_function_mark(JSRuntime *rt, JSValueConst val,
                                      JS_MarkFunc *mark_func);
#ifdef CONFIG_BIGNUM
void js_operator_set_finalizer(JSRuntime *rt, JSValue val);
void js_operator_set_mark(JSRuntime *rt, JSValueConst val,
                          JS_MarkFunc *mark_func);
#endif
JSValue JS_ToStringFree(JSContext *ctx, JSValue val);
int JS_ToBoolFree(JSContext *ctx, JSValue val);
int JS_ToInt32Free(JSContext *ctx, int32_t *pres, JSValue val);
int JS_ToFloat64Free(JSContext *ctx, double *pres, JSValue val);
int JS_ToUint8ClampFree(JSContext *ctx, int32_t *pres, JSValue val);
JSValue js_compile_regexp(JSContext *ctx, JSValueConst pattern,
                          JSValueConst flags);
JSValue js_regexp_constructor_internal(JSContext *ctx, JSValueConst ctor,
                                       JSValue pattern, JSValue bc);
void gc_decref(JSRuntime *rt);
int JS_NewClass1(JSRuntime *rt, JSClassID class_id, const JSClassDef *class_def,
                 JSAtom name);

typedef enum JSStrictEqModeEnum {
  JS_EQ_STRICT,
  JS_EQ_SAME_VALUE,
  JS_EQ_SAME_VALUE_ZERO,
} JSStrictEqModeEnum;

BOOL js_strict_eq2(JSContext *ctx, JSValue op1, JSValue op2,
                   JSStrictEqModeEnum eq_mode);
BOOL js_strict_eq(JSContext *ctx, JSValue op1, JSValue op2);
BOOL js_same_value(JSContext *ctx, JSValueConst op1, JSValueConst op2);
BOOL js_same_value_zero(JSContext *ctx, JSValueConst op1, JSValueConst op2);
JSValue JS_ToObject(JSContext *ctx, JSValueConst val);
JSValue JS_ToObjectFree(JSContext *ctx, JSValue val);
JSProperty *add_property(JSContext *ctx, JSObject *p, JSAtom prop,
                         int prop_flags);
#ifdef CONFIG_BIGNUM
void js_float_env_finalizer(JSRuntime *rt, JSValue val);
JSValue JS_NewBigFloat(JSContext *ctx);
JSValue JS_CompactBigInt1(JSContext *ctx, JSValue val,
                          BOOL convert_to_safe_integer);
JSValue JS_CompactBigInt(JSContext *ctx, JSValue val);
int JS_ToBigInt64Free(JSContext *ctx, int64_t *pres, JSValue val);
bf_t *JS_ToBigInt(JSContext *ctx, bf_t *buf, JSValueConst val);
void JS_FreeBigInt(JSContext *ctx, bf_t *a, bf_t *buf);
bf_t *JS_ToBigFloat(JSContext *ctx, bf_t *buf, JSValueConst val);
JSValue JS_ToBigDecimalFree(JSContext *ctx, JSValue val,
                            BOOL allow_null_or_undefined);
bfdec_t *JS_ToBigDecimal(JSContext *ctx, JSValueConst val);
#endif
JSValue JS_ThrowOutOfMemory(JSContext *ctx);
JSValue JS_ThrowTypeErrorRevokedProxy(JSContext *ctx);
JSValue js_proxy_getPrototypeOf(JSContext *ctx, JSValueConst obj);
int js_proxy_setPrototypeOf(JSContext *ctx, JSValueConst obj,
                            JSValueConst proto_val, BOOL throw_flag);
int js_proxy_isExtensible(JSContext *ctx, JSValueConst obj);
int js_proxy_preventExtensions(JSContext *ctx, JSValueConst obj);
int js_proxy_isArray(JSContext *ctx, JSValueConst obj);
int JS_CreateProperty(JSContext *ctx, JSObject *p, JSAtom prop,
                      JSValueConst val, JSValueConst getter,
                      JSValueConst setter, int flags);
int js_string_memcmp(const JSString *p1, const JSString *p2, int len);
void reset_weak_ref(JSRuntime *rt, JSObject *p);
JSValue js_array_buffer_constructor3(JSContext *ctx, JSValueConst new_target,
                                     uint64_t len, JSClassID class_id,
                                     uint8_t *buf,
                                     JSFreeArrayBufferDataFunc *free_func,
                                     void *opaque, BOOL alloc_flag);
JSArrayBuffer *js_get_array_buffer(JSContext *ctx, JSValueConst obj);
JSValue js_typed_array_constructor(JSContext *ctx, JSValueConst this_val,
                                   int argc, JSValueConst *argv, int classid);
BOOL typed_array_is_detached(JSContext *ctx, JSObject *p);
uint32_t typed_array_get_length(JSContext *ctx, JSObject *p);
JSValue JS_ThrowTypeErrorDetachedArrayBuffer(JSContext *ctx);
JSVarRef *get_var_ref(JSContext *ctx, JSStackFrame *sf, int var_idx,
                      BOOL is_arg);
JSValue js_generator_function_call(JSContext *ctx, JSValueConst func_obj,
                                   JSValueConst this_obj, int argc,
                                   JSValueConst *argv, int flags);
void js_async_function_resolve_finalizer(JSRuntime *rt, JSValue val);
void js_async_function_resolve_mark(JSRuntime *rt, JSValueConst val,
                                    JS_MarkFunc *mark_func);
JSValue JS_EvalInternal(JSContext *ctx, JSValueConst this_obj,
                        const char *input, size_t input_len,
                        const char *filename, int flags, int scope_idx);
void js_free_module_def(JSContext *ctx, JSModuleDef *m);
void js_mark_module_def(JSRuntime *rt, JSModuleDef *m, JS_MarkFunc *mark_func);
JSValue js_import_meta(JSContext *ctx);
JSValue js_dynamic_import(JSContext *ctx, JSValueConst specifier);
void free_var_ref(JSRuntime *rt, JSVarRef *var_ref);
JSValue js_new_promise_capability(JSContext *ctx, JSValue *resolving_funcs,
                                  JSValueConst ctor);
__exception int perform_promise_then(JSContext *ctx, JSValueConst promise,
                                     JSValueConst *resolve_reject,
                                     JSValueConst *cap_resolving_funcs);
JSValue js_promise_resolve(JSContext *ctx, JSValueConst this_val, int argc,
                           JSValueConst *argv, int magic);
int js_string_compare(JSContext *ctx, const JSString *p1, const JSString *p2);
JSValue JS_ToNumber(JSContext *ctx, JSValueConst val);
int JS_SetPropertyValue(JSContext *ctx, JSValueConst this_obj, JSValue prop,
                        JSValue val, int flags);
int JS_NumberIsInteger(JSContext *ctx, JSValueConst val);
BOOL JS_NumberIsNegativeOrMinusZero(JSContext *ctx, JSValueConst val);
JSValue JS_ToNumberFree(JSContext *ctx, JSValue val);
int JS_GetOwnPropertyInternal(JSContext *ctx, JSPropertyDescriptor *desc,
                              JSObject *p, JSAtom prop);
void js_free_desc(JSContext *ctx, JSPropertyDescriptor *desc);
void async_func_mark(JSRuntime *rt, JSAsyncFunctionState *s,
                     JS_MarkFunc *mark_func);
void JS_AddIntrinsicBasicObjects(JSContext *ctx);
void js_free_shape(JSRuntime *rt, JSShape *sh);
void js_free_shape_null(JSRuntime *rt, JSShape *sh);
int js_shape_prepare_update(JSContext *ctx, JSObject *p,
                            JSShapeProperty **pprs);
int init_shape_hash(JSRuntime *rt);
__exception int js_get_length32(JSContext *ctx, uint32_t *pres,
                                JSValueConst obj);
__exception int js_get_length64(JSContext *ctx, int64_t *pres,
                                JSValueConst obj);
void free_arg_list(JSContext *ctx, JSValue *tab, uint32_t len);
JSValue *build_arg_list(JSContext *ctx, uint32_t *plen, JSValueConst array_arg);
BOOL js_get_fast_array(JSContext *ctx, JSValueConst obj, JSValue **arrpp,
                       uint32_t *countp);
JSValue JS_CreateAsyncFromSyncIterator(JSContext *ctx, JSValueConst sync_iter);
void js_c_function_data_finalizer(JSRuntime *rt, JSValue val);
void js_c_function_data_mark(JSRuntime *rt, JSValueConst val,
                             JS_MarkFunc *mark_func);
JSValue js_c_function_data_call(JSContext *ctx, JSValueConst func_obj,
                                JSValueConst this_val, int argc,
                                JSValueConst *argv, int flags);
JSAtom js_symbol_to_atom(JSContext *ctx, JSValue val);
void add_gc_object(JSRuntime *rt, JSGCObjectHeader *h, JSGCObjectTypeEnum type);
void remove_gc_object(JSGCObjectHeader *h);
void js_async_function_free0(JSRuntime *rt, JSAsyncFunctionData *s);
JSValue js_instantiate_prototype(JSContext *ctx, JSObject *p, JSAtom atom,
                                 void *opaque);
JSValue js_module_ns_autoinit(JSContext *ctx, JSObject *p, JSAtom atom,
                              void *opaque);
JSValue JS_InstantiateFunctionListItem2(JSContext *ctx, JSObject *p,
                                        JSAtom atom, void *opaque);
void JS_SetUncatchableError(JSContext *ctx, JSValueConst val, BOOL flag);

#undef kill_dependency
#undef __exception
#undef __maybe_unused
#undef atomic_init
#undef atomic_is_lock_free
#undef atomic_load

#pragma GCC diagnostic pop

#endif
