/*
 * QuickJS Javascript Engine
 *
 * Copyright (c) 2017-2021 Fabrice Bellard
 * Copyright (c) 2017-2021 Charlie Gordon
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

#include "js-json.h"
#include "../convertion.h"
#include "../exception.h"
#include "../function.h"
#include "../object.h"
#include "../parser.h"
#include "../runtime.h"
#include "../string.h"
#include "../types.h"
#include "js-array.h"
#include "js-function.h"
#include "js-object.h"

/* JSON */

int json_parse_expect(JSParseState *s, int tok)
{
  if (s->token.val != tok) {
    /* XXX: dump token correctly in all cases */
    return js_parse_error(s, "expecting '%c'", tok);
  }
  return json_next_token(s);
}

JSValue json_parse_value(JSParseState *s)
{
  JSContext *ctx = s->ctx;
  JSValue val = JS_NULL;
  int ret;

  switch(s->token.val) {
    case '{':
    {
      JSValue prop_val;
      JSAtom prop_name;

      if (json_next_token(s))
        goto fail;
      val = JS_NewObject(ctx);
      if (JS_IsException(val))
        goto fail;
      if (s->token.val != '}') {
        for(;;) {
          if (s->token.val == TOK_STRING) {
            prop_name = JS_ValueToAtom(ctx, s->token.u.str.str);
            if (prop_name == JS_ATOM_NULL)
              goto fail;
          } else if (s->ext_json && s->token.val == TOK_IDENT) {
            prop_name = JS_DupAtom(ctx, s->token.u.ident.atom);
          } else {
            js_parse_error(s, "expecting property name");
            goto fail;
          }
          if (json_next_token(s))
            goto fail1;
          if (json_parse_expect(s, ':'))
            goto fail1;
          prop_val = json_parse_value(s);
          if (JS_IsException(prop_val)) {
          fail1:
            JS_FreeAtom(ctx, prop_name);
            goto fail;
          }
          ret = JS_DefinePropertyValue(ctx, val, prop_name,
                                       prop_val, JS_PROP_C_W_E);
          JS_FreeAtom(ctx, prop_name);
          if (ret < 0)
            goto fail;

          if (s->token.val != ',')
            break;
          if (json_next_token(s))
            goto fail;
          if (s->ext_json && s->token.val == '}')
            break;
        }
      }
      if (json_parse_expect(s, '}'))
        goto fail;
    }
    break;
    case '[':
    {
      JSValue el;
      uint32_t idx;

      if (json_next_token(s))
        goto fail;
      val = JS_NewArray(ctx);
      if (JS_IsException(val))
        goto fail;
      if (s->token.val != ']') {
        idx = 0;
        for(;;) {
          el = json_parse_value(s);
          if (JS_IsException(el))
            goto fail;
          ret = JS_DefinePropertyValueUint32(ctx, val, idx, el, JS_PROP_C_W_E);
          if (ret < 0)
            goto fail;
          if (s->token.val != ',')
            break;
          if (json_next_token(s))
            goto fail;
          idx++;
          if (s->ext_json && s->token.val == ']')
            break;
        }
      }
      if (json_parse_expect(s, ']'))
        goto fail;
    }
    break;
    case TOK_STRING:
      val = JS_DupValue(ctx, s->token.u.str.str);
      if (json_next_token(s))
        goto fail;
      break;
    case TOK_NUMBER:
      val = s->token.u.num.val;
      if (json_next_token(s))
        goto fail;
      break;
    case TOK_IDENT:
      if (s->token.u.ident.atom == JS_ATOM_false ||
          s->token.u.ident.atom == JS_ATOM_true) {
        val = JS_NewBool(ctx, (s->token.u.ident.atom == JS_ATOM_true));
      } else if (s->token.u.ident.atom == JS_ATOM_null) {
        val = JS_NULL;
      } else {
        goto def_token;
      }
      if (json_next_token(s))
        goto fail;
      break;
    default:
    def_token:
      if (s->token.val == TOK_EOF) {
        js_parse_error(s, "unexpected end of input");
      } else {
        js_parse_error(s, "unexpected token: '%.*s'",
                       (int)(s->buf_ptr - s->token.ptr), s->token.ptr);
      }
      goto fail;
  }
  return val;
fail:
  JS_FreeValue(ctx, val);
  return JS_EXCEPTION;
}

