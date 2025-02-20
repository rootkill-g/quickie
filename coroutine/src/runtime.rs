use std::{
    any::{self, Any},
    cell::Cell,
    mem::MaybeUninit,
    ptr::{self, null_mut},
};

use crate::register_context::RegisterContext;

thread_local! {
    /// Each thread has it's own generator context stack
    static ROOT_CONTEXT_P: Cell<*mut Context> = const { Cell::new(ptr::null_mut()) };
}

/// Generator Context
#[repr(C)]
#[repr(align(128))]
pub struct Context {
    /// Generator regs context
    pub regs: RegisterContext,

    /// Child context
    pub(crate) child: *mut Context,

    /// Parent context
    pub parent: *mut Context,

    /// Passed in parameter in send
    pub para: MaybeUninit<*mut dyn Any>,

    /// Buffer for the return value
    pub ret: MaybeUninit<*mut dyn Any>,

    /// Track coroutine ref. Yield will -1 and Send will +1
    pub _ref: usize,

    /// Context local storage
    pub local_data: *mut u8,

    /// Propagate panic
    pub err: Option<Box<dyn Any + Send>>,

    /// Cached stack guard for fast path
    pub stack_guard: (usize, usize),
}

impl Context {
    /// New instance of Generator Context
    pub fn new() -> Context {
        Context {
            regs: RegisterContext::empty(),
            para: MaybeUninit::zeroed(),
            ret: MaybeUninit::zeroed(),
            _ref: 1, // Non-zero means Not running
            err: None,
            child: null_mut(),
            parent: null_mut(),
            local_data: null_mut(),
            stack_guard: (0, 0),
        }
    }

    /// Check if it is generator's context
    #[inline]
    pub fn is_generator(&self) -> bool {
        self.parent != self as *const _ as *mut _
    }

    /// Get current generator send parameter
    #[inline]
    pub fn get_para<T>(&mut self) -> Option<T>
    where
        T: Any,
    {
        let para = unsafe {
            let para_ptr = *self.para.as_mut_ptr();

            assert!(!para_ptr.is_null());

            &mut *para_ptr
        };

        match para.downcast_mut::<Option<T>>() {
            Some(v) => v.take(),
            None => type_error::<T>("Get yield type mismatch error detected"),
        }
    }

    /// Get coroutine parameter
    pub fn coroutine_get_para<T>(&mut self) -> Option<T> {
        let para = unsafe {
            let para_ptr = *self.para.as_mut_ptr();

            debug_assert!(!para_ptr.is_null());

            &mut *(para_ptr as *mut Option<T>)
        };

        para.take()
    }

    /// Set coroutine send para
    pub fn coroutine_set_para<T>(&mut self, data: T) {
        let para = unsafe {
            let para_ptr = *self.para.as_mut_ptr();

            debug_assert!(!para_ptr.is_null());

            &mut *(para_ptr as *mut Option<T>)
        };

        *para = Some(data);
    }

    /// Set current generator return value
    pub fn set_ret<T>(&mut self, v: T)
    where
        T: Any,
    {
        let ret = unsafe {
            let ret_ptr = *self.ret.as_mut_ptr();

            debug_assert!(!ret_ptr.is_null());

            &mut *ret_ptr
        };

        match ret.downcast_mut::<Option<T>>() {
            Some(r) => *r = Some(v),
            None => type_error::<T>("Yield type mismatch error detected"),
        }
    }

    /// Set coroutine return value
    /// Without checking the data type for coroutine performance
    #[inline]
    pub fn coroutine_set_ret<T>(&mut self, v: T) {
        let ret = unsafe {
            let ret_ptr = *self.ret.as_mut_ptr();

            debug_assert!(!ret_ptr.is_null());

            &mut *(ret_ptr as *mut Option<T>)
        };

        *ret = Some(v)
    }
}

/// Coroutine managing environment
pub struct ContextStack {
    pub(crate) root: *mut Context,
}

impl ContextStack {
    #[cold]
    fn init_root() -> *mut Context {
        let root = {
            let mut root = Box::new(Context::new());
            let p = &mut *root as *mut _;

            root.parent = p;

            Box::leak(root)
        };

        ROOT_CONTEXT_P.set(root);

        root
    }

    /// Get the current context stack
    pub fn current() -> ContextStack {
        let mut root = ROOT_CONTEXT_P.get();

        if root.is_null() {
            root = ContextStack::init_root();
        }

        ContextStack { root }
    }

    /// Get the top context
    #[inline]
    pub fn top(&self) -> &'static mut Context {
        let root = unsafe { &mut *self.root };

        unsafe { &mut *root.parent }
    }

    /// Get the coroutine context
    #[inline]
    pub fn coroutine_ctx(&self) -> Option<&'static mut Context> {
        let root = unsafe { &mut *self.root };

        // Search from top
        let mut ctx = unsafe { &mut *root.parent };

        while ctx as *const _ != root as *const _ {
            if !ctx.local_data.is_null() {
                return Some(ctx);
            }

            ctx = unsafe { &mut *ctx.parent };
        }

        // Not find any coroutine
        None
    }
}

/// Check the current context if it's generator
#[inline]
pub fn is_generator() -> bool {
    let env = ContextStack::current();
    let root = unsafe { &mut *env.root };

    !root.child.is_null()
}

#[inline]
#[cold]
fn type_error<A>(msg: &str) -> ! {
    log::error!("{}, expected type: {}", msg, any::type_name::<A>());

    std::panic::panic_any(crate::error::Error::TypeErr);
}

/// Get the current context local data
/// Only coroutine support local data
pub(crate) fn get_local_data() -> *mut u8 {
    let env = ContextStack::current();
    let root = unsafe { &mut *env.root };

    // Search from top
    let mut ctx = unsafe { &mut *root.parent };

    while ctx as *const _ != root as *const _ {
        if !ctx.local_data.is_null() {
            return ctx.local_data;
        }

        ctx = unsafe { &mut *ctx.parent };
    }

    ptr::null_mut()
}
