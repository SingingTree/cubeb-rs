use libc::c_void;

pub trait NullCheck {
    fn not_null(&self) -> bool;
    fn is_null(&self) -> bool;
}

pub fn cast<'a, T>(ptr: *mut ::libc::c_void) -> &'a T {
    assert!(!ptr.is_null());
    unsafe { &*(ptr as *mut T) }
}

pub fn cast_mut<'a, T>(ptr: *mut ::libc::c_void) -> &'a mut T {
    assert!(!ptr.is_null());
    unsafe { &mut *(ptr as *mut T) }
}

pub fn cast_void_ptr<T>(t: &T) -> *mut c_void {
    t as *const _ as *mut c_void
}
