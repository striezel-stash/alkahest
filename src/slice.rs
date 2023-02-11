use crate::{
    formula::{BareFormula, Formula},
    serialize::Serialize,
};

impl<F> Formula for [F]
where
    F: Formula,
{
    const MAX_STACK_SIZE: Option<usize> = None;
    const EXACT_SIZE: bool = false;
    const HEAPLESS: bool = F::HEAPLESS;
}

impl<F> BareFormula for [F] where F: Formula {}

#[inline(always)]
pub fn default_iter_fast_sizes<F, I>(iter: &I) -> Option<usize>
where
    F: Formula,
    I: Iterator,
    I::Item: Serialize<F>,
{
    default_iter_fast_sizes_unchecked::<F, I>(iter)
}

#[inline(always)]
pub fn default_iter_fast_sizes_unchecked<F, I>(iter: &I) -> Option<usize>
where
    F: Formula,
    I: Iterator,
{
    match (F::EXACT_SIZE, F::HEAPLESS, F::MAX_STACK_SIZE) {
        (true, true, Some(max_stack_size)) => {
            let (lower, upper) = iter.size_hint();
            match upper {
                Some(upper) if upper == lower => {
                    // Expect this to be the truth.
                    // If not, serialization will fail or produce incorrect results.
                    Some(lower * max_stack_size)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[inline(always)]
pub fn default_iter_fast_sizes_owned<F, T, I>(iter: I) -> Option<usize>
where
    F: Formula,
    I: Iterator<Item = T> + Clone,
    T: Serialize<F>,
{
    match (F::EXACT_SIZE, F::HEAPLESS, F::MAX_STACK_SIZE) {
        (true, true, Some(max_stack_size)) => {
            let (lower, upper) = iter.size_hint();
            match upper {
                Some(upper) if upper == lower => {
                    // Expect this to be the truth.
                    // If not, serialization will fail or produce incorrect results.
                    Some(lower * max_stack_size)
                }
                _ => None,
            }
        }
        _ => {
            let (_, upper) = iter.size_hint();
            match upper {
                Some(upper) if upper <= 4 => {
                    let mut size = 0;
                    for elem in iter {
                        size += <T as Serialize<F>>::fast_sizes(&elem)?;
                    }
                    Some(size)
                }
                _ => None,
            }
        }
    }
}

#[inline(always)]
pub fn default_iter_fast_sizes_by_ref<'a, F, T, I>(iter: I) -> Option<usize>
where
    F: Formula,
    I: Iterator<Item = &'a T>,
    T: Serialize<F> + 'a,
{
    match (F::EXACT_SIZE, F::HEAPLESS, F::MAX_STACK_SIZE) {
        (true, true, Some(max_stack_size)) => {
            let (lower, upper) = iter.size_hint();
            match upper {
                Some(upper) if upper == lower => {
                    // Expect this to be the truth.
                    // If not, serialization will fail or produce incorrect results.
                    Some(lower * max_stack_size)
                }
                _ => None,
            }
        }
        _ => {
            let (_, upper) = iter.size_hint();
            match upper {
                Some(upper) if upper <= 4 => {
                    let mut size = 0;
                    for elem in iter {
                        size += <T as Serialize<F>>::fast_sizes(elem)?;
                    }
                    Some(size)
                }
                _ => None,
            }
        }
    }
}
