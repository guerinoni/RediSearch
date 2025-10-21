/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use std::ptr::NonNull;

use libc::size_t;
use value::{
    collection::{RsValueArray, RsValueMap, RsValueMapEntry},
    shared::SharedRsValue,
};

/// Create a new, uninitialized `RsValueMap`, reserving space for `cap`
/// entries. The map entries are uninitialized and must be set using `RSValueMap_SetEntry`.
/// @param cap the number of entries (key and value) of capacity the map needs to get
/// @returns an uninitialized `RsValueMap` of `cap` capacity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RsValueMap_AllocUninit(cap: u32) -> RsValueMap {
    unsafe { RsValueMap::reserve_uninit(cap) }
}

/// Set a key-value pair at a specific index in the map.
/// Takes ownership of both the key and value RSValues.
///
/// # Safety
/// - `map` must be a valid pointer to an `RsValueMap` that
///   has been created by `RsValueMap_AllocUninit`.
/// - `i` must smaller than the capacity of the `RsValueMap`.
///
/// @param map The map to modify
/// @param i The index where to set the entry (must be < map->len)
/// @param key The key RSValue (ownership is transferred to the map)
/// @param value The value RSValue (ownership is transferred to the map)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RsValueMap_SetEntry(
    map: Option<NonNull<RsValueMap>>,
    i: size_t,
    key: SharedRsValue,
    value: SharedRsValue,
) {
    let map = unsafe { debug_unwrap!(map) };
    let map_refm: &mut RsValueMap = unsafe { &mut *map.as_ptr() };

    let entry = RsValueMapEntry::new(key, value);

    let i = unsafe { debug_unwrap!(i.try_into().ok(), "`i` should fit in a u32") };

    unsafe { map_refm.inner_mut().write_entry(entry, i) };
}

#[unsafe(no_mangle)]
pub extern "C" fn RsValueArray_AllocUninit(cap: u32) -> RsValueArray {
    unsafe { RsValueArray::reserve_uninit(cap) }
}

#[unsafe(no_mangle)]
pub extern "C" fn RsValueArray_SetEntry(arr: Option<NonNull<RsValueArray>>, i: size_t, value: SharedRsValue) {
    let arr = unsafe { debug_unwrap!(arr) };
    let arr_refm: &mut RsValueArray = unsafe { &mut *arr.as_ptr() };
    
    let i = unsafe { debug_unwrap!(i.try_into().ok(), "`i` should fit in a u32") };
    
    unsafe { arr_refm.inner_mut().write_entry(value, i) };
}
