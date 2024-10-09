use rquickjs_sys::JSRuntime;

use crate::qjs;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock, RwLock,
    },
};

/// The type of identifier of class
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "classes")))]
pub struct ClassId {
    type_id: AtomicUsize,
}

#[derive(Eq, Hash, PartialEq)]
struct ClassIdKey(*mut JSRuntime, usize);

unsafe impl Sync for ClassIdKey {}
unsafe impl Send for ClassIdKey {}

static CLASS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
static CLASS_ID_MAP: OnceLock<RwLock<HashMap<ClassIdKey, qjs::JSClassID>>> = OnceLock::new();

impl ClassId {
    /// Create a new class id.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            type_id: AtomicUsize::new(0),
        }
    }

    /// Get the class Id.
    /// Will initialize itself if it has not done so.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn get(&self, rt: *mut JSRuntime) -> qjs::JSClassID {
        let type_id = self.init_type_id();
        let key = ClassIdKey(rt, type_id);
        let class_id_lock = CLASS_ID_MAP.get_or_init(|| RwLock::new(HashMap::new()));
        if let Some(class_id) = class_id_lock.read().unwrap().get(&key) {
            return *class_id;
        }

        let mut read_lock = class_id_lock.write().unwrap();
        let mut id = 0;
        unsafe { qjs::JS_NewClassID(rt, &mut id) };

        read_lock.insert(key, id);

        id
    }

    /// Initialize the class ID.
    /// Can be called multiple times but will only be initialized once.
    fn init_type_id(&self) -> usize {
        let id: usize = self.type_id.load(Ordering::Relaxed);
        if id == 0 {
            let new_id = CLASS_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
            self.type_id.store(new_id, Ordering::Relaxed);
            return new_id;
        }
        id
    }
}
