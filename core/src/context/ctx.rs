#[cfg(feature = "futures")]
use core::future::Future;

use alloc::{boxed::Box, ffi::CString, vec::Vec};
use core::{
    any::Any,
    ffi::{c_char, CStr},
    mem::{self, MaybeUninit},
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

/// How an internal job drain handles uncatchable errors.
pub(crate) enum JobDrain {
    /// Clear the pending exception.
    Sync,
    /// Also record uncatchable errors for promise futures.
    #[cfg(feature = "futures")]
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
    /// previous take. A failure without an exception value is retained as
    /// `undefined`.
    ///
    /// Failures handled by [`crate::Runtime::set_job_error_handler`] or
    /// `AsyncRuntime::set_job_error_handler` are not retained.
    ///
    /// The retained error is runtime-wide: it belongs to the context the job
    /// ran in, which may differ from this context. It keeps that context and
    /// its object graph alive until taken or the runtime is dropped.
    ///
    /// Failures surfaced by the runtime's own `execute_pending_job` methods
    /// are returned to the caller instead. JS failures during runtime teardown
    /// are discarded without being retained, counted, or reported to the job
    /// error handler; Rust panics from failing callbacks still propagate, as
    /// from any destructor.
    pub fn take_pending_job_error(&self) -> Option<Value<'js>> {
        unsafe { self.get_opaque() }.take_job_error()
    }

    /// Takes the number of job failures that could not be retained because
    /// an earlier failure was still held for [`Ctx::take_pending_job_error`].
    ///
    /// The count is runtime-wide, saturates at `u64::MAX`, and resets to 0.
    ///
    /// Failures handed to the job error handler are not counted.
    pub fn take_pending_job_error_overflow(&self) -> u64 {
        unsafe { self.get_opaque() }.take_job_error_overflow()
    }

    /// Takes the uncatchable error (e.g. an interrupt) that an async job
    /// drain recorded for promise futures in this context, if no promise
    /// future has observed it yet.
    ///
    /// Until observed or taken, the recorded error fails the next unsettled
    /// promise future polled in this context, which may await unrelated work
    /// (see [`Promise::into_future`]). Once the host has handled the failure,
    /// e.g. through the job error handler or
    /// [`Ctx::take_pending_job_error`], and abandoned the affected work, take
    /// the recorded error so it cannot fail a later operation.
    ///
    /// The recorded error belongs to the context the interrupted job ran in,
    /// so take it on that context: the job error handler runs in it, and the
    /// error returned by [`Ctx::take_pending_job_error`] carries it
    /// ([`Value::ctx`]). It keeps that context and its object graph alive
    /// until observed, taken, or the runtime is dropped.
    #[cfg(feature = "futures")]
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn take_interrupted_job_error(&self) -> Option<Value<'js>> {
        unsafe { self.get_opaque() }.take_interrupted_job(self)
    }

    /// Handle a failed job in its own context, which may differ from the
    /// context the drain was started on.
    ///
    /// Resume a stashed Rust panic only if the job failed with a synthetic
    /// panic exception. Otherwise, report the failure to the handler or
    /// retain it for [`Ctx::take_pending_job_error`].
    ///
    /// JS can rethrow a captured panic sentinel, which resumes the currently
    /// stashed payload. A caught sentinel otherwise leaves the payload for a
    /// later host error, as in `handle_exception`.
    ///
    /// Async drains first record uncatchable errors in the runtime's opaque
    /// state, outside the engine's volatile pending-exception slot, and wake
    /// spawned futures, so a `PromiseFuture` observes an interrupted job even
    /// if its promise never settles. Recording before the handler runs keeps
    /// a handler panic from skipping it. Handler panics propagate without
    /// being stashed.
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
                // The job has returned, so the unwind does not cross QuickJS.
                crate::util::resume_unwind(payload);
            }
        }
        // Missing exceptions and rethrown panic sentinels with a consumed
        // payload are not usable JS values. Neither tag is reference counted.
        let error = if tag == qjs::JS_TAG_UNINITIALIZED || tag == qjs::JS_TAG_EXCEPTION {
            Value::new_undefined(self.clone())
        } else {
            unsafe { Value::from_js_value(self.clone(), raw) }
        };
        match drain {
            JobDrain::Sync => {}
            #[cfg(feature = "futures")]
            JobDrain::Async => {
                if error.is_uncatchable_error() {
                    opaque.record_interrupted_job(error.clone());
                }
            }
        }
        if !opaque.run_job_error_handler(self.clone(), error.clone()) {
            opaque.retain_job_error(error);
        }
    }

    /// Handle a failed job given the raw context pointer reported by
    /// `JS_ExecutePendingJob`, via [`Ctx::handle_failed_job`].
    ///
    /// # Safety
    /// The runtime owning `ctx` must be locked for the duration of the call,
    /// and `ctx` must be the context reported by a failed
    /// `JS_ExecutePendingJob`.
    pub(crate) unsafe fn handle_failed_job_ptr(ctx: *mut qjs::JSContext, drain: JobDrain) {
        let ctx =
            NonNull::new(ctx).expect("executing pending job returned a null context on error");
        // Safety: guaranteed by the caller. `from_raw` dups the context and
        // the temporary `Ctx` frees it on drop.
        unsafe { Ctx::from_raw(ctx) }.handle_failed_job(drain);
    }

    /// Take a failed job's stashed Rust panic if the pending exception is its
    /// sentinel, otherwise leave the exception pending. Returned rather than
    /// resumed so the callers can release the runtime lock first.
    pub(crate) fn take_job_panic(&self) -> Option<Box<dyn Any + Send + 'static>> {
        let raw = unsafe { qjs::JS_GetException(self.as_ptr()) };
        let tag = unsafe { qjs::JS_VALUE_GET_NORM_TAG(raw) };
        if tag == qjs::JS_TAG_UNINITIALIZED {
            return None;
        }
        if tag == qjs::JS_TAG_EXCEPTION {
            if let Some(payload) = unsafe { self.get_opaque() }.take_panic() {
                return Some(payload);
            }
        }
        // Leave the exception pending for the caller.
        unsafe { qjs::JS_Throw(self.as_ptr(), raw) };
        None
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
    /// Returns whether a job was executed, or false if no job was pending.
    ///
    /// The job may belong to any context of the runtime. If it fails, its
    /// exception is taken out of the engine and handed to the job error
    /// handler if one is set, or otherwise retained for
    /// [`Ctx::take_pending_job_error`]. The failed job's exception is cleared.
    /// Rust panics from a failing callback or the job error handler propagate
    /// out of this function.
    pub fn execute_pending_job(&self) -> bool {
        let mut ptr = MaybeUninit::<*mut qjs::JSContext>::uninit();
        let rt = unsafe { qjs::JS_GetRuntime(self.ctx.as_ptr()) };
        let res = unsafe { qjs::JS_ExecutePendingJob(rt, ptr.as_mut_ptr()) };
        if res < 0 {
            // QuickJS reports the failed job's context, which need not be `self`.
            // Safety: the runtime is locked through `self`, and `ptr` was
            // initialized by the failed `JS_ExecutePendingJob`.
            unsafe { Self::handle_failed_job_ptr(ptr.assume_init(), JobDrain::Sync) };
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
            // The trailing `undefined` keeps the panic sentinel out of the eval result.
            ctx.eval::<(), _>(
                "try { rustPanic(); } catch (e) { globalThis.sentinel = e; } undefined;",
            )
            .unwrap();
            // Consume the panic payload before rethrowing the captured sentinel.
            let payload = catch_unwind(AssertUnwindSafe(|| {
                let _ = ctx.eval::<(), _>("throw 1");
            }))
            .expect_err("a host error should resume the stashed panic");
            assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "rust boom");

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

            let payload = catch_unwind(AssertUnwindSafe(|| while ctx.execute_pending_job() {}))
                .expect_err("the handler panic should propagate out of the drain");
            assert_eq!(*payload.downcast_ref::<&str>().unwrap(), "handler boom");

            assert!(ctx.take_pending_job_error().is_none());
            assert!(!ctx.has_exception());

            // The handler panic must not be stashed for a later eval.
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

            // Catching the panic sentinel leaves its payload stashed, but the
            // unrelated job failure below must not resume it.
            ctx.eval::<(), _>("queueMicrotask(() => { try { rustPanic(); } catch (e) {} });")
                .unwrap();
            ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
                .unwrap();

            while ctx.execute_pending_job() {}

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
    fn execute_pending_job_handles_failure_in_job_context() {
        use crate::{Context, Runtime};
        use std::sync::{Arc, Mutex};

        let seen = Arc::new(Mutex::new(Vec::new()));
        let runtime = Runtime::new().unwrap();
        {
            let seen = seen.clone();
            runtime.set_job_error_handler(Some(Box::new(move |ctx, error| {
                let tag: String = ctx.globals().get("__ctxTag").unwrap();
                seen.lock().unwrap().push((tag, error.as_int()));
            })));
        }
        let ctx_a = Context::full(&runtime).unwrap();
        let ctx_b = Context::full(&runtime).unwrap();
        ctx_a.with(|ctx| ctx.eval::<(), _>("globalThis.__ctxTag = 'A';").unwrap());
        ctx_b.with(|ctx| ctx.eval::<(), _>("globalThis.__ctxTag = 'B';").unwrap());

        // The job is queued in B but drained from A.
        ctx_b.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
                .unwrap()
        });
        ctx_a.with(|ctx| {
            while ctx.execute_pending_job() {}
            assert!(!ctx.has_exception());
        });
        assert_eq!(*seen.lock().unwrap(), vec![("B".to_string(), Some(1))]);

        // Without a handler, the retained value belongs to B.
        runtime.set_job_error_handler(None);
        ctx_b.with(|ctx| {
            ctx.eval::<(), _>("queueMicrotask(() => { throw 1; });")
                .unwrap()
        });
        ctx_a.with(|ctx| {
            while ctx.execute_pending_job() {}
            assert!(!ctx.has_exception());

            let error = ctx.take_pending_job_error().unwrap();
            assert_eq!(error.as_int(), Some(1));
            let tag: String = error.ctx().globals().get("__ctxTag").unwrap();
            assert_eq!(tag, "B");
        });
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn execute_pending_job_clears_interrupted_job_error() {
        use crate::{Context, Runtime};
        use std::sync::{
            atomic::{AtomicBool, Ordering::SeqCst},
            Arc,
        };

        let armed = Arc::new(AtomicBool::new(false));
        let runtime = Runtime::new().unwrap();
        {
            let armed = armed.clone();
            // Interrupt once, then disarm so no later code is interrupted.
            runtime.set_interrupt_handler(Some(Box::new(move || armed.swap(false, SeqCst))));
        }
        let ctx = Context::full(&runtime).unwrap();
        ctx.with(|ctx| {
            ctx.eval::<(), _>("Promise.resolve().then(() => { for (;;) {} });")
                .unwrap();

            // Arm only once the job is queued, so only the job is interrupted.
            armed.store(true, SeqCst);
            while ctx.execute_pending_job() {}
            assert!(!armed.load(SeqCst));

            // The sync drain retains the uncatchable error but does not
            // leave it pending.
            assert!(ctx.take_pending_job_error().unwrap().is_uncatchable_error());
            assert!(!ctx.has_exception());

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
