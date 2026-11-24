/* wasm32-unknown-unknown has no fcntl.h: quickjs only needs the include under
 * EMSCRIPTEN, where the open()/O_* paths are compiled out. Keep this empty so
 * no wasi-libc members (and their imports) get pulled into the link. */
