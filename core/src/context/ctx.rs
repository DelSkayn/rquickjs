#[cfg(feature = "futures")]
use core::future::Future;

use alloc::{boxed::Box, ffi::CString, vec::Vec};
use core::{
    any::Any,
    ffi::{c_char, CStr},
    mem::{self, MaybeUninit},
    panic::AssertUnwindSafe,
    ptr::NonNull,
    result::Result as StdResult,
};

#[cfg(feature = "std")]
use std::{fs, path::Path, string::String as StdString};

#[cfg(feature = "futures")]
use crate::AsyncContext;
use crate::{
    markers::Invariant,
    qjs,
    runtime::{opaque::Opaque, UserDataError, UserDataGuard},
    Atom, Error, FromJs, Function, IntoJs, JsLifetime, Object, Promise, Result, String, Value,
};

use super::Context;

/// Eval options.
#[non_exhaustive]
pub struct EvalOptions {
    /// Global code.
    pub global: bool,
    /// Force 'strict' mode.
    pub strict: bool,
    /// Don't include the stack frames before this eval in the Error() backtraces.
    pub backtrace_barrier: bool,
    /// Support top-level-await.
    pub promise: bool,
    /// Filename. Ignored when calling eval_file_*.
    #[cfg(feature = "std")]
    pub filename: Option<StdString>,
}

impl EvalOptions {
    fn to_flag(&self) -> i32 {
        let mut flag = if self.global {
            qjs::JS_EVAL_TYPE_GLOBAL
        } else {
            qjs::JS_EVAL_TYPE_MODULE
        };

        if self.strict {
            flag |= qjs::JS_EVAL_FLAG_STRICT;
        }

        if self.backtrace_barrier {
            flag |= qjs::JS_EVAL_FLAG_BACKTRACE_BARRIER;
        }

        if self.promise {
            flag |= qjs::JS_EVAL_FLAG_ASYNC;
        }

        flag as i32
    }
}

impl Default for EvalOptions {
    fn default() -> Self {
        EvalOptions {
            global: true,
            strict: true,
            backtrace_barrier: false,
            promise: false,
            #[cfg(feature = "std")]
            filename: None,
        }
    }
}

/// The kind of internal job drain handing a failed job to
/// [`Ctx::handle_failed_job`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JobDrain {
    /// [`Ctx::execute_pending_job`]: the engine's pending-exception slot is
    /// left clear after the failure is handled.
    Sync,
    /// A drain driving futures (the `async_with` driver, the
    /// `AsyncRuntime::drive` future, `AsyncRuntime::idle`): an uncatchable
    /// error is left pending in the engine after the failure is handled.
    #[cfg_attr(not(feature = "futures"), allow(dead_code))]
    Async,
}

/// Context in use, passed to [`Context::with`].
#[derive(Debug)]
pub struct Ctx<'js> {
    ctx: NonNull<qjs::JSContext>,
    _marker: Invariant<'js>,
}

impl<'js> Clone for Ctx<'js> {
    fn clone(&self) -> Self {
        unsafe { qjs::JS_DupContext(self.ctx.as_ptr()) };
        Ctx {
            ctx: self.ctx,
            _marker: self._marker,
        }
    }
}

impl<'js> Drop for Ctx<'js> {
    fn drop(&mut self) {
        unsafe { qjs::JS_FreeContext(self.ctx.as_ptr()) };
    }
}

unsafe impl Send for Ctx<'_> {}

impl<'js> Ctx<'js> {
    pub(crate) fn as_ptr(&self) -> *mut qjs::JSContext {
        self.ctx.as_ptr()
    }

    pub(crate) unsafe fn from_ptr(ctx: *mut qjs::JSContext) -> Self {
        unsafe { qjs::JS_DupContext(ctx) };
        let ctx = NonNull::new_unchecked(ctx);
        Ctx {
            ctx,
            _marker: Invariant::new(),
        }
    }

    pub(crate) unsafe fn new(ctx: &'js Context) -> Self {
        unsafe { qjs::JS_DupContext(ctx.0.ctx().as_ptr()) };
        Ctx {
            ctx: ctx.0.ctx(),
            _marker: Invariant::new(),
        }
    }

    #[cfg(feature = "futures")]
    pub(crate) unsafe fn new_async(ctx: &'js AsyncContext) -> Self {
        unsafe { qjs::JS_DupContext(ctx.0.ctx().as_ptr()) };
        Ctx {
            ctx: ctx.0.ctx(),
            _marker: Invariant::new(),
        }
    }

