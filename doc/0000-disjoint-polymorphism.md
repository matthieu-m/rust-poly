- Start Date: (fill me in with today's date, YYYY-MM-DD)
- RFC PR: (leave this empty)
- Rust Issue: (leave this empty)



# Summary

Solving the Servo DOM design requirements (and beyond), while integrating smoothly with the already existing `trait` mechanism.

This RFC provides:

- Struct Polymorphism
- Trait Polymorphism
- Runtime Type Information (to handle safe down-cast)
- Thin Polymorphic Pointer/References Support

> Note: an experimental repository was setup at https://github.com/matthieu-m/rust-poly, it is incomplete because of missing support in the compiler but gives an idea of what this RFC aims for. The curious might find the implementation of functions/methods there.



# Motivation

Rust currently supports polymorphism through its traits, however the experience in Servo has raised a number of requirements which are not fulfilled.

A summary of those requirements is given here:

 - cheap field access from internal methods;
 - cheap dynamic dispatch of methods;
 - cheap down-casting;
 - thin pointers;
 - sharing of fields and methods between definitions;
 - safe, i.e., doesn't require a bunch of transmutes or other unsafe code to be usable;
 - syntactically lightweight or implicit upcasting;
 - calling functions through smartpointers, e.g. fn foo(JSRef<T>, ...);
 - static dispatch of methods.

There have already been a number of proposals (see [Summary of Efficient Inheritance RFCs](https://internals.rust-lang.org/t/summary-of-efficient-inheritance-rfcs/494)).

This RFC is similar in nature to [Fat Objects](https://github.com/rust-lang/rfcs/pull/9) and tries not to focus on building independent bricks, but instead focus on maximizing integration with the existing code and avoid splitting the Rust landscape into two incompatible run-time polymorphism paradigms, which would hurt re-usability. It is also similar in nature to [Trait based inheritance](https://github.com/rust-lang/rfcs/pull/223).

Indeed, this RFC designs two disjoint polymorphism paths (one for data, one for interfaces) and emphasizes a clean separation of concern between payload (`struct`), behaviour (`trait`) and usage (`Box<Trait>` or `SomeThinPointer<Trait>`).

It manages to do so with (relatively) minimal compiler support, thus opening the door to other library schemes.


# Detailed Design

This RFC is rather long, as there is a lot to cover, it is presented in two major sections:

 - the Language & Compiler section, introducing the necessary language changes and accompanying compiler changes
 - the Library section, introducing a number of "new" structs, as well as the adaptations that should performed in the existing Rust `core` and `std` modules for better integration.

> Note: The lack of support for Custom DST greatly affects the Library section and leads to a number of Unresolved Questions.

> Note: As usual, all names are subject to discussion.


# Detailed Design [Language & Compiler]

This section of the RFC presents the changes in the language and compiler that will provide the necessary building bricks for developers to implement polymorphism. Those bricks will be used in the Library section afterwards.

In keeping with Rust tradition, this RFC preserves the orthogonality of defining data structures (in `struct`) and defining interfaces (in `trait`). By doing so, it maximizes the opportunity to mix and match "object-like" polymorphism and trait polymorphism.


## `rtti` module

A new `core::rtti` module is introduced to collect all run-time information; the various sections of this RFC will add their items in this module.

As part of the `std` facade, this module is re-exported as `std::rtti`.

### Cast traits

The following traits `rtti::{UpCast,UpCastRef,DownCast,DownCastRef,Cast}` are introduced:

```rust
pub trait UpCast<Target> {
    fn up_cast(self) -> Target;
}

pub trait UpCastRef<Target> {
    fn up_cast_ref(&self) -> &Target;
    fn up_cast_ref_mut(&mut self) -> &mut Target;
}

pub trait DownCast<Target> {
    fn down_cast(self) -> Result<Target, Self>;

    unsafe fn unchecked_down_cast(self) -> Target;
}

pub trait DownCastRef<Target> {
    fn down_cast_ref(&self) -> Option<&Target>;
    fn down_cast_ref_mut(&mut self) -> Option<&mut Target>;

    unsafe fn unchecked_down_cast_ref(&self) -> &Target;
    unsafe fn unchecked_down_cast_ref_mut(&mut self) -> &mut Target;
}

pub trait Cast<Target> {
    fn cast(self) -> Result<Target, Self>;

    unsafe fn unchecked_cast(self) -> Target;
}
```

While the `[Down|Up]Cast[|Ref]` traits are intended to cast between related traits/structs, the `Cast` trait is intended for general purpose casts, such as cross-casts.

> Alternative: It is unclear whether the `*CastRef` variants are that useful, this is discussed in the Alternatives section.

> Unresolved Question: Could `DownCastRef` and `UpCastRef` be supplanted by implementing the regular `DownCast`/`UpCast` on references instead?


## Struct Polymorphism

Struct Polymorphism aims at allowing a partial view of the fields inside a larger `struct`.

### Syntax

Grammar:

```rust
struct ChildName: ParentName (+ ParentName)* {
    // other attributes
}
```

Example:

```rust
struct FirstParent;

struct SecondParent {
    a: int
}

impl SecondParent() {
    fn increment(&mut self) { self.a += 1; }
}

struct Child: FirstParent + pub SecondParent {
    // other attributes
}

fn usage(child: &mut Child) {
    SecondParent::increment(child as &mut SecondParent);
    (child as &mut SecondParent).increment();
    println!("{}", (child as &SecondParent).a);
}
```

This RFC proposes to reuse the trait notation for extension, and use a similar behaviour for both derivations.

`ParentName (+ ParentName)*` is called the list of parents of the structure:

 - it is an error for any `ParentName` to resolve to `ChildName`
 - it is an error for the same `struct` to appear multiple times in the list of parents
 - any `struct` appearing in the list of parents of `ChildName` is called a direct parent of `ChildName`

> Alternative: the use of extension (instead of attributes) is discussed in the Alternatives section.

> Alternative: the ability to extend from multiple bases, which introduced the parent ambiguity problem, is discussed in the Alternatives section.

### Semantics

By extending another `struct`, the derived `struct` embeds its parents' fields. However, due to encapsulation, it can only access fields that it could access if the parent was an attribute.

In essence, the previous example could be rewritten:

```rust
struct Child {
    pub _super_first: FirstParent,      // 0 bytes, but mentioned anyway
    pub _super_second: SecondParent,
    // other attributes
}

fn usage(child: &mut Child) {
    SecondParent::increment(&mut d._super_second);
    SecondParent::increment(&mut d._super_second);
    println!("{}", child._super_second.a);
}
```

This RFC does not introduce any auto-dereferencing; the developer should feel free to implement `Deref`.

This RFC does not introduce any way to make the extended struct private. Indeed, as it relies on traits to expose the relationship and trait implementations cannot be private, this would not make sense.

> Goal Reached: sharing of fields ~~and methods~~ between definitions

> Goal Reached: syntactically lightweight ~~or implicit upcasting~~

### `ExtendStruct`, `FirstExtendStruct`

In order to query, from the language, the relationship between two `struct`, we introduce some traits `intrinsics::{ExtendStruct, FirstExtendStruct}` and a short-hand notation. These traits are `unsafe`, and automatically implemented by the compiler.

```rust
unsafe trait ExtendStruct<P> { fn offsets() -> &'static [isize]; }
unsafe trait FirstExtendStruct<P> {}
```

The compiler automatically implements the traits according to the following rules:

 - `Child` implements all traits for itself, `offsets` returns `&[0]`.
 - `Child` implements `ExtendStruct<()>`.
 - `Child` implements `ExtendStruct<P>` if `P` appears in the list of extended structs of `Child`.
 - `Child` implements `ExtendStruct<P>` if there exists a `T` such that `Child: ExtendStruct<T>` and `T: ExtendStruct<P>`; due to parent ambiguity multiple such `T` may exist.
 - `<C as ExtendStruct<P>>::offsets()` returns the sorted list of offsets denoting `P` instances in a `C` instance. The offsets are expressed in bytes.
 - `Child` implements `FirstExtendStruct<P>` if `Child: ExtendStruct<P>` and the result of `<C as ExtendStruct<P>>::offsets()` is `&[0]`.

The short-hand notation `C: P` expands to `C: ExtendStruct<P>`.

> Alternative: the short-hand notation is discussed in the Alternatives section.

> Alternative: if parent ambiguity is forbidden, `fn offsets() -> &'static isize` can become `fn offset() -> isize`, this is discussed in the Alternatives section.

### Cast

The following default implementations of `UpCastRef` are provided:

```rust
impl<C, P> UpCastRef<P> for C
    where C: FirstExtend<P>
{
    fn up_cast_ref(&self) -> &P;
    fn up_cast_ref_mut(&mut self) -> &mut P;
}
```

> Note: it is not possible to provide an implementation of `DownCastRef` because doing so requires run-time type information, which a `struct` does not contain by default.

> Note: it is not possible to implement `UpCastRef` for any `(C, P)` such that `C: Extend<P>` because of the parent ambiguitye problem (several `P` could be reachable); should parent ambiguity be banned, then it would become possible.

### Lightweight up-cast

The `as` keyword is used as a lightweight syntax for up-casting, it desugars so:

 - with `c: &C`, `c as &P` desugars into `<C as UpCastRef<P>>::up_cast_ref(c)`
 - with `c: &mut C`, `c as &mut P` desugars into `<C as UpCastRef<P>>::up_cast_ref_mut(c)`

> Goal Reached: syntactically lightweight or implicit upcasting

> Alternative: the `as` short-hand is discussed in the Alternatives section.

### `StructId` and `struct_id`

The `raw::StructId` struct and the `intrinsics::struct_id` function are introduced:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StructId { id: u64 }

pub extern "rust-intrinsic" fn struct_id<S>() -> raw::StructId
    where S: marker::Reflect + 'static;
```

To each `'static` struct is associated a unique `StructId`, which can be obtained via the `struct_id` intrinsic.

> Alternative: the `'static` requirement is discussed in the Alternatives section.

### `StructInfo` and `struct_info`

The `raw::StructInfo` struct and the `intrinsics::struct_info` function are introduced:

```rust
pub extern "rust-intrinsic" fn struct_info<S>() -> &'static raw::StructInfo
    where S: marker::Reflect + 'static;

#[repr(C)]
pub struct StructInfo {
    size_align: u64,        // high 8 bits: log2(align), low 56 bits: size
    struct_id: StructId,
    v_table_getter: fn (TraitId) -> Option<&'static VTable>,
    offsets_getter: fn (StructId) -> &'static [isize],
    //  FIXME: the Clone trait is not for now amenable to cloning in raw storage.
    //  cloner: Option<fn (&mut (), *mut u8) -> ()>,
    dropper: fn (&mut ()) -> (),
}

//  Methods can be seen on the rust-poly repository.
```

This structure provides the necessary information to perform some up-casts and down-casts, as well as manipulating structures without compile-time type information.

For each monomorphized `struct`, the compiler will emit a `StructInfo` in read-only memory in the crate the `struct` is instantiated and wire up the `struct_info` intrinsic to return a pointer to it.

 - the size and alignment of the `struct` are stored to (later) allow DST implementation.
 - the ID of the `struct` is used to know the type of the object at run-time.
 - the `v_table_getter` function provides the `VTable` for all knowns traits this `struct` implements (at the point `StructInfo` is instantiated).
 - the `offsets_getter` function is the type-erased version of `ExtendStruct<P>::offsets`, for all `P` from which the current `struct` extends, and otherwise returns an empty array.
 - the `cloner` function should (later) provide a way to clone a type-erased type.
 - the `dropper` function is the type-erased version of `drop`, its argument is a pointer to the first byte of the `struct`.

As mentioned, this type should represent a "view" of read-only memory laid out by the compiler.

> Note: a rational on the sufficiency of `v_table_getter` is provided in the `TraitInfo` section.

> Unresolved question: How to provide cloning?

### Common Ancestor `()`

As mentioned in the Semantics section, the unit type (0-length tuple) is a common ancestor to all `struct`. As such, for any `struct` `C`, `C: Extend<()>`.

> Alternative: using a common ancestor is handy, but forces parent ambiguity resolution in, it is discussed in the Alternatives section.


## Trait Polymorphism

Trait Polymorphism aims at allowing a partial view of the methods inside a larger `trait`.

### Syntax and Semantics

The syntax and semantics of traits are kept as is.

### `trait` bound

The `trait` bound is introduced, making it possible to restrict a given generic type parameter to *only* traits. Doing so does not give access to any method of the trait, however it allows:

 - if `T: trait`, then `S: T` is a valid bound, and requires that `S` implements `T`
 - if `T: trait`, then `let t: &T = mem::transmate(raw::TraitObject { ... })` is a valid (and unsafe) expression

For backward compatibility reasons, the absence of `trait` bound cannot mean `!trait`, but instead means `?trait`.

### `ExtendTrait`, `FirstExtendTrait`

In order to query, from the language, the relationship between two `trait`, we introduce some traits `intrinsics::{ExtendTrait, FirstExtendTrait}`. Those traits are `unsafe`, and automatically implemented by the compiler.

```rust
unsafe trait ExtendTrait<B>: trait { fn offset() -> isize; }
unsafe trait FirstExtendTrait<B>: ExtendTrait<B> {}
```

The compiler automatically implements the trait according to the following rules:

 - `D` implements `ExtendTrait<D>`, `offset` returns `0`.
 - `D` implements `ExtendTrait<B>` if `D: B`, `offset` returns the necessary in-memory adjustment to point to the right v-table.
 - `D` implements `FirstExtendTrait<B>` if `<D as ExtendTrait<B>>::offset()` returns 0.

> Note: the layout of v-tables is constrained by the presence of `offset`, which requires that the compiler and the `offset` implementation work hand in hand.

### Cast

The following default implementations of `UpCastRef` and `DownCastRef` are provided:

```rust
impl<D, B> UpCastRef<B> for D
    where D: B + trait + 'static,
          B: trait + 'static,
{
    fn up_cast_ref(&self) -> &B;
    fn up_cast_ref_mut(&mut self) -> &mut B;
}

impl<D, B> DownCastRef<D> for B
    where D: B + trait + 'static,
          B: trait + 'static,
{
    fn down_cast_ref(&self) -> Option<&D>;
    fn down_cast_ref_mut(&mut self) -> Option<&mut D>;
}
```

> Unresolved question: Are `DownCastRef` and `UpCastRef` necessary? Could they be supplanted by implementing the regular `DownCast`/`UpCast` on references instead?

### Lightweight up-cast

The `as` keyword is used as a lightweight syntax for up-casting, it desugars so:

 - with `c: &C`, `c as &P` desugars into `<C as UpCastRef<P>>::up_cast_ref(c)`
 - with `c: &mut C`, `c as &mut P` desugars into `<C as UpCastRef<P>>::up_cast_ref_mut(c)`

> Goal Reached: syntactically lightweight or implicit upcasting

> Alternative: the `as` short-hand is discussed in the Alternatives section.

### `TraitId` and `trait_id`

The `raw::TraitId` struct and the `intrinsics::trait_id` function are introduced:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TraitId { id: u64 }

pub extern "rust-intrinsic" fn trait_id<T>() -> raw::TraitId
    where T: marker::Reflect + 'static;
```

To each `'static` trait is associated a unique `TraitId`, which can be obtained via the `trait_id` intrinsic.

> Alternative: the `'static` requirement is discussed in the Alternatives section.

### `TraitInfo` and `trait_info`

The `raw::TraitInfo` struct and the `intrinsics::trait_info` function are introduced:

```rust
pub extern "rust-intrinsic" fn trait_info<T>() -> &'static raw::TraitInfo
    where T: trait + marker::Reflect + 'static;

#[repr(C)]
pub struct TraitInfo {
    trait_id: TraitId,
    v_table_getter: fn (StructId) -> Option<&'static VTable>,
}

//  Methods can be seen on the rust-poly repository.
```

This structure provides the necessary information to perform some up-casts and down-casts, as well as manipulating structures without compile-time type information.

For each monomorphized `trait`, the compiler will emit a `TraitInfo` in read-only memory in the crate the `trait` is instantiated and wire up the `trait_info` intrinsic to return a pointer to it.

 - the ID of the `trait` is used to know the type of the object at run-time.
 - the `v_table_getter` function provides the `VTable` for all knowns structs implementing this `trait` (at the point `TraitInfo` is instantiated).

Most languages feature closed-inheritance, such that when the v-table of a class is instantiated the definite list of all implemented interfaces is known. Rust, instead, features open-polymorphism: the set of `trait` a given `struct` implements is only known when creating a fully statically linked executable.

Rust does give some guarantees, however: the coherence rules guarantee that for any two monomorphized `trait T` and `struct S`, `impl T for S` may only appear in either the crate of `T` (or one of its dependencies) or the crate of `S` (or one of its dependencies). This RFC proposes to piggy-back on the coherence rules:

 - `StructInfo::v_table_getter`: all known traits at the point of instantiation of the monomorphized `struct`
 - `TraitInfo::v_table_getter`: all known structs at the point of instantiation of the monomorphized `trait`

Thus, for any given monomorphized `trait T` and `struct S`, if `S: T`, then:

 - either `(trait_info::<T>().v_table_getter)(struct_id::<S>()).is_some()`
 - or `(struct_info::<S>().v_table_getter)(trait_id::<T>()).is_some()`

and should `T` and `S` be statically known, both computation should be resolvable at compile-time.

> Note: it is unclear yet which should resolve faster than the other.

> Goal Reached: cheap down-casting (using `v_table_getter`), although maybe slightly less cheap than in C++.

### `VTable` and `v_table`

The `raw::VTable` struct and the `intrinsics::v_table` function are introduced:

```rust
pub extern "rust-intrinsic" fn v_table<T, S>() -> &'static rtti::VTable
    where T: trait + marker::Reflect + 'static,
          S: T + marker::Reflect + 'static;

#[repr(C)]
pub struct VTable {
    pub struct_info: &'static StructInfo,
    pub trait_info: &'static TraitInfo,
    // In memory, follows a list of pointers to methods and other amenities
}
```

For each monomorphized `impl`, the compiler will emit a `VTable` in read-only memory in the crate the `impl` is instantiated and wire up the `v_table` intrinsic to return a pointer to it.

This structure's size depend on the `trait` implemented

> Goal Reached: cheap field access from internal methods (monomorphized)

> Goal Reached: cheap dynamic dispatch of methods (fixed offset from the start of `VTable`)

> Alternative: some functions (`dropper`, `cloner`, ...) could be lifted in `VTable`, this is discussed in the Alternatives section.

> Alternative: a typed, sized, version of `VTable` (`TypedVTable<T>`) could be provided to the user, this is discussed in the Alternatives section.

### `TraitObject` and Casts

The existing `raw::TraitObject` should be modified so that its `vtable` member may be safely transmuted to `&'static VTable`.

Then with `&T` relying on `VTable`, `&T` and `&mut T` can be cast at run-time:

```rust
impl<'a, T, B> UpCast<&'a B> for &'a T
    where T: B + trait + marker::Reflect + 'static,
          B: trait + marker::Reflect + 'static;

impl<'a, T, D> DownCast<&'a D> for &'a T
    where T: trait + marker::Reflect + 'static,
          D: T + trait + marker::Reflect + 'static;

impl<'a, T, X> Cast<&'a X> for &'a T
    where T: trait + marker::Reflect + 'static,
          X: trait + marker::Reflect + 'static;

impl<'a, T, B> UpCast<&'a mut B> for &'a mut T
    where T: B + trait + marker::Reflect + 'static,
          B: trait + marker::Reflect + 'static;

impl<'a, T, D> DownCast<&'a mut D> for &'a mut T
    where T: trait + marker::Reflect + 'static,
          D: T + trait + marker::Reflect + 'static;

impl<'a, T, X> Cast<&'a mut X> for &'a mut T
    where T: trait + marker::Reflect + 'static,
          X: trait + marker::Reflect + 'static;
```



# Detailed Design [Library]

This section of the RFC presents additional library elements that could be added to provide users with an out-of-the-box experience.

> Unresolved Question: How to provide a safe `Unsized` type in the absence of Custom DST?

> Alternative: the necessity for those types is discussed in the Alternatives section.

## Thin Pointers/References

Thin Pointers require bundling together the pointer to run-time information and the data of a particular instance of a `struct`. This RFC introduces some types to deal with those.

### `Class` and `DynClass` (and `Dyn`)

The `rtti::{Class,DynClass,Dyn}` types are introduced, their particular implementations are only sketched.

```rust
#[repr(...)] // C ?
struct Class<T, S>
    where T: trait,
          S: T,
{
    // Exposure only
    v_ptr: &'static VTable,
    offset: isize,
    data: S,
}

//  FIXME: this type should be !Sized, this requires negative bounds
//  FIXME: this type would benefit from the availability of support for custom DST
#[repr(...)] // C ?
struct DynClass<T, S>
    where T: trait,
{
    // Exposure only
    v_ptr: &'static VTable,
    offset: isize,
}

//  
type Dyn<T> = DynClass<T, ()>;
```

Some details:

 - It is intended for `DynClass` to be an dynamically sized prefix of `Class`.
 - `v_ptr` is the traditional pointer to the v-table, it is initialized to `v_table::<T0, S0>()` when `Class<T0, S0>` is first created; it always points to v-tables of `S0` throughout the lifetime of the instance, but jumps from `trait` to `trait` following the casts (of `DynClass`).
 - `offset` represents, at first, the `offset` of `data` in `Class<T0, S0>`, it is necessary to support `S0` types for which `mem::align_of::<S0>() > mem::align_of::<isize>()`; it jumps from `struct` to `struct` (always within the original confines of `Class<T0, S0>`) following the casts (of `DynClass`)
 - `data` is simply the piece of data.

Invariants:

 - for any `DynClass<T, S>`, `self.v_ptr` points to a `VTable` for `trait T` (and the original `struct`)
 - for any `DynClass<T, S>`, `(&self as *const u8) + self.offset` points to an instance of `S`

as a result of those invariants, building a `&T` or a `&S` is cheap.

> Goal Reached: thin pointers (`Box<DynClass<T, S>>`).

> Goal Reached: safe, i.e., doesn't require a bunch of transmutes or other unsafe code to be usable.

> Note: since `offset` is necessary for large alignment support, this RFC makes the best of it and handles extending multiple `struct` at once without additional storage costs.

> Note: `DynClass` cannot be directly constructed, instead an implementation of `convert::From` exists to convert from `Box<Class<T, S>>` to `Box<DynClass<T, S>>`.

### Casts

The various `*Cast*` traits are implemented for `Box<DynClass<T, S>>`:

```rust
impl<T, S, B, P> UpCast<Box<DynClass<B, P>>> for Box<DynClass<T, S>>
    where T: B + trait + marker::Reflect + 'static,
          S: P + marker::Reflect + 'static,
          B: trait + marker::Reflect + 'static,
          P: marker::Reflect + 'static;

impl<T, S, B, P> UpCastRef<DynClass<B, P>> for DynClass<T, S>
    where T: FirstExtendTrait<B> + marker::Reflect + 'static,
          S: FirstExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static;

impl<T, S, D, C> DownCast<Box<DynClass<D, C>>> for Box<DynClass<T, S>>
    where T: trait + marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: T + trait + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static;

impl<T, S, D, C> DownCastRef<DynClass<D, C>> for DynClass<T, S>
    where T: trait + marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: FirstExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static;

impl<T, S, X, Y> Cast<Box<DynClass<X, Y>>> for Box<DynClass<T, S>>
    where T: trait + marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: trait + marker::Reflect + 'static,
          Y: marker::Reflect + 'static;
```

A prototype implementation can be seen on rust-poly; an excerpt is presented here:

```rust
    //  up-cast from trait T to trait B
    let v_table: &'static VTable = unsafe {
        let raw: *const u8 = mem::transmute(self.v_table);
        mem::transmute(raw as isize + <T as TraitExtendTrait<B>>::offset())
    };
```

which illustrates the support of efficient up-casts, as the result of `<T as TraitExtendTrait<B>>::offset()` can be computed at compile-time (providing `offset` is inlined).

> Note: the extra restrictions imposed on `UpCastRef` and `DownCastRef` apply any time the content pointed to is immutable; for example, it would apply to `Rc<DynClass<T, S>>`; this cannot be worked around by using `Rc<RefCell<DynClass<T, S>>>` (or any other run-time mutability enabler) as the `*Cast` traits consume their inputs.


## Fat Pointers/References

Why?

 - `&T` does not provide direct access to fields
 - `&DynClass<T, S>` does not provide access to *other* fields

### `DynRef` and `DynRefMut`

The `rtti::{DynRef,DynRefMut}` types are introduced, their particular implementations are only sketched.

```rust
pub struct DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    // Exposure only
    v_ptr: &'static VTable,
    offset: isize,
    v_data: &mut (),
}

pub struct DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    // Exposure only
    v_ptr: &'static VTable,
    offset: isize,
    v_data: &mut (),
}
```

Those references can be pointed at any existing storage, borrowing it, and allow navigating the extension hierarchy of `S` safely.


## Implementing the DOM according to requirements

Let us now how an example of a simple DOM would look like given those facilities, as it is the reference example used by the existing RFCs.

```rust
trait Node {}

#[derive(Debug)]
struct NodeData {
    parent: Option<Box<DynClass<Node, NodeData>>>,
    first_child: Option<Box<DynClass<Node, NodeData>>>,
}

impl Node for NodeData {}

#[derive(Debug)]
struct TextNode: NodeData {}

impl Node for TextNode {}


trait Element: Node {
    fn do_the_thing(&self);

    fn before_set_attr(&mut self, _key: &str, _val: &str) {}
    fn after_set_attr(&mut self, _key: &str, _val: &str) {}
}

#[derive(Debug)]
struct ElementData: NodeData {
    attrs: HashMap<String, String>,
}

// Note: private access to ElementData::data, ensuring invariants;
//       also, this method is always statically dispatched and thus inlinable.
impl ElementData {
    fn set_attribute(&mut self, key: &str, value: &str) {
        self.before_set_attr(key, value);
        self.attrs.insert(key.to_string(), value.to_string());
        self.after_set_attr(key, value);
    }
}

impl Node for ElementData {}

impl Element for ElementData {
    fn do_the_thing(&self) { println!("ElementData is in da place!"); }
}

#[derive(Debug)]
struct HTMLImageElement: ElementData {}

impl Node for HTMLImageElement {}

impl Element for HTMLImageElement {
    fn do_the_thing(&self) { println!("HTMLImageElement is in da place!"); }

    fn before_set_attr(&mut self, key: &str, val: &str) {
        if key == "src" {
            // remove cached image
        }
        <ElementData as Element>::before_set_attr(&mut self as &mut ElementData, key, val);
    }
}

#[derive(Debug)]
struct HTMLVideoElement: ElementData {
    cross_origin: bool,
}

impl Node for HTMLVideoElement {}

impl Element for HTMLVideoElement {
    fn do_the_thing(&self) { println!("HTMLVideoElement is in da place!"); }

    fn after_set_attr(&mut self, key: &str, value: &str) {
        if key == "crossOrigin" {
            self.cross_origin = value == "true";
        }
        <ElementData as Element>::after_set_attr(&mut self as &mut ElementData, key, value);
    }
}

fn process_any_element<'a>(element: &'a Element) {
    element.do_the_thing();
}

pub fn doit() {
    type Node = DynClass<Node, NodeData>>;
    type Text = DynClass<Node, TextNode>;
    type Element = DynClass<Element, ElementData>;

    let text_node: Box<Node> = {
        let nd = NodeData { parent: None, first_child: None };
        Box::new(Class::new(TextNode { NodeData: nd })).into().up_cast()
    };

    let video_element: Box<Element> = {
        let nd = NodeData { parent: None, first_child: Some(text_node) };
        let ed = ElementData { NodeData: nd, attrs: HashMap::new() };
        let hve = HTMLVideoElement { ElementData: ed, cross_origin: false };
        Box::new(Class::new(hve)).into().up_cast()
    };

    process_any_element((*video_element).as_trait());

    let node = (video_element.as_struct() as &NodeData).first_child.as_ref().unwrap();

    if let Some(text) = { let t: Option<&Text> = (*node).down_cast_ref(); t } {
        println!("I got me some text node {:?}", &text);
    } else if let Some(element) = { let t: Option<&Box<Element>> = (*node).down_cast_ref(); t } {
        println!("I got me some element {:?}", &element);
    } else {
        println!("Oh shoot, nothing I know!");
    }
}
```

> Note: It appears that support for "in-flight" type ascriptions is lacking today, leading to those ugly "if let" expressions.


## Goal Check

Starting by a freebie:

> Goal Reached: calling functions through smartpointers, e.g. fn foo(JSRef<T>, ...), if said smart pointers implement `Deref<Target = T>` for the desired trait.

The goals check list:

 - cheap field access from internal methods:
    - the code for a given `struct` or `trait` is monomorphized.
 - cheap dynamic dispatch of methods:
    - the method is at a compile-time known offset from in the v-table current being pointed, thus the cost is identical to calling a method through a `&T` where `T: trait`.
 - cheap down-casting:
    - down-casting at an arbitrary point in an open hierarchy is a complicated operation, achieved here with at most 2 function calls (one statically known, with a run-time argument, the other dynamically known, with a compile-time argument); cheap given the functionality.
 - thin pointers:
    - `Box<DynClass<..>>` is a thin pointer, but is unavailable without custom DST support.
 - sharing of fields and methods between definitions:
    - fields are shared through `struct` extension.
    - methods... see "Unresolved Question"
 - safe, i.e., doesn't require a bunch of transmutes or other unsafe code to be usable:
    - the library types `Class` and `DynClass` can be used safely.
 - syntactically lightweight or implicit upcasting:
    - a single method call (when inference works) or the "as" keyword.
 - calling functions through smartpointers, e.g. fn foo(JSRef<T>, ...):
    - as long as said pointers implement the appropriate `Deref`
 - static dispatch of methods:
    - as today.

> Unresolved Question: What does "sharing methods between definitions" mean, exactly?


## Wrapping up

This RFC proposes a polymorphism which integrates smoothly with the existing language today: this is evidenced by the fact that `&T` gains polymorphism, for example.

By fusing the necessary facilities for up-casting, down-casting and cross-casting with the existing facilities, this RFC avoids duplicating the functionalities and augments the existing language without introducing a shift, thus guaranteeing the interoperability of code which makes use of its library facilities and code which does not.

Yet, despite being lightweight and rusty, it is quite possible to translate traditional objects hierarchy with a one-to-one mapping, use type aliases to mask the novelty and type inference to avoid ever mentioning it directly.



# Drawbacks

 - **No custom DST yet**: in the absence of support for custom DST types, thin-pointers can only exist in the compiler (today); this RFC does not seek to address the issue, and rather than introducing more special cases into the compiler (for a built-in `DynClass`), the suggestion offered is to revise this proposal after some minimal custom DST support has landed.
 - **heavier syntax** (`&T` vs `&Dyn<T>`): it is expected that the need for such bundling be rare, and it is possible to convert to `&T` immediately (so that only `struct` code and not `fn` code be affected), thus this is seen as a minor drawback.
 - **performance untested**: the performance can only be assessed with the proper compiler intrinsics in place; complexity analysis looks good, but it only goes so far.
 - **opinionated**: some building blocks are created, though maybe not as generic as one would wish. It seems unlikely that building blocks catering to *any* case would emerge, though.



# Alternatives


## `*CastRef`

It is unclear whether those variants are really useful, or whether they could be supplanted by implementing the `*Cast` traits on reference types instead.


## Extension syntax

The use of the extension syntax for `struct` immediately introduce a number of specific constructs:

 - the extension syntax itself,
 - the initialization syntax is changed to be able to initialized the extended structs,
 - ...

Gereeter (co-author of the [#11](https://github.com/rust-lang/rfcs/pull/11)), instead proposes the use of an attribute such as `#[parent]` (for example) placed on a regular data member.

Despite the ease of introducing such an attribute, there seems to be a number of downsides. Support would seem lack-luster, compared to the first-class support of `trait` extension, for example, and it would seem bizarre that even though a sub-typing relationship is introduced as for `trait`, the syntax is completely different.

It is unclear whether first-class syntax support for this feature is really desired, though, as it could be an edge case.


## Short-hand notation `:`

The short-hand notation for `syntax` extension (or even, `trait` extension) is not necessary; however it greatly simplifies writing bounds, in a way that current Rustaceans have already internalized.

Still, in the presence of the intrinsic traits, it is redundant.


## Short-hand notation `as`

The short-hand notation for up-casting is not necessary, however `as` is greatly simplifies writing the up-casts, in a way that current Rustaceans have already internalized.

Still, in the presence of the `UpCast*` traits, it is redundant.


## Multiple parents

The syntax and compiler intrinsics proposed support a `struct` having multiple parents and indeed having multiple times the same parents (through various branches).

It would be possible to restrict `struct` extension to a single parent. This would obviate the need for `ExtendStruct::offsets`, and other bits and nits, at the cost of restricting the user.

It would instead be possible to allow `struct` extension to multiple parents, but forbidding parent ambiguity, that is having `P` reachable multiple times via different branches of the parents hierarchy. This would simplify the interface of `ExtendStruct::offset` (which would always return a single offset), and the restriction might not be too onerous on the user.

And finally, full-support for parent ambiguity is also possible, in multiple ways:

 - `ExtendStruct` could be restricted to unambiguous cases; forcing the user to specify the path when casting. In this case, `offset` can be simplified as above.
 - a supplementary `UnambiguouslyExtendStruct` could be introduced for this purpose.
 - or thing remain as-is, and a run-time decision need be taken, such as using the first instance (when up-casting), bailing out (Option) or panicking.

> Note: the fate of the proposed common ancestor `()` hangs in the balance here as the ability to cast to `()` requires support for parent ambiguity. Without a common ancestor, `Dyn<T>` requires a full-blown implementation rather than being a mere alias.


## `'static` lifetime

Today, in `Any`, the `'static` lifetime is required because `Any` erases the lifetime.

This seems arbitrarily restrictive, and it seems that:

 - it is unnecessary for `trait_id` and `struct_id` (as it is for `type_id` today) as long as one accepts that traits/structs varying only by their lifetime (a pure compile-time artefact) would obtain identical IDs.
 - the safety could instead be enforced at library level, `Class` and `DynClass` could for example be extended to take a lifetime, and up-casts/down-casts would only be allowed to *narrow* the lifetime (yes, narrowing whatever the direction of the cast)

It could be worth future-proofing the API (introducing an extraneous lifetime in `Class` and `Dyn*` but with a `'static` bound for now), to avoid API changes, but may not be worth introducing it from the get-go: there is already much to deal with.


## Layout of `VTable`

The layout of `VTable` as proposed only takes into account fast up-casting and fast dynamic method dispatching. The down-casting path is slower, as is a call to `dropper` or (in the future) `cloner`.

Other layouts are possible:

 - not duplicating the v-table headers in-between the methods list when duplicating the list: this prevents the existing of `ExtendTrait::offset` and requires run-time look-up of the v-table.
 - increasing the v-table header, by moving / duplicating information from `StructInfo` or `TraitInfo`, in a typical attempt to speed things up (one less dereference) by using more memory.

The current layout was proposed as a normalized version of what the v-table could look like; whilst still remaining efficient. Benchmarks are necessary to effectively measure the effect of the various options on the performance of different usecases (method call, drop/clone call, casting).


## Sized, Typed, `VTable`

A dedicated (compiler-generated) `TypedVTable<T>` could be generated for each trait, allowing a user to navigate the v-table in library code (even sub-tables and methods).

This RFC proposes NOT to introduce such a type for now, as it can be added in a backward compatible fashion.


## Library section

The entire library section could be tabled, or an entirely different version of the library could be proposed, without affecting much the value of the Language & Compiler section.

Notably, even without the library section, one would gain casts on existing `&T` and `&mut T` references.


## Comparison to existing RFCs

There are many other RFCs, as already mentioned:

 - [#9](https://github.com/rust-lang/rfcs/pull/9): Fat Objects
 - [#11](https://github.com/rust-lang/rfcs/pull/11): Extending Enums
 - [#223](https://github.com/rust-lang/rfcs/pull/223): Trait Based Inheritance
 - [#250](https://github.com/rust-lang/rfcs/pull/250): Associated Field Inheritance

This RFC emphasizes *flexibility* and a clean separation of concern between payload (`struct`), behaviour (`trait`) and usage (`&T` or `&Dyn<T>`). The same `struct` or `trait` can freely be shared in situations where thin pointers are desirable and in situations they are not.

This RFC can be seen as a refined version of Fat Objects (#9), proposing a more fully fleshed out implementation and simplifying the implementation of non-virtual methods by simply adding them to the `struct` rather than creating an extraneous `trait`. It was also pointed by gereeter (author of Trait Based Inheritance (#223)) that there was some overlap.

Compared to `...` (#11), this RFC does not require distinguishing between `enum` that can be extended and `enum` that cannot (mixes payload and usage). This distinction already exists today in C++ (inheriting a class without a `virtual` destructor) and has proven to be a pain point of the language; it introduces a split in the language ecosystem between those `struct` that can be extended and those that cannot. On the contrary, this RFC emphasizes that every existing `trait` and `struct` can be reused, and no foresight is necessary when designing new ones. It is somewhat less ambitious, as it does not attempt any large scale changes to the language beyond fulfilling the given requirements, but at the same time implements up-casting and down-casting for existing traits.

Compared to associated fields (#250), this RFC does not inject data in traits (mixes payload and behaviour). It neatly sidesteps the issue of splitting the ecosystem into stateful traits and stateless traits, and therefore guarantees that traits can be shared between any library, in any direction.

Compared to the associated fields (#250), this RFC's approach to fields is both cheaper than the indirect fields approach (with its required offset in v-table per field) and less constrained than the `#[repr(fixed)]` approach (which precludes implementing two fixed traits with contradicting requirements). It also does not require the compiler to try and guarantee the non-aliasing of fields. On the other hand, it is obviously less flexible given its conservative choice (no renaming/re-arrangement).

Compared to the associated fields (#250), this RFC's approach does not require that common fields be public, which is a violation of encapsulation. The `struct` can define methods with exclusive access to its fields, guaranteeing the invariants of its choice, and because those methods are not polymorphic they can be easily inlined. Still, if desired, its fields *can* be public. Note that this RFC does require that the Parent/Child relationship be public, as it is exposed via traits.

Compared to the Internal Vtable (#250), this RFC once again avoids enforcing that a `struct` or `trait` only be usable in a particular way (mixes payload and usage). This allows using either the `struct` or `trait` in other contexts, where this particular representation would be less attractive (it is known that LLVM has issues with devirtualizing calls through internal v-pointers, for example).


# Unresolved Questions

 - Could `DownCastRef` and `UpCastRef` be supplanted by implementing the regular `DownCast`/`UpCast` on references instead?
 - How to provide cloning? Beyond `Clone` not being object-safe today, it also does not work with a raw memory area, furthermore, in the absence of negative bounds, it seems impossible to implement a function (or set of) returning a type-erased `Option<ClonerFn>`.
 - How to provide a safe `?Sized` type in the absence of Custom DST? Is it even possible?
 - What does "sharing methods between definitions" mean, exactly?
 
