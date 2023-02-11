use core::{iter::FusedIterator, marker::PhantomData, mem::size_of, str::Utf8Error};

use crate::{
    cold::{cold, err},
    formula::{unwrap_size, BareFormula, Formula},
    size::{FixedIsizeType, FixedUsize, FixedUsizeType},
};

/// Error that can occur during deserialization.
#[derive(Clone, Copy, Debug)]
pub enum Error {
    /// Indicates that input buffer is smaller than
    /// expected value length.
    OutOfBounds,

    /// Relative address is invalid.
    WrongAddress,

    /// Incorrect expected value length.
    WrongLength,

    /// Size value exceeds the maximum `usize` for current platform.
    InvalidUsize(FixedUsizeType),

    /// Size value exceeds the maximum `isize` for current platform.
    InvalidIsize(FixedIsizeType),

    /// Enum variant is invalid.
    WrongVariant(u32),

    /// Bytes slice is not UTF8 where `str` is expected.
    NonUtf8(Utf8Error),
}

/// Trait for types that can be deserialized
/// from raw bytes with specified `F: `[`BareFormula`].
pub trait Deserialize<'de, F: Formula + ?Sized> {
    /// Deserializes value provided deserializer.
    /// Returns deserialized value and the number of bytes consumed from
    /// the and of input.
    ///
    /// The value appears at the end of the slice.
    /// And referenced values are addressed from the beginning of the slice.
    fn deserialize(deserializer: Deserializer<'de>) -> Result<Self, Error>
    where
        Self: Sized;

    /// Deserializes value in-place provided deserializer.
    /// Overwrites `self` with data from the `input`.
    ///
    /// The value appears at the end of the slice.
    /// And referenced values are addressed from the beginning of the slice.
    fn deserialize_in_place(&mut self, deserializer: Deserializer<'de>) -> Result<(), Error>;
}

#[must_use]
#[derive(Clone)]
pub struct Deserializer<'de> {
    /// Input buffer sub-slice usable for deserialization.
    input: &'de [u8],
    stack: usize,
}

impl<'de> Deserializer<'de> {
    #[must_use]
    #[inline(always)]
    pub fn new(stack: usize, input: &'de [u8]) -> Result<Self, Error> {
        if stack > input.len() {
            return err(Error::OutOfBounds);
        }
        Ok(Self::new_unchecked(stack, input))
    }

    #[must_use]
    #[inline(always)]
    pub const fn new_unchecked(stack: usize, input: &'de [u8]) -> Self {
        debug_assert!(stack <= input.len());
        Deserializer { input, stack }
    }

    #[must_use]
    #[inline(always)]
    #[track_caller]
    pub(crate) fn sub(&mut self, stack: usize) -> Result<Self, Error> {
        if self.stack < stack {
            return err(Error::WrongLength);
        }

        let sub = Deserializer::new_unchecked(stack, self.input);

        self.stack -= stack;
        let end = self.input.len() - stack;
        self.input = &self.input[..end];
        Ok(sub)
    }

