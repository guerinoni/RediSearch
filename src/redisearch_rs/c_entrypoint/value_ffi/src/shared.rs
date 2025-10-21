/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use std::{ffi::c_char, ptr::NonNull, slice};

use libc::strlen;
use redis_module::RedisModuleString;
use value::{
    Value,
    collection::{RsValueArray, RsValueMap},
    shared::SharedRsValue,
    strings::RsValueString,
};

/// Creates a heap-allocated `RsValue` wrapping a string.
/// Doesn't duplicate the string. Use strdup if the value needs to be detached.
/// @param str The string to wrap (ownership is transferred)
/// @param len The length of the string
/// @return A pointer to a heap-allocated RsValue
///
/// # Safety
/// - `str` must point to a valid, NULL-terminated C string with a length of at most `u32::MAX` bytes.
/// - `str` must not be aliased.
///
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewString(
    str: Option<NonNull<c_char>>,
    len: u32,
) -> SharedRsValue {
    let str = unsafe { debug_unwrap!(str) };
    // unsafe { SharedRsValue::malloc_string(str, len) }

    let s = unsafe { RsValueString::copy_from_c_chars(str.as_ptr(), len) };

    let s = unsafe { debug_unwrap!(s, "error creating RsValueString") };

    SharedRsValue::string(s)
}

/**
 * Creates a heap-allocated RSValue wrapping a null-terminated C string.
 *
 * # Safety
 *
 * - `str` must point to a valid, NULL-terminated C string with a length of at most `u32::MAX` bytes.
 * - `str` must not be aliased.
 *
 * @param str The null-terminated string to wrap (ownership is transferred)
 * @return A pointer to a heap-allocated RSValue
 */
pub unsafe extern "C" fn SharedRsValue_NewCString(str: Option<NonNull<c_char>>) -> SharedRsValue {
    // Safety:
    // Caller must ensure `str` is a valid pointer to a C string.
    let str = unsafe { debug_unwrap!(str) };

    let len = {
        // Safety:
        // Caller must ensure `str` is a NULL-terminated C string
        unsafe { strlen(str.as_ptr()) }
    };

    let len =
        unsafe { debug_unwrap!(len.try_into(), "Length of str cannot be more than u32::MAX") };

    unsafe { SharedRsValue_NewString(Some(str), len) }
}

/// Creates a heap-allocated `RsValue` wrapping a const string.
///
/// # Safety
/// - `str` must be a valid const pointer to a char sequence of `len` chars.
///
/// @param str The null-terminated string to wrap (ownership is transferred)
/// @return A pointer to a heap-allocated RsValue wrapping a constant C string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewConstString(
    str: *const c_char,
    len: u32,
) -> SharedRsValue {
    // unsafe { SharedRsValue::const_string(str, len) }
    unsafe { SharedRsValue_NewString(NonNull::new(str as *mut c_char), len) }
}

///
/// # Safety
/// - `str` must be a valid pointer to a [`RedisModuleString`]
/// - The [`RedisModuleString`] `str` points to must be valid UTF-8
pub unsafe extern "C" fn SharedRsValue_NewCopiedRedisString(
    str: *const RedisModuleString,
) -> SharedRsValue {
    debug_assert!(!str.is_null(), "`str` must not be NULL");

    // Note: `from_ptr` is erroneously not marked unsafe,
    // but it does not validate whether it points to a valid
    // [`RedisModuleString`]
    let str = redis_module::RedisString::from_ptr(str);
    // Safety:
    // Caller is required to ensure the RedisModule string is valid UTF-8.
    let str = unsafe { debug_unwrap!(str, "passed string was not UTF-8") };
    let str = RsValueString::copy_from_str(str);
    let str = unsafe { debug_unwrap!(str, "failed to create RsValueString") };
    SharedRsValue::string(str)
}

/// Creates a heap-allocated `RsValue` wrapping a RedisModuleString.
/// Does not increment the refcount of the Redis string.
/// The passed Redis string's refcount does not get decremented
/// upon freeing the returned RsValue.
/// @param str The RedisModuleString to wrap
/// @return A pointer to a heap-allocated RsValue
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewBorrowedRedisString(
    str: *const RedisModuleString,
) -> SharedRsValue {
    // unsafe { SharedRsValue::borrowed_redis_string(str) }
    unsafe { SharedRsValue_NewCopiedRedisString(str) }
}

/// Creates a heap-allocated `RsValue` which increments and owns a reference to the Redis string.
/// The RsValue will decrement the refcount when freed.
/// @param str The RedisModuleString to wrap (refcount is incremented)
/// @return A pointer to a heap-allocated RsValue
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewOwnedRedisString(
    str: *const RedisModuleString,
) -> SharedRsValue {
    // unsafe { SharedRsValue::retain_owned_redis_string(str) }
    unsafe { SharedRsValue_NewCopiedRedisString(str) }
}

/// Creates a heap-allocated `RsValue` which steals a reference to the Redis string.
/// The caller's reference is transferred to the RsValue.
/// @param s The RedisModuleString to wrap (ownership is transferred)
/// @return A pointer to a heap-allocated RsValue
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewStolenRedisString(
    str: Option<NonNull<RedisModuleString>>,
) -> SharedRsValue {
    let str = unsafe { debug_unwrap!(str) };
    // unsafe { SharedRsValue::take_owned_redis_string(str) }
    unsafe { SharedRsValue_NewCopiedRedisString(str.as_ptr() as *const _) }
}