    pub(crate) unsafe fn eval_raw<S: Into<Vec<u8>>>(
        &self,
        source: S,
        file_name: &CStr,
        flag: i32,
    ) -> Result<qjs::JSValue> {
        let mut src: Vec<u8> = source.into();
        let len = src.len();
        // JS_Eval requires the input to be null-terminated,
        // but internal \0 bytes are allowed
        src.push(0);

        #[cfg(feature = "parallel")]
        qjs::JS_UpdateStackTop(qjs::JS_GetRuntime(self.ctx.as_ptr()));

        let val = qjs::JS_Eval(
            self.ctx.as_ptr(),
            src.as_ptr().cast::<c_char>(),
            len as _,
            file_name.as_ptr(),
            flag,
        );
        self.handle_exception(val)
    }

    /// Evaluate a script in global context.
    pub fn eval<V: FromJs<'js>, S: Into<Vec<u8>>>(&self, source: S) -> Result<V> {
        self.eval_with_options(source, Default::default())
    }

    /// Evaluate a script in global context with top level await support.
    ///
    /// This function always returns a promise which resolves to the result of the evaluated
    /// expression.
    pub fn eval_promise<S: Into<Vec<u8>>>(&self, source: S) -> Result<Promise<'js>> {
        self.eval_with_options(
            source,
            EvalOptions {
                promise: true,
                ..Default::default()
            },
        )
    }

    /// Evaluate a script with the given options.
    pub fn eval_with_options<V: FromJs<'js>, S: Into<Vec<u8>>>(
        &self,
        source: S,
        options: EvalOptions,
    ) -> Result<V> {
        #[cfg(feature = "std")]
        let file_name = {
            if let Some(filename) = &options.filename {
                &CString::new(filename.clone())?
            } else {
                c"eval_script"
            }
        };

        #[cfg(not(feature = "std"))]
        let file_name = c"eval_script";

        V::from_js(self, unsafe {
            let val = self.eval_raw(source, file_name, options.to_flag())?;
            Value::from_js_value(self.clone(), val)
        })
    }

    #[cfg(feature = "std")]
    /// Evaluate a script directly from a file.
    pub fn eval_file<V: FromJs<'js>, P: AsRef<Path>>(&self, path: P) -> Result<V> {
        self.eval_file_with_options(path, Default::default())
    }

    #[cfg(feature = "std")]
    pub fn eval_file_with_options<V: FromJs<'js>, P: AsRef<Path>>(
        &self,
        path: P,
        options: EvalOptions,
    ) -> Result<V> {
        let buffer = fs::read(path.as_ref())?;
        let file_name = CString::new(
            path.as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )?;

        V::from_js(self, unsafe {
            let val = self.eval_raw(buffer, file_name.as_c_str(), options.to_flag())?;
            Value::from_js_value(self.clone(), val)
        })
    }

    /// Returns the global object of this context.
    pub fn globals(&self) -> Object<'js> {
        unsafe {
            let v = qjs::JS_GetGlobalObject(self.ctx.as_ptr());
            Object::from_js_value(self.clone(), v)
        }
    }

    /// Returns the last raised JavaScript exception, if there is no exception the JavaScript value `null` is returned.
    ///
    /// # Usage
    /// ```
    /// # use rquickjs::{Error, Context, Runtime};
    /// # let rt = Runtime::new().unwrap();
    /// # let ctx = Context::full(&rt).unwrap();
    /// # ctx.with(|ctx|{
    /// if let Err(Error::Exception) = ctx.eval::<(),_>("throw 3"){
    ///     assert_eq!(ctx.catch().as_int(),Some(3));
    /// # }else{
    /// #    panic!()
    /// }
    /// # });
    /// ```
    pub fn catch(&self) -> Value<'js> {
        unsafe {
            let v = qjs::JS_GetException(self.ctx.as_ptr());
            Value::from_js_value(self.clone(), v)
        }
    }

    /// Returns true if there is a pending JavaScript exception.
    pub fn has_exception(&self) -> bool {
        unsafe { qjs::JS_HasException(self.ctx.as_ptr()) }
    }

    /// Throws a JavaScript value as a new exception.
    /// Always returns `Error::Exception`;
    pub fn throw(&self, value: Value<'js>) -> Error {
        unsafe {
            let v = value.into_js_value();
            qjs::JS_Throw(self.ctx.as_ptr(), v);
        }
        Error::Exception
    }

    /// Takes the exception of the first job that failed in an internal job
    /// drain (the `async_with` driver, the `AsyncRuntime::drive` future,
    /// `AsyncRuntime::idle`, or [`Ctx::execute_pending_job`]) since the
    /// previous take, leaving nothing retained. A job that failed without an
    /// exception value (e.g. a Rust callback returned [`Error::Exception`]
    /// without throwing) is retained as `undefined`.
    ///
    /// Retention and the job error handler (`Runtime::set_job_error_handler`
    /// or `AsyncRuntime::set_job_error_handler`) are alternatives: a failure
    /// that occurs while a handler is set is handed to the handler only and
    /// is never retained.
    ///
    /// The retained error is runtime-wide, not per context: the returned
    /// value belongs to the context the failed job ran in, which may be
    /// another context of the same runtime. Until it is taken, the retained
    /// value keeps its object graph and that context alive. It is dropped
    /// with the runtime if never taken.
    ///
    /// Failures surfaced by the runtime's own `execute_pending_job` methods
    /// are handed to the caller instead and not retained. Failures of jobs
    /// run while the runtime is being dropped (e.g. drained by the `Drop` of
    /// a spawned future or of userdata) are discarded: neither retained nor
    /// counted.
    pub fn take_pending_job_error(&self) -> Option<Value<'js>> {
        unsafe { self.get_opaque() }.take_job_error()
    }

    /// Returns the number of job failures that could not be retained for
    /// [`Ctx::take_pending_job_error`] because an earlier failure was still
    /// retained, since the previous call. The count saturates at `u64::MAX`
    /// and is reset to 0.
    ///
    /// Like the retained error, the count is runtime-wide.
    ///
    /// A failure handed to the job error handler is neither retained nor
    /// counted; without a handler this count is the only record of failures
    /// beyond the first.
    pub fn take_pending_job_error_overflow(&self) -> u64 {
        unsafe { self.get_opaque() }.take_job_error_overflow()
    }

    /// Handle a job that failed in a drain; `self` is the context the job ran
    /// in, which need not be the context the drain was started on, and
    /// `drain` is the kind of drain that ran it.
    ///
    /// The failure's exception is taken out of the engine's pending-exception
    /// slot. If it is the synthetic value thrown for a stashed Rust panic
    /// (norm tag `JS_TAG_EXCEPTION`) and a panic is stashed, the stash is
    /// attributed to this failure: its payload is resumed and this function
    /// never returns. JS cannot construct that value, but it can capture one
    /// with `catch` and rethrow it later; such a rethrow resumes whichever
    /// payload is stashed at that moment, which need not be the one the
    /// value was originally thrown for. This matches the crate's existing
    /// host-error crossings, which resume any stashed payload.
    ///
    /// Any other failure is reported as its exception, or as `undefined` if
    /// the job failed without one (e.g. a Rust callback returned
    /// [`Error::Exception`] without throwing, or JS rethrew a captured
    /// synthetic panic value whose payload was already consumed). If a job
    /// error handler is set, the failure is handed to it only. Otherwise it
    /// is retained, first-wins, for [`Ctx::take_pending_job_error`], or
    /// counted for [`Ctx::take_pending_job_error_overflow`] if an earlier
    /// failure is still retained.
    ///
    /// For [`JobDrain::Async`], an uncatchable error (e.g. raised by the
    /// interrupt handler) is then thrown back into the pending-exception slot
    /// so that a `PromiseFuture` awaiting a promise the failed job never
    /// settled still observes it and returns an error instead of waiting
    /// forever. For [`JobDrain::Sync`] the slot is left clear.
    ///
    /// If the handler panics, the payload is resumed out of this function
    /// (and so out of the drain) after the above; the panic stash is not
    /// touched.
    ///
    /// The drain does not resume a panic whose synthetic exception JS caught
    /// and did not rethrow; that payload stays stashed and is resumed at the
    /// next host error crossing (e.g. a later `eval` that throws), or
    /// dropped with the runtime.
    pub(crate) fn handle_failed_job(&self, drain: JobDrain) {
        // Take the exception out of the engine's pending-exception slot so
        // unrelated code can't later mistake it for a fresh one (e.g. the
        // module loader's "is a module already loaded" check).
        let raw = unsafe { qjs::JS_GetException(self.as_ptr()) };
        // Safety: the caller holds the runtime lock for the whole drain.
        let opaque = unsafe { self.get_opaque() };
        let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(raw) };
        if tag == qjs::JS_TAG_EXCEPTION {
            if let Some(payload) = opaque.take_panic() {
                // Safety: the runtime lock is held for the whole drain and
                // the engine is quiescent between jobs with the
                // pending-exception slot just cleared, so no engine or
                // `Opaque` mutation is in flight when the unwind crosses
                // the drain.
                crate::util::resume_unwind(payload);
            }
        }
        // `JS_TAG_UNINITIALIZED` means no exception was pending; a sentinel
        // with an empty stash (JS rethrowing a captured sentinel after the
        // stash was consumed) carries no value either. Neither is a usable
        // JS value, and neither is reference counted, so `raw` needs no free.
        let error = if tag == qjs::JS_TAG_UNINITIALIZED || tag == qjs::JS_TAG_EXCEPTION {
            Value::new_undefined(self.clone())
        } else {
            unsafe { Value::from_js_value(self.clone(), raw) }
        };
        // The handler gets its own handle to the error, so it may re-enter a
        // drain; the handler and retention are alternatives.
        let handler_panic = match crate::util::catch_unwind(AssertUnwindSafe(|| {
            opaque.run_job_error_handler(self.clone(), error.clone())
        })) {
            Ok(true) => None,
            Ok(false) => {
                opaque.retain_job_error(error.clone());
                None
            }
            Err(payload) => Some(payload),
        };
        if drain == JobDrain::Async && error.is_uncatchable_error() {
            // Leave the uncatchable error pending, as the engine reported it,
            // so `PromiseFuture` bails out on it.
            self.throw(error);
        }
        if let Some(payload) = handler_panic {
            // Safety: as above; the handler has returned and the bookkeeping
            // for this failure is complete.
            crate::util::resume_unwind(payload);
        }
    }

    /// Parse json into a JavaScript value.
    pub fn json_parse<S>(&self, json: S) -> Result<Value<'js>>
    where
        S: Into<Vec<u8>>,
    {
        let src = json.into();
        let len = src.len();
        let src = CString::new(src)?;
        unsafe {
            let name = b"<input>\0";
            let v = qjs::JS_ParseJSON(
                self.as_ptr(),
                src.as_ptr().cast(),
                len.try_into().expect(qjs::SIZE_T_ERROR),
                name.as_ptr().cast(),
            );
            self.handle_exception(v)?;
            Ok(Value::from_js_value(self.clone(), v))
        }
    }

    /// Stringify a JavaScript value into its JSON representation
    pub fn json_stringify<V>(&self, value: V) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
    {
        self.json_stringify_inner(&value.into_js(self)?, qjs::JS_UNDEFINED, qjs::JS_UNDEFINED)
    }

    /// Stringify a JavaScript value into its JSON representation with a possible replacer.
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    pub fn json_stringify_replacer<V, R>(
        &self,
        value: V,
        replacer: R,
    ) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
    {
        let replacer = replacer.into_js(self)?;

        self.json_stringify_inner(
            &value.into_js(self)?,
            replacer.as_js_value(),
            qjs::JS_UNDEFINED,
        )
    }

    /// Stringify a JavaScript value into its JSON representation with a possible replacer and
    /// spaces
    ///
    /// The replacer is the same as the replacer argument for `JSON.stringify`.
    /// It is is a function that alters the behavior of the stringification process.
    ///
    /// Space is either a number or a string which is used to insert whitespace into the output
    /// string for readability purposes. This behaves the same as the space argument for
    /// `JSON.stringify`.
    pub fn json_stringify_replacer_space<V, R, S>(
        &self,
        value: V,
        replacer: R,
        space: S,
    ) -> Result<Option<String<'js>>>
    where
        V: IntoJs<'js>,
        R: IntoJs<'js>,
        S: IntoJs<'js>,
    {
        let replacer = replacer.into_js(self)?;
        let space = space.into_js(self)?;

        self.json_stringify_inner(
            &value.into_js(self)?,
            replacer.as_js_value(),
            space.as_js_value(),
        )
    }

    // Inner non-generic version of json stringify>
    fn json_stringify_inner(
        &self,
        value: &Value<'js>,
        replacer: qjs::JSValueConst,
        space: qjs::JSValueConst,
    ) -> Result<Option<String<'js>>> {
        unsafe {
            let res = qjs::JS_JSONStringify(self.as_ptr(), value.as_js_value(), replacer, space);
            self.handle_exception(res)?;
            let v = Value::from_js_value(self.clone(), res);
            if v.is_undefined() {
                Ok(None)
            } else {
                let v = v.into_string().expect(
                    "JS_JSONStringify did not return either an exception, undefined, or a string",
                );
                Ok(Some(v))
            }
        }
    }

    /// Creates javascipt promise along with its reject and resolve functions.
    pub fn promise(&self) -> Result<(Promise<'js>, Function<'js>, Function<'js>)> {
        let mut funcs = mem::MaybeUninit::<[qjs::JSValue; 2]>::uninit();

        Ok(unsafe {
            let promise = self.handle_exception(qjs::JS_NewPromiseCapability(
                self.ctx.as_ptr(),
                funcs.as_mut_ptr() as _,
            ))?;
            let [resolve, reject] = funcs.assume_init();
            (
                Promise::from_js_value(self.clone(), promise),
                Function::from_js_value(self.clone(), resolve),
                Function::from_js_value(self.clone(), reject),
            )
        })
    }

    /// Executes a quickjs job.
    ///
    /// Returns wether a job was actually executed.
    /// If this function returned false, no job was pending.
    ///
    /// The job may belong to any context of the runtime. If it fails, its
    /// exception is taken out of the engine and handed to the job error
    /// handler if one is set, or otherwise retained for
    /// [`Ctx::take_pending_job_error`]; it is not left pending. If it
    /// failed because a Rust callback panicked, the stashed panic is resumed
    /// and this function does not return. A panic in the job error handler
    /// also propagates out of this function.
    pub fn execute_pending_job(&self) -> bool {
        let mut ptr = MaybeUninit::<*mut qjs::JSContext>::uninit();
        let rt = unsafe { qjs::JS_GetRuntime(self.ctx.as_ptr()) };
        let res = unsafe { qjs::JS_ExecutePendingJob(rt, ptr.as_mut_ptr()) };
        if res < 0 {
            // A negative result means the job threw, and `JS_ExecutePendingJob`
            // leaves the exception pending in the runtime-wide exception slot
            // and reports the context the job ran in (`*ptr`, which need not
            // be `self` in a multi-context runtime) instead of surfacing it
            // here.
            let ctx = NonNull::new(unsafe { ptr.assume_init() })
                .expect("executing pending job returned a null context on error");
            let ctx = unsafe { Self::from_raw(ctx) };
            ctx.handle_failed_job(JobDrain::Sync);
        }
        res != 0
    }

    pub(crate) unsafe fn get_opaque(&self) -> &Opaque<'js> {
        Opaque::from_runtime_ptr(qjs::JS_GetRuntime(self.ctx.as_ptr()))
    }

    /// Spawn future using configured async runtime
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'js,
    {
        unsafe { self.get_opaque().push(future) }
    }

    /// Create a new `Ctx` from a pointer to the context and a invariant lifetime.
    ///
    /// # Safety
    /// User must ensure that a lock was acquired over the runtime and that invariant is a unique
    /// lifetime which can't be coerced to a lifetime outside the scope of the lock of to the
    /// lifetime of another runtime.
    pub unsafe fn from_raw_invariant(ctx: NonNull<qjs::JSContext>, inv: Invariant<'js>) -> Self {
        unsafe { qjs::JS_DupContext(ctx.as_ptr()) };
        Ctx { ctx, _marker: inv }
    }

    /// Create a new `Ctx` from a pointer to the context and a invariant lifetime.
    ///
    /// # Safety
    /// User must ensure that a lock was acquired over the runtime and that invariant is a unique
    /// lifetime which can't be coerced to a lifetime outside the scope of the lock of to the
    /// lifetime of another runtime.
    pub unsafe fn from_raw(ctx: NonNull<qjs::JSContext>) -> Self {
        unsafe { qjs::JS_DupContext(ctx.as_ptr()) };
        Ctx {
            ctx,
            _marker: Invariant::new(),
        }
    }

    /// Returns the name of the current module or script that is running.
    ///
    /// It called from a javascript callback it will return the current running javascript script
    /// name.
    /// Otherwise it will return none.
    pub fn script_or_module_name(&self, stack_level: isize) -> Option<Atom<'js>> {
        let stack_level = core::ffi::c_int::try_from(stack_level).unwrap();
        let atom = unsafe { qjs::JS_GetScriptOrModuleName(self.as_ptr(), stack_level) };
        #[allow(clippy::useless_conversion)] //needed for multi platform binding support
        if qjs::__JS_ATOM_NULL == atom.try_into().unwrap() {
            unsafe { qjs::JS_FreeAtom(self.as_ptr(), atom) };
            return None;
        }
        unsafe { Some(Atom::from_atom_val(self.clone(), atom)) }
    }

    /// Runs the quickjs garbage collector for a cycle.
    ///
    /// Quickjs uses reference counting with a collection cycle for cyclic references.
    /// This runs the cyclic reference collector cycle, types which are not part of a reference cycle
    /// will be freed the momement their reference count becomes zero.
    pub fn run_gc(&self) {
        unsafe { qjs::JS_RunGC(qjs::JS_GetRuntime(self.ctx.as_ptr())) }
    }

    /// Store a type in the runtime which can be retrieved later with `Ctx::userdata`.
    ///
    /// Returns the value from the argument if the userdata is currently being accessed and
    /// insertion is not possible.
    /// Otherwise returns the exising value for this type if it existed.
    pub fn store_userdata<U>(&self, data: U) -> StdResult<Option<Box<U>>, UserDataError<U>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        unsafe { self.get_opaque().insert_userdata(data) }
    }

    /// Remove the userdata of the given type from the userdata storage.
    ///
    /// Returns Err(()) if the userdata is currently being accessed and removing isn't possible.
    /// Returns Ok(None) if userdata of the given type wasn't inserted.
    pub fn remove_userdata<U>(&self) -> StdResult<Option<Box<U>>, UserDataError<()>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        unsafe { self.get_opaque().remove_userdata() }
    }

    /// Retrieves a borrow to the userdata of the given type from the userdata storage.
    ///
    /// Returns None if userdata of the given type wasn't inserted.
    pub fn userdata<U>(&self) -> Option<UserDataGuard<U>>
    where
        U: JsLifetime<'js>,
        U::Changed<'static>: Any,
    {
        unsafe { self.get_opaque().get_userdata() }
    }

    /// Returns the pointer to the C library context.
    pub fn as_raw(&self) -> NonNull<qjs::JSContext> {
        self.ctx
    }
}