    #[inline(always)]
    pub fn read_bytes(&mut self, len: usize) -> Result<&'de [u8], Error> {
        if len > self.stack {
            return err(Error::WrongLength);
        }
        let at = self.input.len() - len;
        let (head, tail) = self.input.split_at(at);
        self.input = head;
        self.stack -= len;
        Ok(tail)
    }

    #[inline(always)]
    pub fn read_all_bytes(self) -> &'de [u8] {
        let at = self.input.len() - self.stack;
        &self.input[at..]
    }

    #[inline(always)]
    #[track_caller]
    pub fn read_value<F, T>(&mut self, last: bool) -> Result<T, Error>
    where
        F: Formula + ?Sized,
        T: Deserialize<'de, F>,
    {
        let stack = match (last, F::MAX_STACK_SIZE) {
            (true, _) => self.stack,
            (false, Some(max_stack)) => max_stack,
            (false, None) => self.read_auto::<FixedUsize>(false)?.into(),
        };

        <T as Deserialize<'de, F>>::deserialize(self.sub(stack)?)
    }

    #[inline(always)]
    pub fn skip_values<F>(&mut self, n: usize) -> Result<(), Error>
    where
        F: Formula + ?Sized,
    {
        if n == 0 {
            return Ok(());
        }

        match F::MAX_STACK_SIZE {
            None => {
                for _ in 0..n {
                    let skip_bytes = self.read_auto::<FixedUsize>(false)?;
                    self.read_bytes(skip_bytes.into())?;
                }
            }
            Some(max_stack) => {
                let skip_bytes = max_stack * (n - 1);
                self.read_bytes(skip_bytes)?;
            }
        }
        Ok(())
    }

    #[inline(always)]
    #[track_caller]
    pub fn read_auto<T>(&mut self, last: bool) -> Result<T, Error>
    where
        T: BareFormula + Deserialize<'de, T>,
    {
        self.read_value::<T, T>(last)
    }

    #[inline(always)]
    pub fn read_in_place<F, T>(&mut self, place: &mut T, last: bool) -> Result<(), Error>
    where
        F: Formula + ?Sized,
        T: Deserialize<'de, F> + ?Sized,
    {
        let stack = match (last, F::MAX_STACK_SIZE) {
            (true, _) => self.stack,
            (false, Some(max_stack)) => max_stack,
            (false, None) => self.read_auto::<FixedUsize>(false)?.into(),
        };

        <T as Deserialize<'de, F>>::deserialize_in_place(place, self.sub(stack)?)
    }

    #[inline(always)]
    pub fn read_auto_in_place<T>(&mut self, place: &mut T, last: bool) -> Result<(), Error>
    where
        T: BareFormula + Deserialize<'de, T> + ?Sized,
    {
        self.read_in_place::<T, T>(place, last)
    }

    #[inline(always)]
    pub fn deref(mut self) -> Result<Deserializer<'de>, Error> {
        let [address, size] = self.read_auto::<[FixedUsize; 2]>(false)?;

        if usize::from(address) > self.input.len() {
            return err(Error::WrongAddress);
        }

        let input = &self.input[..address.into()];
        self.finish()?;

        Deserializer::new(size.into(), input)
    }

    #[inline(always)]
    pub fn into_iter<F, T>(self) -> Result<DeIter<'de, F, T>, Error>
    where
        F: Formula,
        T: Deserialize<'de, F>,
    {
        Ok(DeIter {
            de: self,
            marker: PhantomData,
        })
    }

    #[inline(always)]
    pub fn finish(self) -> Result<(), Error> {
        if self.stack == 0 {
            Ok(())
        } else {
            err(Error::WrongLength)
        }
    }
}

pub struct DeIter<'de, F: ?Sized, T> {
    de: Deserializer<'de>,
    marker: PhantomData<fn(&F) -> T>,
}

impl<'de, F, T> Clone for DeIter<'de, F, T>
where
    F: ?Sized,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        DeIter {
            de: self.de.clone(),
            marker: PhantomData,
        }
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        self.de = source.de.clone();
    }
}

impl<'de, F, T> Iterator for DeIter<'de, F, T>
where
    F: Formula + ?Sized,
    T: Deserialize<'de, F>,
{
    type Item = Result<T, Error>;

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match F::MAX_STACK_SIZE {
            None => (0, Some(self.de.stack / size_of::<FixedUsize>())),
            Some(0) => {
                let count = self.de.stack;
                (count, Some(count))
            }
            Some(max_stack) => {
                let count = (self.de.stack + max_stack - 1) / max_stack;
                (count, Some(count))
            }
        }
    }

    #[inline(always)]
    fn next(&mut self) -> Option<Result<T, Error>> {
        if self.de.stack == 0 {
            return None;
        }

        match self.de.read_value::<F, T>(false) {
            Err(error) => {
                self.de.input = &[];
                self.de.stack = 0;
                Some(err(error))
            }
            Ok(value) => Some(Ok(value)),
        }
    }

    #[inline(always)]
    fn count(self) -> usize {
        match F::MAX_STACK_SIZE {
            None => self.fold(0, |acc, _| acc + 1),
            Some(0) => self.de.stack,
            Some(max_stack) => (self.de.stack + max_stack - 1) / max_stack,
        }
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<Result<T, Error>> {
        if n > 0 {
            if let Err(_) = self.de.skip_values::<F>(n) {
                return None;
            }
        }
        self.next()
    }

    #[inline(always)]
    fn fold<B, Fun>(mut self, init: B, mut f: Fun) -> B
    where
        Fun: FnMut(B, Result<T, Error>) -> B,
    {
        let mut accum = init;
        loop {
            let result = self.de.read_value::<F, T>(false);
            if let Err(Error::WrongLength) = result {
                self.de.input = &[];
                self.de.stack = 0;
                if self.de.stack == 0 {
                    return accum;
                }
                cold();
                return f(accum, result);
            }
            accum = f(accum, result);
        }
    }
}

