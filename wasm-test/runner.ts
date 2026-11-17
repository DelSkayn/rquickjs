// Drives the rquickjs wasm32-unknown-unknown build under deno.
//
//   cargo build --manifest-path wasm-test/Cargo.toml --release \
//     --target wasm32-unknown-unknown
//   deno run --allow-read wasm-test/runner.ts
//
// Asserts two things: the module imports nothing but the host clock, and the
// embedded engine passes a battery covering language features, determinism and
// the three containment limits (cpu, memory, stack).
const wasmPath = Deno.args[0] ??
  new URL(
    "./target/wasm32-unknown-unknown/release/rquickjs_wasm_test.wasm",
    import.meta.url,
  ).pathname;

const bytes = await Deno.readFile(wasmPath);
const module = new WebAssembly.Module(bytes);

let failed = 0;
function check(name: string, ok: boolean, detail: string) {
  console.log(`${ok ? "pass" : "FAIL"}  ${name.padEnd(18)} ${detail}`);
  if (!ok) failed++;
}

// ---------------------------------------------------------------- imports
const imports = WebAssembly.Module.imports(module)
  .map((imp) => `${imp.module}.${imp.name}`)
  .sort();
console.log(`wasm size: ${(bytes.length / 1024).toFixed(0)} KiB`);
check(
  "imports",
  imports.length === 1 && imports[0] === "env.__rquickjs_host_now_us",
  `[${imports.join(", ")}]`,
);

// ---------------------------------------------------------------- instance
const instance = new WebAssembly.Instance(module, {
  env: { __rquickjs_host_now_us: () => Date.now() * 1000 },
});
const exports = instance.exports as {
  memory: WebAssembly.Memory;
  rq_alloc: (len: number) => number;
  rq_dealloc: (ptr: number, len: number) => void;
  rq_eval: (src: number) => number;
  rq_free: (ptr: number) => void;
  rq_set_timeout_ms: (ms: number) => void;
};

const enc = new TextEncoder();
const dec = new TextDecoder();

function evalJs(src: string): string {
  const srcBytes = enc.encode(src);
  const ptr = exports.rq_alloc(srcBytes.length + 1);
  const inView = new Uint8Array(exports.memory.buffer, ptr, srcBytes.length + 1);
  inView.set(srcBytes);
  inView[srcBytes.length] = 0;
  const out = exports.rq_eval(ptr);
  // reread the buffer: the eval may have grown memory, detaching old views.
  const outView = new Uint8Array(exports.memory.buffer);
  let end = out;
  while (outView[end] !== 0) end++;
  const result = dec.decode(outView.subarray(out, end));
  exports.rq_free(out);
  exports.rq_dealloc(ptr, srcBytes.length + 1);
  return result;
}

// ---------------------------------------------------------------- battery
// each case is [name, source, expected `rq_eval` output].
const cases: [string, string, string][] = [
  ["arith", "1 + 41", "OK:42"],
  ["string", "'hello ' + 'world'", 'OK:"hello world"'],
  ["json", "JSON.parse('{\"a\":[1,2,3]}').a[2]", "OK:3"],
  ["class", "class A { #x = 41; get x() { return this.#x + 1 } } new A().x", "OK:42"],
  ["bigint", "(2n ** 64n).toString()", 'OK:"18446744073709551616"'],
  ["regexp", "/w(or)ld/.exec('hello world')[1]", 'OK:"or"'],
  ["regexp-unicode", "/\\p{Script=Greek}+/u.exec('abcαβγ')[0]", 'OK:"αβγ"'],
  ["unicode", "'\\u{1F600}'.codePointAt(0).toString(16)", 'OK:"1f600"'],
  ["unicode-norm", "'e\\u0301'.normalize('NFC') === '\\u00e9'", "OK:true"],
  ["math-libm", "Math.pow(2, 10) + Math.log(Math.E) + Math.sin(0)", "OK:1025"],
  ["float-fmt", "(0.1 + 0.2).toString()", 'OK:"0.30000000000000004"'],
  ["float-max", "Number.MAX_VALUE.toString()", 'OK:"1.7976931348623157e+308"'],
  ["strtod", "Number('3.14159') * 2", "OK:6.28318"],
  ["tofixed", "(1234.5678).toFixed(2)", 'OK:"1234.57"'],
  ["toprecision", "(0.000123456).toPrecision(4)", 'OK:"0.0001235"'],
  ["date-now", "const t = Date.now(); t > 1e12 && t < 1e13", "OK:true"],
  ["date-iso", "new Date(86400000).toISOString()", 'OK:"1970-01-02T00:00:00.000Z"'],
  ["tz-offset", "new Date().getTimezoneOffset()", "OK:0"],
  [
    "promise-setup",
    "globalThis.pr = 0; Promise.resolve(41).then(v => { globalThis.pr = v + 1 }); 'scheduled'",
    'OK:"scheduled"',
  ],
  ["promise-drained", "pr", "OK:42"],
  [
    "async-await",
    "globalThis.aw = 0; (async () => { aw = await Promise.resolve(7) })(); 'started'",
    'OK:"started"',
  ],
  ["async-drained", "aw", "OK:7"],
  ["error", "throw new Error('boom')", "ERR:Error: boom"],
  [
    "deep-recursion",
    "function f(n) { return f(n + 1) } try { f(0) } catch (e) { e.name }",
    'OK:"RangeError"',
  ],
  ["isolation-fetch", "typeof fetch === 'undefined'", "OK:true"],
  ["isolation-deno", "typeof Deno === 'undefined'", "OK:true"],
  ["isolation-require", "typeof require === 'undefined'", "OK:true"],
];

for (const [name, src, expected] of cases) {
  try {
    const actual = evalJs(src);
    // the error case carries a stack trace tail; compare on the prefix.
    const ok = actual === expected || actual.startsWith(expected + "\n");
    check(name, ok, ok ? actual.split("\n")[0] : `${actual} != ${expected}`);
  } catch (err) {
    check(name, false, `HOST TRAP: ${(err as Error).message}`);
  }
}

// ---------------------------------------------------------------- cpu limit
exports.rq_set_timeout_ms(200);
const spinStart = performance.now();
const spin = evalJs("while (true) {}");
const spinMs = performance.now() - spinStart;
exports.rq_set_timeout_ms(0);
check(
  "cpu-interrupt",
  spin.startsWith("ERR:") && spinMs > 100 && spinMs < 2000,
  `${spin.split("\n")[0]} after ${spinMs.toFixed(0)}ms`,
);

// ------------------------------------------------------------- memory limit
const bomb = evalJs(
  "try { const arr = []; while (true) arr.push(new Array(65536).fill(1)); } catch (e) { 'contained: ' + e.message }",
);
check("memory-limit", bomb.startsWith('OK:"contained: '), bomb.split("\n")[0]);

// the engine is still usable after every limit fired.
check("still-alive", evalJs("1 + 1") === "OK:2", evalJs("1 + 1"));

console.log(failed === 0 ? "\nall cases passed" : `\n${failed} case(s) FAILED`);
if (failed > 0) Deno.exit(1);