#[cfg(test)]
mod test {
    use crate::{CatchResultExt, JsLifetime};

    #[test]
    fn exports() {
        use crate::{context::intrinsic, Context, Function, Module, Promise, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::custom::<(intrinsic::Promise, intrinsic::Eval)>(&runtime).unwrap();
        ctx.with(|ctx| {
            let (module, promise) = Module::declare(ctx, "test", "export default async () => 1;")
                .unwrap()
                .eval()
                .unwrap();
            promise.finish::<()>().unwrap();
            let func: Function = module.get("default").unwrap();
            func.call::<(), Promise>(()).unwrap();
        });
    }

    #[test]
    fn execute_pending_job_clears_exception_from_failed_job() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            // Schedule a job that throws when it runs.
            ctx.eval::<(), _>("queueMicrotask(() => { throw new Error('boom'); });")
                .unwrap();

            // Run it: the job throws, but that must not leave a pending
            // exception on the context for unrelated code to trip over later.
            while ctx.execute_pending_job() {}

            assert!(!ctx.has_exception());

            // The context must still be perfectly usable afterwards.
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn execute_pending_job_retains_failed_job_exception() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            // A raw job's failure would otherwise be unobservable.
            ctx.eval::<(), _>("queueMicrotask(() => { throw 42; });")
                .unwrap();

            while ctx.execute_pending_job() {}

            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(42));
            // Taking the retained error clears it.
            assert!(ctx.take_pending_job_error().is_none());
        })
    }

    #[test]
    fn execute_pending_job_retains_first_failed_job_exception() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });",
            )
            .unwrap();

            // Both jobs fail, but only the earliest failure is retained.
            while ctx.execute_pending_job() {}

            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(1));
        })
    }

    #[test]
    fn execute_pending_job_calls_job_error_handler_in_order() {
        use crate::{Context, Runtime};
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let runtime = Runtime::new().unwrap();
        {
            let seen = seen.clone();
            runtime.set_job_error_handler(Some(Box::new(move |_, error| {
                seen.lock().unwrap().push(error.as_int());
            })));
        }
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });",
            )
            .unwrap();

            while ctx.execute_pending_job() {}

            // Every failure is reported, in order, and handed to the handler
            // only: nothing is retained or counted.
            assert_eq!(*seen.lock().unwrap(), vec![Some(1), Some(2)]);
            assert!(ctx.take_pending_job_error().is_none());
            assert_eq!(ctx.take_pending_job_error_overflow(), 0);
            assert!(!ctx.has_exception());
        })
    }

    #[test]
    fn reentrant_job_error_handler_reports_every_failure_in_order() {
        use crate::{Context, Runtime};
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let runtime = Runtime::new().unwrap();
        {
            let seen = seen.clone();
            runtime.set_job_error_handler(Some(Box::new(move |ctx, error| {
                seen.lock().unwrap().push(error.as_int());
                // Re-enter the drain from inside the handler.
                while ctx.execute_pending_job() {}
            })));
        }
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });
                 queueMicrotask(() => { throw 3; });",
            )
            .unwrap();

            while ctx.execute_pending_job() {}

            assert_eq!(*seen.lock().unwrap(), vec![Some(1), Some(2), Some(3)]);
            assert!(ctx.take_pending_job_error().is_none());
            assert_eq!(ctx.take_pending_job_error_overflow(), 0);
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn execute_pending_job_retains_valueless_failure_as_undefined() {
        use crate::{Context, Error, Function, Result, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            // A Rust callback reporting an exception without throwing one.
            ctx.globals()
                .set(
                    "failSilently",
                    Function::new(ctx.clone(), || -> Result<()> { Err(Error::Exception) }),
                )
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(failSilently);").unwrap();

            while ctx.execute_pending_job() {}

            assert!(ctx.take_pending_job_error().unwrap().is_undefined());
            assert!(!ctx.has_exception());
        })
    }

    #[test]
    fn execute_pending_job_retains_rethrown_consumed_sentinel_as_undefined() {
        use crate::{Context, Function, Runtime};
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set(
                    "rustPanic",
                    Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
                )
                .unwrap();
            // JS captures the synthetic exception of a panic (the trailing
            // `undefined` keeps it from becoming the script's result)...
            ctx.eval::<(), _>(
                "try { rustPanic(); } catch (e) { globalThis.sentinel = e; } undefined;",
            )
            .unwrap();
            // ...whose payload a later host error crossing consumes.
            let payload = catch_unwind(AssertUnwindSafe(|| {
                let _ = ctx.eval::<(), _>("throw 1");
            }))
            .expect_err("a host error should resume the stashed panic");
            assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "rust boom");

            // Rethrowing the captured value in a job fails it with a sentinel
            // and an empty stash: it is retained as `undefined`.
            ctx.eval::<(), _>("queueMicrotask(() => { throw globalThis.sentinel; });")
                .unwrap();
            while ctx.execute_pending_job() {}

            assert!(ctx.take_pending_job_error().unwrap().is_undefined());
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn execute_pending_job_counts_dropped_failures() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>(
                "queueMicrotask(() => { throw 1; });
                 queueMicrotask(() => { throw 2; });
                 queueMicrotask(() => { throw 3; });",
            )
            .unwrap();

            while ctx.execute_pending_job() {}

            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(1));
            // The two later failures could not be retained.
            assert_eq!(ctx.take_pending_job_error_overflow(), 2);
            // Both takes reset.
            assert!(ctx.take_pending_job_error().is_none());
            assert_eq!(ctx.take_pending_job_error_overflow(), 0);
        })
    }

    #[test]
    fn job_error_handler_panic_propagates_out_of_drain() {
        use crate::{Context, Error, Runtime};
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let runtime = Runtime::new().unwrap();
        runtime.set_job_error_handler(Some(Box::new(|_, _| panic!("handler boom"))));
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
                .unwrap();

            // The handler's panic unwinds out of the drain with its payload.
            let payload = catch_unwind(AssertUnwindSafe(|| while ctx.execute_pending_job() {}))
                .expect_err("the handler panic should propagate out of the drain");
            assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "handler boom");

            // The failure went to the handler only, and the failed job's
            // exception was not left pending.
            assert!(ctx.take_pending_job_error().is_none());
            assert!(!ctx.has_exception());

            // The panic was not stashed: a later host error is a plain
            // exception, and the runtime remains usable.
            assert!(matches!(
                ctx.eval::<(), _>("throw 2"),
                Err(Error::Exception)
            ));
            assert_eq!(ctx.catch().as_int(), Some(2));
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn execute_pending_job_drain_skips_caught_panic() {
        use crate::{Context, Function, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.globals()
                .set(
                    "rustPanic",
                    Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
                )
                .unwrap();

            // Job A panics, but JS catches the synthetic exception: job A
            // does not fail, so the drain must not resume the panic. (The
            // payload stays stashed for the next host error crossing.)
            ctx.eval::<(), _>("queueMicrotask(() => { try { rustPanic(); } catch (e) {} });")
                .unwrap();
            // Job B fails unrelatedly.
            ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
                .unwrap();

            while ctx.execute_pending_job() {}

            // Only job B's failure is retained.
            assert_eq!(ctx.take_pending_job_error().unwrap().as_int(), Some(1));

            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn execute_pending_job_resumes_pending_panic() {
        use crate::{Context, Function, Runtime};
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            // A panicking Rust callback inside a job: the payload is stashed
            // and the job fails with a synthetic exception.
            ctx.globals()
                .set(
                    "rustPanic",
                    Function::new(ctx.clone(), || -> i32 { panic!("rust boom") }),
                )
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(rustPanic);").unwrap();

            // The drain itself resumes the stashed panic, with its original
            // payload.
            let payload = catch_unwind(AssertUnwindSafe(|| while ctx.execute_pending_job() {}))
                .expect_err("the drain should have resumed the stashed panic");
            assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "rust boom");

            // A second drain is a no-op; the context remains usable.
            while ctx.execute_pending_job() {}
            let sum: i32 = ctx.eval("1 + 1").unwrap();
            assert_eq!(sum, 2);
        })
    }

    #[test]
    fn eval() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: String = ctx
                .eval(
                    r#"
                    function test() {
                        var foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                )
                .unwrap();

            assert_eq!("bar".to_string(), res);
        })
    }

    #[test]
    fn eval_minimal_test() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: i32 = ctx.eval(" 1 + 1 ").unwrap();
            assert_eq!(2, res);
        })
    }

    #[test]
    #[should_panic(expected = "foo is not defined")]
    fn eval_with_sloppy_code() {
        use crate::{CatchResultExt, Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let _: String = ctx
                .eval(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                )
                .catch(&ctx)
                .unwrap();
        })
    }

    #[test]
    fn eval_with_options_no_strict_sloppy_code() {
        use crate::{context::EvalOptions, Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: String = ctx
                .eval_with_options(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                    EvalOptions {
                        strict: false,
                        ..Default::default()
                    },
                )
                .unwrap();

            assert_eq!("bar".to_string(), res);
        })
    }

    #[test]
    #[should_panic(expected = "foo is not defined")]
    fn eval_with_options_strict_sloppy_code() {
        use crate::{context::EvalOptions, CatchResultExt, Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let _: String = ctx
                .eval_with_options(
                    r#"
                    function test() {
                        foo = "bar";
                        return foo;
                    }

                    test()
                "#,
                    EvalOptions {
                        strict: true,
                        ..Default::default()
                    },
                )
                .catch(&ctx)
                .unwrap();
        })
    }

    #[test]
    fn json_parse() {
        use crate::{Array, Context, Object, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let v = ctx
                .json_parse(r#"{ "a": { "b": 1, "c": true }, "d": [0,"foo"] }"#)
                .unwrap();
            let obj = v.into_object().unwrap();
            let inner_obj: Object = obj.get("a").unwrap();
            assert_eq!(inner_obj.get::<_, i32>("b").unwrap(), 1);
            assert!(inner_obj.get::<_, bool>("c").unwrap());
            let inner_array: Array = obj.get("d").unwrap();
            assert_eq!(inner_array.get::<i32>(0).unwrap(), 0);
            assert_eq!(inner_array.get::<String>(1).unwrap(), "foo".to_string());
        })
    }

    #[test]
    fn json_stringify() {
        use crate::{Array, Context, Object, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let obj_inner = Object::new(ctx.clone()).unwrap();
            obj_inner.set("b", 1).unwrap();
            obj_inner.set("c", true).unwrap();

            let array_inner = Array::new(ctx.clone()).unwrap();
            array_inner.set(0, 0).unwrap();
            array_inner.set(1, "foo").unwrap();

            let obj = Object::new(ctx.clone()).unwrap();
            obj.set("a", obj_inner).unwrap();
            obj.set("d", array_inner).unwrap();

            let str = ctx
                .json_stringify(obj)
                .unwrap()
                .unwrap()
                .to_string()
                .unwrap();

            assert_eq!(str, r#"{"a":{"b":1,"c":true},"d":[0,"foo"]}"#);
        })
    }

    #[test]
    fn userdata() {
        use crate::{Context, Function, Runtime};

        pub struct MyUserData<'js> {
            base: Function<'js>,
        }

        unsafe impl<'js> JsLifetime<'js> for MyUserData<'js> {
            type Changed<'to> = MyUserData<'to>;
        }

        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();

        ctx.with(|ctx| {
            let func = ctx.eval("() => 42").catch(&ctx).unwrap();
            ctx.store_userdata(MyUserData { base: func }).unwrap();
        });

        ctx.with(|ctx| {
            let userdata = ctx.userdata::<MyUserData>().unwrap();

            assert!(ctx.remove_userdata::<MyUserData>().is_err());

            let r: usize = userdata.base.call(()).unwrap();
            assert_eq!(r, 42)
        });

        ctx.with(|ctx| {
            ctx.remove_userdata::<MyUserData>().unwrap().unwrap();
        })
    }

    #[test]
    fn eval_null_byte() {
        use crate::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            let res: String = ctx.eval("\"hi\0there\"").unwrap();
            assert_eq!(res, "hi\0there");
        })
    }
}