JSValue JS_ParseJSON2(JSContext *ctx, const char *buf, size_t buf_len,
                      const char *filename, int flags)
{
  JSParseState s1, *s = &s1;
  JSValue val = JS_UNDEFINED;

  js_parse_init(ctx, s, buf, buf_len, filename);
  s->ext_json = ((flags & JS_PARSE_JSON_EXT) != 0);
  if (json_next_token(s))
    goto fail;
  val = json_parse_value(s);
  if (JS_IsException(val))
    goto fail;
  if (s->token.val != TOK_EOF) {
    if (js_parse_error(s, "unexpected data at the end"))
      goto fail;
  }
  return val;
fail:
  JS_FreeValue(ctx, val);
  free_token(s, &s->token);
  return JS_EXCEPTION;
}

JSValue JS_ParseJSON(JSContext *ctx, const char *buf, size_t buf_len,
                     const char *filename)
{
  return JS_ParseJSON2(ctx, buf, buf_len, filename, 0);
}

JSValue internalize_json_property(JSContext *ctx, JSValueConst holder,
                                         JSAtom name, JSValueConst reviver)
{
  JSValue val, new_el, name_val, res;
  JSValueConst args[2];
  int ret, is_array;
  uint32_t i, len = 0;
  JSAtom prop;
  JSPropertyEnum *atoms = NULL;

  if (js_check_stack_overflow(ctx->rt, 0)) {
    return JS_ThrowStackOverflow(ctx);
  }

  val = JS_GetProperty(ctx, holder, name);
  if (JS_IsException(val))
    return val;
  if (JS_IsObject(val)) {
    is_array = JS_IsArray(ctx, val);
    if (is_array < 0)
      goto fail;
    if (is_array) {
      if (js_get_length32(ctx, &len, val))
        goto fail;
    } else {
      ret = JS_GetOwnPropertyNamesInternal(ctx, &atoms, &len, JS_VALUE_GET_OBJ(val), JS_GPN_ENUM_ONLY | JS_GPN_STRING_MASK);
      if (ret < 0)
        goto fail;
    }
    for(i = 0; i < len; i++) {
      if (is_array) {
        prop = JS_NewAtomUInt32(ctx, i);
        if (prop == JS_ATOM_NULL)
          goto fail;
      } else {
        prop = JS_DupAtom(ctx, atoms[i].atom);
      }
      new_el = internalize_json_property(ctx, val, prop, reviver);
      if (JS_IsException(new_el)) {
        JS_FreeAtom(ctx, prop);
        goto fail;
      }
      if (JS_IsUndefined(new_el)) {
        ret = JS_DeleteProperty(ctx, val, prop, 0);
      } else {
        ret = JS_DefinePropertyValue(ctx, val, prop, new_el, JS_PROP_C_W_E);
      }
      JS_FreeAtom(ctx, prop);
      if (ret < 0)
        goto fail;
    }
  }
  js_free_prop_enum(ctx, atoms, len);
  atoms = NULL;
  name_val = JS_AtomToValue(ctx, name);
  if (JS_IsException(name_val))
    goto fail;
  args[0] = name_val;
  args[1] = val;
  res = JS_Call(ctx, reviver, holder, 2, args);
  JS_FreeValue(ctx, name_val);
  JS_FreeValue(ctx, val);
  return res;
fail:
  js_free_prop_enum(ctx, atoms, len);
  JS_FreeValue(ctx, val);
  return JS_EXCEPTION;
}

JSValue js_json_parse(JSContext *ctx, JSValueConst this_val,
                             int argc, JSValueConst *argv)
{
  JSValue obj, root;
  JSValueConst reviver;
  const char *str;
  size_t len;

  str = JS_ToCStringLen(ctx, &len, argv[0]);
  if (!str)
    return JS_EXCEPTION;
  obj = JS_ParseJSON(ctx, str, len, "<input>");
  JS_FreeCString(ctx, str);
  if (JS_IsException(obj))
    return obj;
  if (argc > 1 && JS_IsFunction(ctx, argv[1])) {
    reviver = argv[1];
    root = JS_NewObject(ctx);
    if (JS_IsException(root)) {
      JS_FreeValue(ctx, obj);
      return JS_EXCEPTION;
    }
    if (JS_DefinePropertyValue(ctx, root, JS_ATOM_empty_string, obj,
                               JS_PROP_C_W_E) < 0) {
      JS_FreeValue(ctx, root);
      return JS_EXCEPTION;
    }
    obj = internalize_json_property(ctx, root, JS_ATOM_empty_string,
                                    reviver);
    JS_FreeValue(ctx, root);
  }
  return obj;
}