/// Creates a heap-allocated `RsValue` with a copied string.
/// The string is duplicated using `rm_malloc`.
///
/// # Safety
/// - `str` must be a valid pointer to a char sequence of `len` chars.
///
/// @param s The string to copy
/// @param dst The length of the string to copy
/// @return A pointer to a heap-allocated `RsValue` owning the copied string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewCopiedString(
    str: *const c_char,
    len: u32,
) -> SharedRsValue {
    // unsafe { SharedRsValue::copy_malloc_string(str, len) }
    let str = unsafe { RsValueString::copy_from_c_chars(str, len) };
    let str = unsafe { debug_unwrap!(str, "failed to create RsValueString") };
    SharedRsValue::string(str)
}

/// Creates a heap-allocated `RsValue` by parsing a string as a number.
/// Returns an undefined value if the string cannot be parsed as a valid number.
///
/// # Safety
/// - `str` must be a valid const pointer to a char sequence of `len` chars.
///
/// @param p The string to parse
/// @param l The length of the string
/// @return A pointer to a heap-allocated `RsValue`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewParsedNumber(
    str: *const c_char,
    len: usize,
) -> SharedRsValue {
    if len == 0 {
        return SharedRsValue::undefined();
    }

    let str = unsafe { std::slice::from_raw_parts(str as *const u8, len) };
    let Ok(str) = std::str::from_utf8(str) else {
        return SharedRsValue::undefined();
    };
    let Ok(n) = str.parse() else {
        return SharedRsValue::undefined();
    };
    SharedRsValue::number(n)
}

/// Creates a heap-allocated `RsValue` containing a number.
/// @param n The numeric value to wrap
/// @return A pointer to a heap-allocated `RsValue` of type `RsValueType_Number`
#[unsafe(no_mangle)]
pub extern "C" fn SharedRsValue_NewNumber(n: f64) -> SharedRsValue {
    SharedRsValue::number(n)
}

/// Creates a heap-allocated `RsValue` containing a number from an int64.
/// This operation casts the passed `i64` to an `f64`, possibly losing information.
/// @param ii The int64 value to convert and wrap
/// @return A pointer to a heap-allocated `RsValue` of type `RsValueType_Number`
#[unsafe(no_mangle)]
pub extern "C" fn SharedRsValue_NewNumberFromInt64(dd: i64) -> SharedRsValue {
    SharedRsValue::number(dd as f64)
}

/// Creates a heap-allocated `RsValue` array from existing values.
/// Takes ownership of the values (values will be freed when array is freed).
/// @param vals The values array to use for the array (ownership is transferred)
/// @param len Number of values
/// @return A pointer to a heap-allocated `RsValue` of type `RsValueType_Array`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewArray(
    vals: Option<NonNull<SharedRsValue>>,
    len: u32,
) -> SharedRsValue {
    let vals = if len == 0 {
        &[]
    } else {
        let vals = unsafe { debug_unwrap!(vals) };
        unsafe { slice::from_raw_parts(vals.as_ptr(), len as usize) }
    };

    todo!()
}

/// Creates a heap-allocated RsValue of type RsValue_Map from an RsValueMap.
/// Takes ownership of the map structure and all its entries.
/// @param map The RsValueMap to wrap (ownership is transferred)
/// @return A pointer to a heap-allocated RsValue of type RsValueType_Map
#[unsafe(no_mangle)]
pub extern "C" fn SharedRsValue_NewMap(map: RsValueMap) -> SharedRsValue {
    SharedRsValue::map(map)
}

/// Creates a heap-allocated RsValue array from NULL terminated C strings.
/// @param strs Array of string pointers
/// @param sz Number of strings in the array
/// @return A pointer to a heap-allocated RsValue array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_NewStringArray(
    strs: Option<NonNull<Option<NonNull<c_char>>>>,
    sz: u32,
) -> SharedRsValue {
    let strs = if sz == 0 {
        &[]
    } else {
        let strs = unsafe { debug_unwrap!(strs) };
        unsafe { std::slice::from_raw_parts(strs.as_ptr(), sz as usize) }
    };
    let mut array = unsafe { RsValueArray::reserve_uninit(sz) };

    strs.iter()
        .copied()
        .map(|str| unsafe { SharedRsValue_NewCString(str) })
        .enumerate()
        .for_each(|(i, v)| unsafe { array.inner_mut().write_entry(v, i as u32) });

    SharedRsValue::array(array)
}

/// Creates a heap-allocated RsValue array from NULL terminated C string constants.
/// @param strs Array of string pointers
/// @param sz Number of strings in the array
/// @return A pointer to a heap-allocated RsValue array
#[unsafe(no_mangle)]
pub extern "C" fn SharedRsValue_NewConstStringArray(
    strs: *mut *const c_char,
    sz: u32,
) -> SharedRsValue {
    todo!()
}

/// Creates a heap-allocated RsValue Trio from three RsValues.
/// Takes ownership of all three values.
/// @param left The left value (ownership is transferred)
/// @param middle The middle value (ownership is transferred)
/// @param right The right value (ownership is transferred)
/// @return A pointer to a heap-allocated RsValue of type RsValueType_Trio
#[unsafe(no_mangle)]
pub extern "C" fn SharedRsValue_NewTrio(
    left: SharedRsValue,
    middle: SharedRsValue,
    right: SharedRsValue,
) -> SharedRsValue {
    SharedRsValue::trio(left, middle, right)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn SharedRsValue_Number_Get(v: &SharedRsValue) -> f64 {
    unsafe { debug_unwrap!(v.get_number(), "v should be of type 'Number'") }
}
