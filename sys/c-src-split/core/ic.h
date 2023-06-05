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

#ifndef QUICKJS_IC_H
#define QUICKJS_IC_H

#include "quickjs/quickjs.h"
#include "shape.h"
#include "types.h"

InlineCache *init_ic(JSContext *ctx);
int rebuild_ic(InlineCache *ic);
int resize_ic_hash(InlineCache *ic);
int free_ic(InlineCache *ic);
uint32_t add_ic_slot(InlineCache *ic, JSAtom atom, JSObject *object,
                     uint32_t prop_offset);
uint32_t add_ic_slot1(InlineCache *ic, JSAtom atom);
force_inline int32_t get_ic_prop_offset(InlineCache *ic, uint32_t cache_offset,
                                        JSShape *shape) {
  uint32_t i;
  InlineCacheRingSlot *cr;
  InlineCacheRingItem *buffer;
  assert(cache_offset < ic->capacity);
  cr = ic->cache + cache_offset;
  i = cr->index;
  for (;;) {
    buffer = cr->buffer + i;
    if (likely(buffer->shape == shape)) {
      cr->index = i;
      return buffer->prop_offset;
    }

    i = (i + 1) % IC_CACHE_ITEM_CAPACITY;
    if (unlikely(i == cr->index)) {
      break;
    }
  }

  return -1;
}
force_inline JSAtom get_ic_atom(InlineCache *ic, uint32_t cache_offset) {
  assert(cache_offset < ic->capacity);
  return ic->cache[cache_offset].atom;
}

#endif