typedef struct JSONStringifyContext {
  JSValueConst replacer_func;
  JSValue stack;
  JSValue property_list;
  JSValue gap;
  JSValue empty;
  StringBuffer *b;
} JSONStringifyContext;

JSValue JS_ToQuotedStringFree(JSContext *ctx, JSValue val) {
  JSValue r = JS_ToQuotedString(ctx, val);
  JS_FreeValue(ctx, val);
  return r;
}

JSValue js_json_check(JSContext *ctx, JSONStringifyContext *jsc,
                             JSValueConst holder, JSValue val, JSValueConst key)
{
  JSValue v;
  JSValueConst args[2];

  if (JS_IsObject(val)
#ifdef CONFIG_BIGNUM
      ||  JS_IsBigInt(ctx, val)   /* XXX: probably useless */
#endif
  ) {
    JSValue f = JS_GetProperty(ctx, val, JS_ATOM_toJSON);
    if (JS_IsException(f))
      goto exception;
    if (JS_IsFunction(ctx, f)) {
      v = JS_CallFree(ctx, f, val, 1, &key);
      JS_FreeValue(ctx, val);
      val = v;
      if (JS_IsException(val))
        goto exception;
    } else {
      JS_FreeValue(ctx, f);
    }
  }

  if (!JS_IsUndefined(jsc->replacer_func)) {
    args[0] = key;
    args[1] = val;
    v = JS_Call(ctx, jsc->replacer_func, holder, 2, args);
    JS_FreeValue(ctx, val);
    val = v;
    if (JS_IsException(val))
      goto exception;
  }

  switch (JS_VALUE_GET_NORM_TAG(val)) {
    case JS_TAG_OBJECT:
      if (JS_IsFunction(ctx, val))
        break;
    case JS_TAG_STRING:
    case JS_TAG_INT:
    case JS_TAG_FLOAT64:
#ifdef CONFIG_BIGNUM
    case JS_TAG_BIG_FLOAT:
#endif
    case JS_TAG_BOOL:
    case JS_TAG_NULL:
#ifdef CONFIG_BIGNUM
    case JS_TAG_BIG_INT:
#endif
    case JS_TAG_EXCEPTION:
      return val;
    default:
      break;
  }
  JS_FreeValue(ctx, val);
  return JS_UNDEFINED;

exception:
  JS_FreeValue(ctx, val);
  return JS_EXCEPTION;
}

