/// Allocates a static cell and returns a mutable reference with `'static` lifetime.
/// Note: Will panic if evaluated more than once.
#[macro_export]
macro_rules! mk_static {
  ($t:ty,$val:expr) => {{
    static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
    #[deny(unused_attributes)]
    let x = STATIC_CELL.uninit().write($val);
    x
  }};
}

/// Like [`mk_static`], but reuses the same static storage on every call instead
/// of panicking on the second one.
///
/// # Safety
/// Sound only when callers are fully serialized — the previous holder of the
/// `&'static mut` must be dropped before this macro is evaluated again.
/// Typically used for network stacks (Wi-Fi, LTE PPP) that tear down completely
/// between transmissions.
#[macro_export]
macro_rules! mk_static_reusable {
  ($t:ty,$val:expr) => {{
    static mut CELL: core::mem::MaybeUninit<$t> = core::mem::MaybeUninit::uninit();
    unsafe { (*core::ptr::addr_of_mut!(CELL)).write($val) }
  }};
}
