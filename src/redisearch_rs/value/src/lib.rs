/*
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
*/

use redis_module::RedisModuleString;
use sds_rs::OwnedSds;

use std::{ffi::c_char, fmt::{self, Debug}, ptr::NonNull};

use crate::{
    collection::{RsValueArray, RsValueMap},
    shared::SharedRsValue,
    strings::{ConstString, OwnedRedisString, OwnedRmAllocString, RedisStringRef, RsValueString},
    trio::RsValueTrio,
};

/// Ports part of the RediSearch RSValue type to Rust. This is a temporary solution until we have a proper
/// Rust port of the RSValue type.
#[cfg(feature = "c_ffi_impl")]
mod rs_value_ffi;
#[cfg(feature = "c_ffi_impl")]
pub use rs_value_ffi::*;

#[cfg(feature = "test_utils")]
mod test_utils;
#[cfg(feature = "test_utils")]
pub use test_utils::RSValueMock;

pub mod strings;

pub mod collection;
pub mod shared;
pub mod trio;

/// Internal storage of [`RsValue`] and [`SharedRsValue`]
/// cbindgen:prefix-with-name
// TODO: optimize memory size
#[derive(Debug, Clone)]
#[repr(C)]
pub enum RsValueInternal {
    /// Null value
    Null,
    /// Numeric value
    Number(f64),
    /// String value backed by a rm_alloc'd string
    MallocString(OwnedRmAllocString),
    ConstString(ConstString),
    /// Owned SDS value
    Sds(OwnedSds),
    OwnedRedisString(OwnedRedisString),
    BorrowedRedisString(RedisStringRef),
    /// String value
    String(RsValueString),
    /// Array value
    Array(RsValueArray),
    /// Reference value
    Ref(SharedRsValue),
    /// Trio value
    Trio(RsValueTrio),
    /// Map value
    Map(RsValueMap),
}

/// A stack-allocated RediSearch dynamic value.
// TODO: optimize memory layout
/// cbindgen:prefix-with-name
#[derive(Default, Clone)]
#[repr(C)]
pub enum RsValue {
    #[default]
    /// Undefined, not holding a value.
    Undef,
    /// Defined and holding a value.
    Def(RsValueInternal),
}

impl RsValue {
    pub const fn null_const() -> Self {
        Self::Def(RsValueInternal::Null)
    }
}

impl fmt::Debug for RsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.internal() {
            Some(internal) => internal.fmt(f),
            None => f.debug_tuple("Undefined").finish(),
        }
    }
}

impl Value for RsValue {
    fn from_internal(internal: RsValueInternal) -> Self {
        Self::Def(internal)
    }

    fn undefined() -> Self {
        Self::Undef
    }

    fn internal(&self) -> Option<&RsValueInternal> {
        match self {
            Self::Undef => None,
            Self::Def(internal) => Some(internal),
        }
    }
}

pub trait Value: Sized {
    /// Create a new value from an [`RsValueInternal`]
    fn from_internal(internal: RsValueInternal) -> Self;

    // Create a new, undefined value
    fn undefined() -> Self;

    // Clear this value
    fn clear(&mut self) {
        *self = Self::undefined();
    }

    /// Get a reference to the [`RsValueInternal`] that is
    /// held by this value if it is defined. Returns `None` if
    /// the value is undefined.
    fn internal(&self) -> Option<&RsValueInternal>;

    /// Create a new, NULL value
    fn null() -> Self {
        Self::from_internal(RsValueInternal::Null)
    }

    /// Create a new numeric value given the passed number
    fn number(n: f64) -> Self {
        Self::from_internal(RsValueInternal::Number(n))
    }

    /// Create a new string value
    fn string(s: RsValueString) -> Self {
        Self::from_internal(RsValueInternal::String(s))
    }

    fn trio(left: SharedRsValue, middle: SharedRsValue, right: SharedRsValue) -> Self {
        Self::from_internal(RsValueInternal::Trio(RsValueTrio::new(left, middle, right)))
    }
    
    unsafe fn malloc_string(str: NonNull<c_char>, len: u32) -> Self {
        Self::from_internal(RsValueInternal::MallocString(unsafe {
            OwnedRmAllocString::new_unchecked(str, len)
        }))
    }

    unsafe fn copy_malloc_string(str: *const c_char, len: u32) -> Self {
        debug_assert!(!str.is_null(), "`str` must not be NULL");
        Self::from_internal(RsValueInternal::MallocString(unsafe {
            OwnedRmAllocString::copy_from_string(str, len)
        }))
    }

    fn const_string(str: *const c_char, len: u32) -> Self {
        debug_assert!(!str.is_null(), "`str` must not be NULL");
        Self::from_internal(RsValueInternal::ConstString(ConstString::new(str, len)))
    }

    unsafe fn borrowed_redis_string(str: *const RedisModuleString) -> Self {
        debug_assert!(!str.is_null(), "`str` must not be NULL");
        Self::from_internal(RsValueInternal::BorrowedRedisString(unsafe {
            RedisStringRef::new_unchecked(str)
        }))
    }

    unsafe fn retain_owned_redis_string(str: *const RedisModuleString) -> Self {
        debug_assert!(!str.is_null(), "`str` must not be NULL");
        Self::from_internal(RsValueInternal::OwnedRedisString(unsafe {
            OwnedRedisString::retain(str)
        }))
    }

    unsafe fn take_owned_redis_string(str: NonNull<RedisModuleString>) -> Self {
        Self::from_internal(RsValueInternal::OwnedRedisString(unsafe {
            OwnedRedisString::take(str)
        }))
    }
    
    fn sds(sds: OwnedSds) -> Self {
        Self::from_internal(RsValueInternal::Sds(sds))
    }

    fn array(arr: RsValueArray) -> Self {
        Self::from_internal(RsValueInternal::Array(arr))
    }

    fn map(map: RsValueMap) -> Self {
        Self::from_internal(RsValueInternal::Map(map))
    }
    /// Attempt to parse the passed string as an `f64`, and wrap it
    /// in a [`SharedRsValue`].
    fn parse_number(s: &str) -> Result<Self, std::num::ParseFloatError> {
        Ok(Self::number(s.parse()?))
    }

    fn get_number(&self) -> Option<f64> {
        let RsValueInternal::Number(number) = self.internal()? else {
            return None;
        };
        Some(*number)
    }
}