int js_json_to_str(JSContext *ctx, JSONStringifyContext *jsc,
                          JSValueConst holder, JSValue val,
                          JSValueConst indent)
{
  JSValue indent1, sep, sep1, tab, v, prop;
  JSObject *p;
  int64_t i, len;
  int cl, ret;
  BOOL has_content;

  indent1 = JS_UNDEFINED;
  sep = JS_UNDEFINED;
  sep1 = JS_UNDEFINED;
  tab = JS_UNDEFINED;
  prop = JS_UNDEFINED;

  switch (JS_VALUE_GET_NORM_TAG(val)) {
    case JS_TAG_OBJECT:
      p = JS_VALUE_GET_OBJ(val);
      cl = p->class_id;
      if (cl == JS_CLASS_STRING) {
        val = JS_ToStringFree(ctx, val);
        if (JS_IsException(val))
          goto exception;
        val = JS_ToQuotedStringFree(ctx, val);
        if (JS_IsException(val))
          goto exception;
        return string_buffer_concat_value_free(jsc->b, val);
      } else if (cl == JS_CLASS_NUMBER) {
        val = JS_ToNumberFree(ctx, val);
        if (JS_IsException(val))
          goto exception;
        return string_buffer_concat_value_free(jsc->b, val);
      } else if (cl == JS_CLASS_BOOLEAN) {
        ret = string_buffer_concat_value(jsc->b, p->u.object_data);
        JS_FreeValue(ctx, val);
        return ret;
      }
#ifdef CONFIG_BIGNUM
      else if (cl == JS_CLASS_BIG_FLOAT) {
        return string_buffer_concat_value_free(jsc->b, val);
      } else if (cl == JS_CLASS_BIG_INT) {
        JS_ThrowTypeError(ctx, "bigint are forbidden in JSON.stringify");
        goto exception;
      }
#endif
      v = js_array_includes(ctx, jsc->stack, 1, (JSValueConst *)&val);
      if (JS_IsException(v))
        goto exception;
      if (JS_ToBoolFree(ctx, v)) {
        JS_ThrowTypeError(ctx, "circular reference");
        goto exception;
      }
      indent1 = JS_ConcatString(ctx, JS_DupValue(ctx, indent), JS_DupValue(ctx, jsc->gap));
      if (JS_IsException(indent1))
        goto exception;
      if (!JS_IsEmptyString(jsc->gap)) {
        sep = JS_ConcatString3(ctx, "\n", JS_DupValue(ctx, indent1), "");
        if (JS_IsException(sep))
          goto exception;
        sep1 = JS_NewString(ctx, " ");
        if (JS_IsException(sep1))
          goto exception;
      } else {
        sep = JS_DupValue(ctx, jsc->empty);
        sep1 = JS_DupValue(ctx, jsc->empty);
      }
      v = js_array_push(ctx, jsc->stack, 1, (JSValueConst *)&val, 0);
      if (check_exception_free(ctx, v))
        goto exception;
      ret = JS_IsArray(ctx, val);
      if (ret < 0)
        goto exception;
      if (ret) {
        if (js_get_length64(ctx, &len, val))
          goto exception;
        string_buffer_putc8(jsc->b, '[');
        for(i = 0; i < len; i++) {
          if (i > 0)
            string_buffer_putc8(jsc->b, ',');
          string_buffer_concat_value(jsc->b, sep);
          v = JS_GetPropertyInt64(ctx, val, i);
          if (JS_IsException(v))
            goto exception;
          /* XXX: could do this string conversion only when needed */
          prop = JS_ToStringFree(ctx, JS_NewInt64(ctx, i));
          if (JS_IsException(prop))
            goto exception;
          v = js_json_check(ctx, jsc, val, v, prop);
          JS_FreeValue(ctx, prop);
          prop = JS_UNDEFINED;
          if (JS_IsException(v))
            goto exception;
          if (JS_IsUndefined(v))
            v = JS_NULL;
          if (js_json_to_str(ctx, jsc, val, v, indent1))
            goto exception;
        }
        if (len > 0 && !JS_IsEmptyString(jsc->gap)) {
          string_buffer_putc8(jsc->b, '\n');
          string_buffer_concat_value(jsc->b, indent);
        }
        string_buffer_putc8(jsc->b, ']');
      } else {
        if (!JS_IsUndefined(jsc->property_list))
          tab = JS_DupValue(ctx, jsc->property_list);
        else
          tab = js_object_keys(ctx, JS_UNDEFINED, 1, (JSValueConst *)&val, JS_ITERATOR_KIND_KEY);
        if (JS_IsException(tab))
          goto exception;
        if (js_get_length64(ctx, &len, tab))
          goto exception;
        string_buffer_putc8(jsc->b, '{');
        has_content = FALSE;
        for(i = 0; i < len; i++) {
          JS_FreeValue(ctx, prop);
          prop = JS_GetPropertyInt64(ctx, tab, i);
          if (JS_IsException(prop))
            goto exception;
          v = JS_GetPropertyValue(ctx, val, JS_DupValue(ctx, prop));
          if (JS_IsException(v))
            goto exception;
          v = js_json_check(ctx, jsc, val, v, prop);
          if (JS_IsException(v))
            goto exception;
          if (!JS_IsUndefined(v)) {
            if (has_content)
              string_buffer_putc8(jsc->b, ',');
            prop = JS_ToQuotedStringFree(ctx, prop);
            if (JS_IsException(prop)) {
              JS_FreeValue(ctx, v);
              goto exception;
            }
            string_buffer_concat_value(jsc->b, sep);
            string_buffer_concat_value(jsc->b, prop);
            string_buffer_putc8(jsc->b, ':');
            string_buffer_concat_value(jsc->b, sep1);
            if (js_json_to_str(ctx, jsc, val, v, indent1))
              goto exception;
            has_content = TRUE;
          }
        }
        if (has_content && JS_VALUE_GET_STRING(jsc->gap)->len != 0) {
          string_buffer_putc8(jsc->b, '\n');
          string_buffer_concat_value(jsc->b, indent);
        }
        string_buffer_putc8(jsc->b, '}');
      }
      if (check_exception_free(ctx, js_array_pop(ctx, jsc->stack, 0, NULL, 0)))
        goto exception;
      JS_FreeValue(ctx, val);
      JS_FreeValue(ctx, tab);
      JS_FreeValue(ctx, sep);
      JS_FreeValue(ctx, sep1);
      JS_FreeValue(ctx, indent1);
      JS_FreeValue(ctx, prop);
      return 0;
    case JS_TAG_STRING:
      val = JS_ToQuotedStringFree(ctx, val);
      if (JS_IsException(val))
        goto exception;
      goto concat_value;
    case JS_TAG_FLOAT64:
      if (!isfinite(JS_VALUE_GET_FLOAT64(val))) {
        val = JS_NULL;
      }
      goto concat_value;
    case JS_TAG_INT:
#ifdef CONFIG_BIGNUM
    case JS_TAG_BIG_FLOAT:
#endif
    case JS_TAG_BOOL:
    case JS_TAG_NULL:
    concat_value:
      return string_buffer_concat_value_free(jsc->b, val);
#ifdef CONFIG_BIGNUM
    case JS_TAG_BIG_INT:
      JS_ThrowTypeError(ctx, "bigint are forbidden in JSON.stringify");
      goto exception;
#endif
    default:
      JS_FreeValue(ctx, val);
      return 0;
  }

exception:
  JS_FreeValue(ctx, val);
  JS_FreeValue(ctx, tab);
  JS_FreeValue(ctx, sep);
  JS_FreeValue(ctx, sep1);
  JS_FreeValue(ctx, indent1);
  JS_FreeValue(ctx, prop);
  return -1;
}

