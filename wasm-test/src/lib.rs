//! The rquickjs safe API compiled for `wasm32-unknown-unknown`, exposed as a
//! minimal C ABI so `runner.ts` can drive a JS battery through it under deno.
//!
//! Every export mirrors what a real embedder would do: one runtime with a
//! memory limit, a max stack size (mandatory on wasm, see the README) and a
//! wall-clock interrupt handler.
use std::cell::OnceCell;
use std::ffi::{CStr, CString, c_char};
use std::sync::atomic::{AtomicU64, Ordering};

use rquickjs::{CatchResultExt, Context, Runtime, Value};

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn __rquickjs_host_now_us() -> f64;
}

/// wall-clock deadline in microseconds; 0 = no limit.
static DEADLINE_US: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ENGINE: OnceCell<(Runtime, Context)> = const { OnceCell::new() };
}

fn eval_json(src: &str) -> String {
    ENGINE.with(|cell| {
        let (runtime, context) = cell.get_or_init(|| {
            let runtime = Runtime::new().unwrap();
            // JS_DEFAULT_STACK_SIZE (1MB) equals the wasm shadow stack, so the
            // default `stack_top - 1MB` underflows: always set this on wasm.
            runtime.set_max_stack_size(256 * 1024);
            runtime.set_memory_limit(64 * 1024 * 1024);
            runtime.set_interrupt_handler(Some(Box::new(|| {
                let deadline = DEADLINE_US.load(Ordering::Relaxed);
                deadline != 0 && unsafe { __rquickjs_host_now_us() } as u64 > deadline
            })));
            let context = Context::full(&runtime).unwrap();
            (runtime, context)
        });
        let out = context.with(|ctx| match ctx.eval::<Value, _>(src).catch(&ctx) {
            Err(err) => format!("ERR:{err}"),
            Ok(value) => match ctx.json_stringify(value) {
                Ok(Some(json)) => format!("OK:{}", json.to_string().unwrap_or_default()),
                Ok(None) => "OK:undefined".to_string(),
                Err(err) => format!("ERR:stringify: {err}"),
            },
        });
        // drain the microtask queue so promise continuations observably run.
        while runtime.is_job_pending() {
            if runtime.execute_pending_job().is_err() {
                break;
            }
        }
        out
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rq_set_timeout_ms(ms: f64) {
    let deadline = if ms <= 0.0 {
        0
    } else {
        (unsafe { __rquickjs_host_now_us() } + ms * 1000.0) as u64
    };
    DEADLINE_US.store(deadline, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn rq_alloc(len: usize) -> *mut u8 {
    unsafe { std::alloc::alloc(std::alloc::Layout::from_size_align(len, 1).unwrap()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn rq_dealloc(ptr: *mut u8, len: usize) {
    unsafe { std::alloc::dealloc(ptr, std::alloc::Layout::from_size_align(len, 1).unwrap()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn rq_eval(src: *const c_char) -> *mut c_char {
    let src = unsafe { CStr::from_ptr(src) }.to_str().unwrap_or_default();
    CString::new(eval_json(src)).unwrap_or_default().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn rq_free(ptr: *mut c_char) {
    unsafe { drop(CString::from_raw(ptr)) }
}
