# Alkahest - Fantastic serialization library.

[![crates](https://img.shields.io/crates/v/alkahest.svg?style=for-the-badge&label=alkahest)](https://crates.io/crates/alkahest)
[![docs](https://img.shields.io/badge/docs.rs-alkahest-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white)](https://docs.rs/alkahest)
[![actions](https://img.shields.io/github/workflow/status/zakarumych/alkahest/badge/master?style=for-the-badge)](https://github.com/zakarumych/alkahest/actions?query=workflow%3ARust)
[![MIT/Apache](https://img.shields.io/badge/license-MIT%2FApache-blue.svg?style=for-the-badge)](COPYING)
![loc](https://img.shields.io/tokei/lines/github/zakarumych/alkahest?style=for-the-badge)

*Alkahest* is blazing-fast, zero-deps, zero-overhead, zero-unsafe, schema-based
serialization library.
It is suitable for broad range of use-cases, but tailored for
for custom high-performance network protocols.

By separating schema definition (aka formula in this lib) *alkahest* gives
better guarantees for cases when types used for serialization and deserialization
are different.
It also supports serializing from iterators instead of collections
and lazy deserialization that defers costly process and may omit it entirely if
value is never accessed.
User controls lazyness on type level by choosing `Deserialize` impls.
For instance deserializing into `Vec<T>` is eager because `Vec<T>` is constructed
with all `T` instances and memory allocated for them.
While `alkahest::SliceIter` implements `Iterator` and deserializes
elements in `Iterator::next` and other methods.

# Formula, Serialize and Deserialize traits.

The crate works using three fundamental traits.
`Formula`, `Serialize` and `Deserialize`.
There's also supporting traits - `UnsizedFormula` and `NonRefFormula`.

*Alkahest* provides derive macros for `Formula`, `Serialize` and `Deserialize`.

## Formula

`Formula` trait is used to allow types to serve as data schemas.
Any value serialized with given formula should be deserializable with the same
formula. Sharing only `Formula` type allows modules and crates
easily communicate.
`Formula` dictates binary data layout and it *must* be platform-independent.

Potentially `Formula` types can be generated from separate files,
opening possibility for cross-language communication.

`Formula` is implemented for a number of types out-of-the-box.
Primitive types like `bool`, integers and floating point types all implement `Formula`.
This excludes `isize` and `usize`.
In their place there's `FixedUsize` and `FixedIsize` types provided,
whose size is controlled by a feature-flag.
*!Caveat!*:
  Sizes and addresses are serialized as `FixedUsize`.
  Truncating `usize` value if it was too large.
  This may result in broken data generated and panic in debug.
  Increase size of the `FixedUsize` if you encounter this.
It is also implemented for tuples, array and slice, `Option` and `Vec` (the later requires `"alloc"` feature).

The easiest way to define a new formula is to derive `Formula` trait for a struct or an enum.
Generics are supported, but may require complex bounds specified in attributes for
`Serialize` and `Deserialize` derive macros.
The only constrain is that all fields must implement `Formula`.

For structs `UnsizedFormula` (super-trait of `Formula`) can be derived instead,
allowing last field to be a slice or another `UnsizedFormula` type.
Other fields still have to implement `Formula`.

`UnsizedFormula` impls can also be wrapped in `Ref` type to convert it to a `Formula`.
Therefore `Ref<[T]>` is a `Formula` given that `T` is a `Formula`.
`Ref` formula contains address and length of a value.

## Serialize

`Serialize<Formula>` trait is used to implement serialization
according to a specific formula.
Serialization writes to mutable bytes slice and *should not*
perform dynamic allocations.
Binary result of any type serialized with a formula must follow it.
At the end, if a stream of primitives serialized is the same,
binary result should be the same.
Types may be serializable with different formulas producing
different binary result.

`Serialize` is implemented for many types.
Most notably there's implementation `T: Serialize<T>`
and `&T: Serialize<T>` for all primitives `T` (except `usize` and `isize`).
Another important implementation is
`Serialize<F> for I where I: IntoIterator, I::Item: Serialize<F>`,
allowing serializing into slice directly from both iterators and collections.
Serialization with formula `Ref<F>` uses serialization with formula `F`
and then stores relative address and size. No dynamic allocations is required.

Deriving `Serialize` for a type will generate `Serialize` implementation,
formula is specified in attribute `#[alkahest(FormulaRef)]` or
`#[alkahest(serialize(FormulaRef))]`. `FormulaRef` is typically a type.
When generics are used it also contains generic parameters and bounds.
If formula is not specified - `Self` is assumed.
`Formula` should be derived for the type as well.
It is in-advised to derive `Serialize` for formulas with
manual `Formula` implementation,
`Serialize` derive macro generates code that uses non-public items
generated by `Formula` derive macro.
So either both *should have* manual implementation or both derived.

For structures `Serialize` derive macro requires that all fields
are present on both `Serialize` and `Formula` structure and has the same
order (trivially if this is the same structure).

For enums `Serialize` derive macro checks that for each variant there
exists variant on `Formula` enum.
Variants content is compared similar to structs.
Serialization inserts variant ID and serializes variant as struct.
The size of variants may vary. Padding is inserted by outer value serialization
if necessary.

`Serialize` can be derived for structure where `Formula` is an enum.
In this case variant should be specified using
`#[alkahest(@variant_ident)]` or `#[alkahest(serialize(@variant_ident))]`
and then `Serialize` derive macro will produce serialization code that works
as if this variant was a formula,
except that variant's ID will be serialized before fields.

For enums

## Deserialize

`Deserialize<'de, Formula>` trait is used to implement deserialization 
according to a specific formula.
Deserialization reads from bytes slice constructs deserialized value.
Deserialization *should not* perform dynamic allocations except those
that required to construct and initialize deserialized value.
E.g. it is allowed to allocate when `Vec<T>` is produced if non-zero
number of `T` values are deserialized. It *should not* over-allocate.

Similar to `Serialize` *alkahest* provides a number of out-of-the-box
implementations of `Deserialize` trait.
`From<T>` types can be deserialized with primitive formula `T`.

Values that can be deserialized with formula `F`
can also deserialize with `Ref<F>`, it reads address and length
and proceeds with formula `F`.

`Vec<T>` may deserialize with slice formula.
`Deserialize<'de, [F]>` is implemented for `alkahest::SliceIter<'de, T>` type
that implements `Iterator` and lazily deserialize elements of type
`T: Deserialize<'de, F>`. `SliceIter` is cloneable,
can be iterated from both ends and skips elements for in constant time.
For convenience `SliceIter` also deserializes with array formula.

Deriving `Deserialize` for a type will generate `Deserialize` implementation,
formula is specified in attribute `#[alkahest(FormulaRef)]` or
`#[alkahest(deserialize(FormulaRef))]`. `FormulaRef` is typically a type.
When generics are used it also contains generic parameters and bounds.
If formula is not specified - `Self` is assumed.
`Formula` should be derived for the type as well.
It is in-advised to derive `Deserialize` for formulas with
manual `Formula` implementation,
`Deserialize` derive macro generates code that uses non-public items
generated by `Formula` derive macro.
So either both *should have* manual implementation or both derived.



# Benchmarking

Alkahest comes with a benchmark to test against other popular serialization crates.
Simply run `cargo bench --all-features` to see results.

## License

Licensed under either of

* Apache License, Version 2.0, ([license/APACHE](license/APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([license/MIT](license/MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