JSValue JS_JSONStringify(JSContext *ctx, JSValueConst obj,
                         JSValueConst replacer, JSValueConst space0)
{
  StringBuffer b_s;
  JSONStringifyContext jsc_s, *jsc = &jsc_s;
  JSValue val, v, space, ret, wrapper;
  int res;
  int64_t i, j, n;

  jsc->replacer_func = JS_UNDEFINED;
  jsc->stack = JS_UNDEFINED;
  jsc->property_list = JS_UNDEFINED;
  jsc->gap = JS_UNDEFINED;
  jsc->b = &b_s;
  jsc->empty = JS_AtomToString(ctx, JS_ATOM_empty_string);
  ret = JS_UNDEFINED;
  wrapper = JS_UNDEFINED;

  string_buffer_init(ctx, jsc->b, 0);
  jsc->stack = JS_NewArray(ctx);
  if (JS_IsException(jsc->stack))
    goto exception;
  if (JS_IsFunction(ctx, replacer)) {
    jsc->replacer_func = replacer;
  } else {
    res = JS_IsArray(ctx, replacer);
    if (res < 0)
      goto exception;
    if (res) {
      /* XXX: enumeration is not fully correct */
      jsc->property_list = JS_NewArray(ctx);
      if (JS_IsException(jsc->property_list))
        goto exception;
      if (js_get_length64(ctx, &n, replacer))
        goto exception;
      for (i = j = 0; i < n; i++) {
        JSValue present;
        v = JS_GetPropertyInt64(ctx, replacer, i);
        if (JS_IsException(v))
          goto exception;
        if (JS_IsObject(v)) {
          JSObject *p = JS_VALUE_GET_OBJ(v);
          if (p->class_id == JS_CLASS_STRING ||
              p->class_id == JS_CLASS_NUMBER) {
            v = JS_ToStringFree(ctx, v);
            if (JS_IsException(v))
              goto exception;
          } else {
            JS_FreeValue(ctx, v);
            continue;
          }
        } else if (JS_IsNumber(v)) {
          v = JS_ToStringFree(ctx, v);
          if (JS_IsException(v))
            goto exception;
        } else if (!JS_IsString(v)) {
          JS_FreeValue(ctx, v);
          continue;
        }
        present = js_array_includes(ctx, jsc->property_list,
                                    1, (JSValueConst *)&v);
        if (JS_IsException(present)) {
          JS_FreeValue(ctx, v);
          goto exception;
        }
        if (!JS_ToBoolFree(ctx, present)) {
          JS_SetPropertyInt64(ctx, jsc->property_list, j++, v);
        } else {
          JS_FreeValue(ctx, v);
        }
      }
    }
  }
  space = JS_DupValue(ctx, space0);
  if (JS_IsObject(space)) {
    JSObject *p = JS_VALUE_GET_OBJ(space);
    if (p->class_id == JS_CLASS_NUMBER) {
      space = JS_ToNumberFree(ctx, space);
    } else if (p->class_id == JS_CLASS_STRING) {
      space = JS_ToStringFree(ctx, space);
    }
    if (JS_IsException(space)) {
      JS_FreeValue(ctx, space);
      goto exception;
    }
  }
  if (JS_IsNumber(space)) {
    int n;
    if (JS_ToInt32Clamp(ctx, &n, space, 0, 10, 0))
      goto exception;
    jsc->gap = JS_NewStringLen(ctx, "          ", n);
  } else if (JS_IsString(space)) {
    JSString *p = JS_VALUE_GET_STRING(space);
    jsc->gap = js_sub_string(ctx, p, 0, min_int(p->len, 10));
  } else {
    jsc->gap = JS_DupValue(ctx, jsc->empty);
  }
  JS_FreeValue(ctx, space);
  if (JS_IsException(jsc->gap))
    goto exception;
  wrapper = JS_NewObject(ctx);
  if (JS_IsException(wrapper))
    goto exception;
  if (JS_DefinePropertyValue(ctx, wrapper, JS_ATOM_empty_string,
                             JS_DupValue(ctx, obj), JS_PROP_C_W_E) < 0)
    goto exception;
  val = JS_DupValue(ctx, obj);

  val = js_json_check(ctx, jsc, wrapper, val, jsc->empty);
  if (JS_IsException(val))
    goto exception;
  if (JS_IsUndefined(val)) {
    ret = JS_UNDEFINED;
    goto done1;
  }
  if (js_json_to_str(ctx, jsc, wrapper, val, jsc->empty))
    goto exception;

  ret = string_buffer_end(jsc->b);
  goto done;

exception:
  ret = JS_EXCEPTION;
done1:
  string_buffer_free(jsc->b);
done:
  JS_FreeValue(ctx, wrapper);
  JS_FreeValue(ctx, jsc->empty);
  JS_FreeValue(ctx, jsc->gap);
  JS_FreeValue(ctx, jsc->property_list);
  JS_FreeValue(ctx, jsc->stack);
  return ret;
}

JSValue js_json_stringify(JSContext *ctx, JSValueConst this_val,
                                 int argc, JSValueConst *argv)
{
  // stringify(val, replacer, space)
  return JS_JSONStringify(ctx, argv[0], argv[1], argv[2]);
}

const JSCFunctionListEntry js_json_funcs[] = {
    JS_CFUNC_DEF("parse", 2, js_json_parse ),
    JS_CFUNC_DEF("stringify", 3, js_json_stringify ),
    JS_PROP_STRING_DEF("[Symbol.toStringTag]", "JSON", JS_PROP_CONFIGURABLE ),
};

const JSCFunctionListEntry js_json_obj[] = {
    JS_OBJECT_DEF("JSON", js_json_funcs, countof(js_json_funcs), JS_PROP_WRITABLE | JS_PROP_CONFIGURABLE ),
};

void JS_AddIntrinsicJSON(JSContext *ctx)
{
  /* add JSON as autoinit object */
  JS_SetPropertyFunctionList(ctx, ctx->global_obj, js_json_obj, countof(js_json_obj));
}