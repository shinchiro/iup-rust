///! High-level callback binding between C IUP and Rust.
use iup_sys::CallbackReturn;
use iup_sys;
use Ihandle;

/// Obtains the static C string which relates to the fat box from the IUP attribute `$cb_name`.
macro_rules! fbox_c_str {
    ($cb_name:expr) => {
        // It's important to use the prefix '_IUP*', it's reserved by IUP for internal use and bindings.
        // So we use '_IUPRUST_*' prefix to refer to data reserved for the Rust binding.
        str_to_c_str!(concat!("_IUPRUST_FBOX_", $cb_name))
    }
}

/// Implements a callback binding.
///
/// After this macro is executed the following functions gets implemented:
///
///    + `pub fn $set_func<F>(&mut Ihandle, Option<F>) -> Option<Box<Fn>>` to set a callback
///      for the specified IUP handle.
///    + `extern fn $listener(*mut iup_sys::Ihandle, ...) -> iup_sys::CallbackReturn` to internally
///      listen to the event generated by set above and then propagate the event to the Rusty callback.
///    + `fn $drop_func(*mut iup_sys::Ihandle) -> Option<Box<Fn>>` to free any Rust resource
///      related to the event. This **must** be called during our `::callback::on_destroy`.
///      
/// The generated functions works on the IUP callback named with the `name` binding.
///
/// The Rust signature of the callback is specified by the `F` constraint in the `$set_func`.
///
/// The `$listen` function should be manually implemented to call the Boxed `F`.
///
/// # Example
///
/// ```
/// use iup_sys::CallbackReturn;
/// use iup_sys;
/// use Ihandle;
///
/// impl_callback! {
///     let name = "ACTION";
/// 
///     pub fn set_action<F: Fn(Ihandle) -> CallbackReturn>(ih: &mut Ihandle, callback: Option<F>) -> Option<Box<_>>;
///     fn drop_action(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;
///     
///     extern fn listener_action(f: &Box<_>, ih: *mut iup_sys::Ihandle) -> CallbackReturn {
///         f(Ihandle::from_ptr(ih))
///     }
/// }
///
/// // don't forget to call the declared `drop_action` in `::callback::on_destroy`!!!!!!!!!!
///
/// ```
///
macro_rules! impl_callback {

    // The following is used when $listener has no additional arguments except for the handler.
    (
        let name = $cb_name:expr;

        pub fn $set_func:ident<F: Fn($($fn_arg_ty:ty),*) -> $fn_ret_ty:ty>(ih: &mut Ihandle, 
            callback: Option<F>) -> Option<Box<_>>;

        fn $drop_func:ident(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;

        extern fn $listener:ident($ls_fbox_arg:ident: &Box<_>,
            $ls_ih_arg:ident: *mut iup_sys::Ihandle) -> CallbackReturn $call:expr
    ) => {
        impl_callback! {
            let name = $cb_name;
            pub fn $set_func<F: Fn($($fn_arg_ty),*) -> $fn_ret_ty>(ih: &mut Ihandle,
                callback: Option<F>) -> Option<Box<_>>;
            fn $drop_func(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;
            extern fn $listener($ls_fbox_arg: &Box<_>, $ls_ih_arg: *mut iup_sys::Ihandle, )
                    -> CallbackReturn {
                $call
            }
        }
    };
    // This is used when $listener has the handler plus additional arguments.
    (
        let name = $cb_name:expr;

        pub fn $set_func:ident<F: Fn($($fn_arg_ty:ty),*) -> $fn_ret_ty:ty>(ih: &mut Ihandle,
            callback: Option<F>) -> Option<Box<_>>;

        fn $drop_func:ident(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;

        extern fn $listener:ident($ls_fbox_arg:ident: &Box<_>,
            $ls_ih_arg:ident: *mut iup_sys::Ihandle, $($ls_arg:ident: $ls_arg_ty:ty),*)
                -> CallbackReturn $call:expr
    ) => {

        /// C compatible listener for an IUP callback.
        extern fn $listener($ls_ih_arg: *mut iup_sys::Ihandle, $($ls_arg: $ls_arg_ty),*)
                -> CallbackReturn {
            let fbox_ptr__ = unsafe { iup_sys::IupGetAttribute($ls_ih_arg, fbox_c_str!($cb_name))
                                             as *mut Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty> };
            assert!(fbox_ptr__.is_null() == false);
            let $ls_fbox_arg: &Box<_> = unsafe { &(*(fbox_ptr__)) };
            { $call }
        }

        /// Sets the Rust listener for an IUP callback.
        ///
        /// Sets the listener if `cb` is `Some` or removes it when `None`.
        ///
        /// The function returns the previous Rust callback listener.
        pub fn $set_func<F: Fn($($fn_arg_ty),*) -> $fn_ret_ty>(ih: &mut Ihandle, cb: Option<F>)
             -> Option<Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty>> {

            use std::mem::transmute;

            // TODO remove this in favour to std::boxed::into_raw when it gets stable.
            unsafe fn box_into_raw<T : ?Sized>(b: Box<T>) -> *mut T {
                transmute(b)
            }

            let old_cb = $drop_func(ih.ptr);

            if cb.is_some() {
                unsafe {
                    let fb: Box<Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty>> = Box::new(Box::new(cb.unwrap()));
                    iup_sys::IupSetAttribute(ih.ptr, fbox_c_str!($cb_name), box_into_raw(fb) as *const _);
                    iup_sys::IupSetCallback(ih.ptr, str_to_c_str!($cb_name), transmute($listener));
                }
            }

            old_cb
        }

        /// Frees up the allocated content by the Rust binding to support Rust closures.
        ///
        /// The function returns the previous Rust callback listener.
        fn $drop_func(ih: *mut iup_sys::Ihandle)
                            -> Option<Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty>> {
            unsafe {
                use std::mem::transmute;
                use std::ptr;

                let capsule_box = iup_sys::IupGetAttribute(ih, fbox_c_str!($cb_name))
                                            as *mut Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty>;
                if capsule_box.is_null() {
                    None 
                } else {

                    // TODO when Box::from_raw gets stable use it instead of transmute here.
                    let inner_box: Box<Box<Fn($($fn_arg_ty),*) -> $fn_ret_ty>> = transmute(capsule_box);

                    iup_sys::IupSetAttribute(ih, fbox_c_str!($cb_name), ptr::null());
                    iup_sys::IupSetCallback(ih, str_to_c_str!($cb_name), transmute(ptr::null::<u8>()));

                    Some(*inner_box)
                    // inner_box itself gets freed now
                }
            }
        }
    }
}


impl_callback! {
    let name = "ACTION";

    pub fn set_action<F: Fn(Ihandle) -> CallbackReturn>(ih: &mut Ihandle, callback: Option<F>) -> Option<Box<_>>;
    fn drop_action(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;

    extern fn listener_action(f: &Box<_>, ih: *mut iup_sys::Ihandle) -> CallbackReturn {
        f(Ihandle::from_ptr(ih))
    }
}

impl_callback! {
    let name = "DESTROY_CB";

    pub fn set_destroy_cb<F: Fn(Ihandle) -> ()>(ih: &mut Ihandle, callback: Option<F>) -> Option<Box<_>>;
    fn drop_destroy_cb(ih: *mut iup_sys::Ihandle) -> Option<Box<_>>;

    extern fn listener_destroy_cb(f: &Box<_>, ih: *mut iup_sys::Ihandle) -> CallbackReturn {
        f(Ihandle::from_ptr(ih));
        CallbackReturn::Default
    }
}


/// Frees up IUP handle callback resources.
///
/// This gets called during the destroy phase of an IUP handle to free up callback resources.
pub fn on_destroy(ih: *mut iup_sys::Ihandle) {
    
    drop_action(ih);

    // Note: drop_destroy_cb **MUST** be the last drop.
    // IUP calls LDESTROY_CB (binding free) before calling DESTROY_CB (user free)
    // but we need the binding data to properly call DESTROY_CB, thus call DESTROY_CB here:
    drop_destroy_cb(ih).and_then(|cb| {
        cb(Ihandle::from_ptr(ih));
        Some(cb)
    });
}