impl<'de, F, T> DoubleEndedIterator for DeIter<'de, F, T>
where
    F: Formula + ?Sized,
    T: Deserialize<'de, F>,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Result<T, Error>> {
        todo!()
    }

    #[inline(always)]
    fn nth_back(&mut self, n: usize) -> Option<Result<T, Error>> {
        todo!()
    }

    #[inline(always)]
    fn rfold<B, Fun>(self, init: B, mut f: Fun) -> B
    where
        Fun: FnMut(B, Result<T, Error>) -> B,
    {
        todo!()
    }
}

impl<'de, F, T> ExactSizeIterator for DeIter<'de, F, T>
where
    F: Formula + ?Sized,
    T: Deserialize<'de, F>,
{
    #[inline(always)]
    fn len(&self) -> usize {
        todo!()
    }
}

impl<'de, F, T> FusedIterator for DeIter<'de, F, T>
where
    F: Formula + ?Sized,
    T: Deserialize<'de, F>,
{
}

#[inline(always)]
pub fn value_size(input: &[u8]) -> Option<usize> {
    if input.len() < FIELD_SIZE {
        return None;
    }

    let mut de = Deserializer::new_unchecked(FIELD_SIZE, &input[..FIELD_SIZE]);
    Some(de.read_auto::<FixedUsize>(false).map(usize::from).unwrap())
}

#[inline(always)]
pub fn deserialize<'de, F, T>(input: &'de [u8]) -> Result<(T, usize), Error>
where
    F: Formula + ?Sized,
    T: Deserialize<'de, F>,
{
    if input.len() < HEADER_SIZE {
        return err(Error::OutOfBounds);
    }

    let mut de = Deserializer::new_unchecked(HEADER_SIZE, &input[..HEADER_SIZE]);
    let [address, size] = de.read_auto::<[FixedUsize; 2]>(false).unwrap();

    if size > address {
        return err(Error::WrongAddress);
    }

    let end = usize::from(address);

    if end > input.len() {
        return err(Error::OutOfBounds);
    }

    let mut de = Deserializer::new_unchecked(size.into(), &input[..end]);
    let value = de.read_value::<F, T>(true)?;

    Ok((value, end))
}

#[inline(always)]
pub fn deserialize_in_place<'de, F, T>(place: &mut T, input: &'de [u8]) -> Result<usize, Error>
where
    F: BareFormula + ?Sized,
    T: Deserialize<'de, F> + ?Sized,
{
    if input.len() < HEADER_SIZE {
        return err(Error::OutOfBounds);
    }

    let mut de = Deserializer::new_unchecked(HEADER_SIZE, &input[..HEADER_SIZE]);
    let [address, size] = de.read_auto::<[FixedUsize; 2]>(false)?;

    if size > address {
        return err(Error::WrongAddress);
    }

    let end = usize::from(address);

    if end > input.len() {
        return err(Error::OutOfBounds);
    }

    let mut de = Deserializer::new_unchecked(size.into(), &input[..end]);
    de.read_in_place::<F, T>(place, true)?;

    Ok(end)
}

const FIELD_SIZE: usize = size_of::<FixedUsize>();
const HEADER_SIZE: usize = FIELD_SIZE * 2;
