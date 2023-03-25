#![doc = include_str!("../README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings)]
#![doc(test(attr(deny(warnings))))]

extern crate self as alkahest;

#[cfg(feature = "alloc")]
extern crate alloc;

mod array;
mod r#as;
mod buffer;
mod bytes;
mod cold;
mod deserialize;
mod formula;
mod iter;
mod lazy;
mod option;
mod primitive;
mod reference;
mod serialize;
mod size;
mod skip;
mod slice;
mod str;
mod tuple;

#[cfg(test)]
mod tests;

#[cfg(feature = "alloc")]
mod vec;

#[cfg(feature = "alloc")]
mod vec_deque;

#[cfg(feature = "alloc")]
mod string;

#[cfg(feature = "serde-bincode")]
mod bincode;

pub use crate::{
    buffer::{Buffer, BufferExhausted, BufferSizeRequired, MaybeFixedBuffer, UncheckedFixedBuffer},
    bytes::Bytes,
    deserialize::{
        deserialize, deserialize_in_place, value_size, DeIter, Deserialize, DeserializeError,
        Deserializer,
    },
    formula::{max_size, BareFormula, Formula},
    iter::{deserialize_extend_iter, deserialize_from_iter, SerIter},
    lazy::Lazy,
    r#as::As,
    reference::Ref,
    serialize::{
        header_size, serialize, serialize_or_size, serialized_size, Serialize, Serializer,
        SliceWriter,
    },
    size::{FixedIsize, FixedUsize},
    skip::Skip,
    slice::default_iter_fast_sizes,
};

#[cfg(feature = "alloc")]
pub use crate::{buffer::VecBuffer, serialize::serialize_to_vec};

#[cfg(feature = "derive")]
pub use alkahest_proc::{Deserialize, Formula, Serialize};

/// Private module for macros to use.
#[doc(hidden)]
pub mod private {
    pub use {
        bool,
        core::{convert::Into, debug_assert_eq, option::Option, result::Result},
        u32, u8, usize,
    };

    use crate::FixedUsize;
    pub use crate::{
        cold::{cold, err},
        deserialize::{Deserialize, DeserializeError, Deserializer},
        formula::{formula_fast_sizes, max_size, sum_size, BareFormula, Formula},
        serialize::{Serialize, Serializer},
    };

    use core::{marker::PhantomData, mem::size_of};

    pub const VARIANT_SIZE: usize = core::mem::size_of::<u32>();
    pub const VARIANT_SIZE_OPT: Option<usize> = Some(VARIANT_SIZE);

    pub struct WithFormula<F: Formula + ?Sized> {
        marker: PhantomData<fn(&F) -> &F>,
    }

    impl<F> WithFormula<F>
    where
        F: Formula + ?Sized,
    {
        #[inline(always)]
        pub fn write_value<T, S>(self, ser: &mut S, value: T) -> Result<(), S::Error>
        where
            S: Serializer,
            T: Serialize<F>,
        {
            ser.write_value::<F, T>(value)
        }

        #[inline(always)]
        pub fn write_last_value<T, S>(self, ser: S, value: T) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
            T: Serialize<F>,
        {
            ser.write_last_value::<F, T>(value)
        }

        #[inline(always)]
        pub fn read_value<'de, T>(
            self,
            de: &mut Deserializer<'de>,
            last: bool,
        ) -> Result<T, DeserializeError>
        where
            F: Formula,
            T: Deserialize<'de, F>,
        {
            de.read_value::<F, T>(last)
        }

        #[inline(always)]
        pub fn read_in_place<'de, T>(
            self,
            place: &mut T,
            de: &mut Deserializer<'de>,
            last: bool,
        ) -> Result<(), DeserializeError>
        where
            F: Formula,
            T: Deserialize<'de, F>,
        {
            de.read_in_place::<F, T>(place, last)
        }

        #[inline(always)]
        pub fn size_hint<T>(self, value: &T, last: bool) -> Option<usize>
        where
            T: Serialize<F>,
        {
            let size = <T as Serialize<F>>::size_hint(value)?;
            if last || F::MAX_STACK_SIZE.is_some() {
                Some(size)
            } else {
                Some(size + size_of::<FixedUsize>())
            }
        }
    }

    #[inline(always)]
    pub fn with_formula<F: Formula + ?Sized, L: Formula + ?Sized>(
        _: impl FnOnce(&F) -> &L,
    ) -> WithFormula<L> {
        WithFormula {
            marker: PhantomData,
        }
    }
}
