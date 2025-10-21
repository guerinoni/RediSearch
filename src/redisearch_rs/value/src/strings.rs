use std::{
    alloc::{self, Layout},
    fmt,
    os::raw::c_char,
    ptr::{NonNull, copy_nonoverlapping},
    sync::Arc,
};

use redis_module::{Context, RedisModule_Alloc, RedisModule_Free, RedisModuleString, RedisString};

#[repr(C)]
pub struct OwnedRmAllocString {
    str: NonNull<c_char>,
    len: u32,
}

impl OwnedRmAllocString {
    /// # Safety
    /// - `str` must not be aliased
    /// - `str` must have been allocated with `rm_alloc`
    pub const unsafe fn new_unchecked(str: NonNull<c_char>, len: u32) -> Self {
        Self { str, len }
    }

    pub unsafe fn copy_from_string(str: *const c_char, len: u32) -> Self {
        let length = len as usize;
        let rm_alloc = unsafe { RedisModule_Alloc.expect("Redis allocator not available") };
        let buf = unsafe { rm_alloc(length + 1) } as *mut c_char;

        let buf = NonNull::new(buf).expect("Failed to allocate memory");

        unsafe { buf.add(length).write(0) };
        unsafe { copy_nonoverlapping(str, buf.as_ptr(), length) };

        Self { str: buf, len }
    }
}

impl Clone for OwnedRmAllocString {
    fn clone(&self) -> Self {
        todo!("malloc a new str, use that pointer")
    }
}

impl fmt::Debug for OwnedRmAllocString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = unsafe {
            std::slice::from_raw_parts(self.str.as_ptr() as *const u8, self.len as usize)
        };
        String::from_utf8_lossy(s).fmt(f)
    }
}

impl fmt::Display for OwnedRmAllocString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl Drop for OwnedRmAllocString {
    fn drop(&mut self) {
        let rm_free = unsafe { RedisModule_Free.expect("Redis allocator not available") };
        unsafe { rm_free(self.str.as_ptr() as *mut _) };
    }
}

unsafe impl Send for OwnedRmAllocString {}

unsafe impl Sync for OwnedRmAllocString {}

#[derive(Clone)]
#[repr(C)]
pub struct ConstString {
    str: *const c_char,
    len: u32,
}

impl ConstString {
    pub fn new(str: *const c_char, len: u32) -> Self {
        Self { str, len }
    }
}

impl fmt::Debug for ConstString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for ConstString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

unsafe impl Send for ConstString {}

unsafe impl Sync for ConstString {}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct RedisStringRef {
    str: *const RedisModuleString,
}

impl RedisStringRef {
    /// Create a new [`BorrowedRedisString`] from a borrowed [`RedisString`].
    /// This does not increment the [`RedisString`]'s reference count.
    ///
    /// # Safety
    /// The produced [`BorrowedRedisString`] must not outlive the [`RedisString`] was is created from.
    pub unsafe fn new_unchecked(str: *const RedisModuleString) -> Self {
        Self { str }
    }

    pub fn retain(&self) -> OwnedRedisString {
        unsafe { OwnedRedisString::retain(self.str) }
    }
}

unsafe impl Send for RedisStringRef {}

unsafe impl Sync for RedisStringRef {}

#[repr(transparent)]
pub struct OwnedRedisString {
    str: NonNull<RedisModuleString>,
}

impl OwnedRedisString {
    pub unsafe fn retain(str: *const RedisModuleString) -> Self {
        let ctx = Context::dummy();
        let s =
            RedisString::from_redis_module_string(ctx.get_raw(), str as *mut _).safe_clone(&ctx);
        let str = NonNull::new(s.inner).expect("Error cloning RedisModuleString");
        std::mem::forget(s);
        OwnedRedisString { str }
    }

    pub unsafe fn take(str: NonNull<RedisModuleString>) -> Self {
        Self { str }
    }
}

impl Drop for OwnedRedisString {
    fn drop(&mut self) {
        let ctx = Context::dummy();
        drop(RedisString::from_redis_module_string(
            ctx.get_raw(),
            self.str.as_ptr(),
        ));
    }
}

impl Clone for OwnedRedisString {
    fn clone(&self) -> Self {
        unsafe { OwnedRedisString::retain(self.str.as_ptr()) }
    }
}

impl fmt::Debug for OwnedRedisString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl fmt::Display for OwnedRedisString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

unsafe impl Send for OwnedRedisString {}

unsafe impl Sync for OwnedRedisString {}

struct RsValueStringData {
    data: *const u8,
    len: u32,
}

impl RsValueStringData {
    /// # Safety:
    /// - The length of `s` must not be 0
    /// - The length of `s` must not exceed `isize::MAX`
    /// - The length if `s` must not exceed `u32::MAX`
    unsafe fn copy_from_str_unchecked(s: &str) -> Self {
        debug_assert!(!s.is_empty());
        debug_assert!(s.len() <= u32::MAX as usize);

        // Safety:
        // Caller ensures that the length of `s` is greater than 0,
        // therefore the layout's size is greater than 0.
        let s_cpy = unsafe { alloc::alloc(Self::layout(s.len())) };
        // Safety:
        // - `s_cpy` results from a fresh alloc, and thus does not overlap with
        //   `s`.
        // - `s` is an `&str`, and as such `s.as_ptr()` is valid for reads of `s.len()` bytes
        // - `s_cpy` has been allocated with a layout of `s.len()` size, making it
        //   valid for writing `s.len()` bytes.
        //
        unsafe { s_cpy.copy_from_nonoverlapping(s.as_ptr(), s.len()) };

        Self {
            len: s.len() as u32,
            data: s_cpy,
        }
    }

    /// Safety:
    /// - `s` must be a valid pointer to a C `char` sequence of `len` length
    /// - `s` must point to a valid UTF-8 byte sequence
    /// - `len` must not be 0
    /// - `len` must not exceed `isize::MAX`
    unsafe fn copy_from_c_chars_unchecked(s: *const c_char, len: u32) -> Self {
        debug_assert!(!s.is_null());
        debug_assert!(len > 0);
        debug_assert!(len <= isize::MAX as u32);

        #[cfg(debug_assertions)]
        str::from_utf8(unsafe { std::slice::from_raw_parts(s as *const u8, len as usize) })
            .expect("Invalid UTF-8 sequence");

        let s_cpy = unsafe { alloc::alloc(Self::layout(len as usize)) };

        unsafe { s_cpy.copy_from_nonoverlapping(s as *const u8, len as usize) };

        Self { len, data: s_cpy }
    }

    pub const fn as_str(&self) -> &str {
        // Safety:
        // `self.data` and `self.len` are obtained from an existing `&str`.
        let s = unsafe { std::slice::from_raw_parts(self.data, self.len as usize) };
        // Safety:
        // `s` is obtained from an existing `&str`, or by a call to
        // `Self::copy_from_c_chars_unchecked`, in which case the caller
        // validats that the passed string is valid UTF-8.
        unsafe { std::str::from_utf8_unchecked(s) }
    }

    fn layout(len: usize) -> Layout {
        debug_assert!(len <= isize::MAX as usize);
        Layout::array::<u8>(len).expect("Length exceeds `isize::MAX`")
    }
}

impl Drop for RsValueStringData {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(
                self.data as *mut u8,
                Layout::array::<u8>(self.len as usize).unwrap(),
            );
        }
    }
}

impl fmt::Debug for RsValueStringData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for RsValueStringData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

unsafe impl Send for RsValueStringData {}

unsafe impl Sync for RsValueStringData {}

/// A string type optimized for use in [`crate::RsValueInternal`].
///
#[repr(C)]
pub struct RsValueString {
    // Like [`Arc<str>`], but is not a fat pointer
    // and thus is smaller. Furthermore, it's FFI-safe
    s: Option<NonNull<RsValueStringData>>,
}

impl RsValueString {
    pub fn copy_from_str(s: &str) -> Result<Self, CreateRsValueStringError> {
        if s.is_empty() {
            return Ok(Self { s: None });
        }

        if s.len() > isize::MAX as usize {
            return Err(CreateRsValueStringError::TooLong);
        }

        if s.len() > u32::MAX as usize {
            return Err(CreateRsValueStringError::TooLong);
        }

        // Safety:
        // We've ensured `s.len` is not zero and does not
        // exceed either `isize::MAX` or `u32::MAX`
        let data = unsafe { RsValueStringData::copy_from_str_unchecked(s) };

        Ok(Self::from_data(data))
    }

    /// # Safety
    /// - `s` must be a valid pointer to a C `char` sequence of `len` length
    /// - `s` must point to a valid UTF-8 byte sequence
    /// - `len` must not exceed `isize::MAX`
    pub unsafe fn copy_from_c_chars(
        s: *const c_char,
        len: u32,
    ) -> Result<Self, CreateRsValueStringError> {
        if len == 0 {
            return Ok(Self { s: None });
        }

        if len as usize > isize::MAX as usize {
            return Err(CreateRsValueStringError::TooLong);
        }

        // Safety:
        // `len` is validated to be greater than zero
        // and to not exceed `isize::MAX`.
        // The caller must ensure the other safety requirements are met.
        let data = unsafe { RsValueStringData::copy_from_c_chars_unchecked(s, len) };

        Ok(Self::from_data(data))
    }

    fn from_data(data: RsValueStringData) -> Self {
        let data = Arc::new(data);
        let data_ptr = Arc::into_raw(data);
        // Safety:
        // `data_ptr` originates from the `Arc` that was just created
        // and is therefore non-null.
        let data_ptr = unsafe { NonNull::new_unchecked(data_ptr as *mut _) };
        Self { s: Some(data_ptr) }
    }

    pub const fn as_str(&self) -> &str {
        if let Some(ptr) = self.s {
            // Safety:
            // `ptr` originates from a call to either
            // `Self::copy_from_str` or `copy_from_c_chars` which
            // both either guarantee or require that they result
            // in a valid `RsValueStringData` being owned by `Self`
            let data = unsafe { ptr.as_ref() };
            data.as_str()
        } else {
            ""
        }
    }
}

impl fmt::Debug for RsValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for RsValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl Clone for RsValueString {
    fn clone(&self) -> Self {
        let s = self.s.map(|s| {
            unsafe {
                Arc::increment_strong_count(s.as_ptr());
            }
            s
        });
        Self { s }
    }
}

impl Drop for RsValueString {
    fn drop(&mut self) {
        if let Some(s) = self.s {
            // Safety:
            // `s` was obtained from a call to `Arc::into_raw`,
            // and is non-null.
            drop(unsafe { Arc::from_raw(s.as_ptr()) })
        }
    }
}

unsafe impl Send for RsValueString {}

unsafe impl Sync for RsValueString {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
pub enum CreateRsValueStringError {
    #[error("The input string is too long")]
    TooLong,
}
