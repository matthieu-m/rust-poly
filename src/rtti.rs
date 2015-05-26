//
//  A tentative implementation of RFC: Disjoint Polymorphism
//
#![allow(dead_code)]
#![allow(drop_with_repr_extern)]

use alloc::boxed::Box;
use core::clone;
use core::convert;
use core::fmt;
use core::intrinsics;
use core::marker;
use core::mem;
use core::ops;
use core::raw;

// KLUDGE
use std;

//
//  Helpers
//
//  Humpf, "offsetof" is a reserved identifier but does not do anything :(
#[macro_export]
macro_rules! offset_of(
    ($T:ty, $field:ident) => {
        unsafe {
            let exemplar: $T = std::mem::uninitialized();
            let base: *const u8 = std::mem::transmute(&exemplar);
            let attr: *const u8 = std::mem::transmute(&exemplar.$field);
            mem::forget(exemplar);
            (attr as isize) - (base as isize)
        }
    }
);

//
//  "Manual" intrinsics
//
//  Some functionality require data set-up for us by the compiler.
//  This is a pure Rust program though, so instead we will rely on
//  KLUDGEs, specifically, an emulated set of type-infos and v-tables.
//

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StructId { id: u64 }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TraitId { id: u64 }

pub fn struct_id<Struct>() -> StructId
    where Struct: marker::Reflect + 'static
{
    StructId { id: unsafe { mem::transmute(intrinsics::type_id::<Struct>()) } }
}

pub fn trait_id<Trait: ?Sized>() -> TraitId
    where Trait: marker::Reflect + 'static
{
    TraitId { id: unsafe { mem::transmute(intrinsics::type_id::<Trait>()) } }
}

// KLUDGE
macro_rules! make_vptr(
    ($T:ty, $S:ty) => (
        {
            let s: &$S = unsafe { ::std::mem::uninitialized() };
            let t: &$T = s;
            let r: ::std::raw::TraitObject = unsafe { ::std::mem::transmute(t) };
            r.vtable
        }
    )
);

// KLUDGE
#[macro_export]
macro_rules! make_vtable_entry(
    ($T:ty, $S:ty) => (
        {
            (
                $crate::rtti::trait_id::<$T>(),
                $crate::rtti::struct_id::<$S>(),
                $crate::rtti::VTable::new::<$T, $S>(
                    make_vptr!($T, $S)
                )
            )
        }
    )
);

// KLUDGE
struct StructInfoRegistry {
    inner: std::sync::Arc<Vec<(StructId, StructInfo)>>,
}

// KLUDGE
struct TraitInfoRegistry {
    inner: std::sync::Arc<Vec<(TraitId, TraitInfo)>>,
}

// KLUDGE
struct VTableRegistry {
    inner: std::sync::Arc<Vec<(TraitId, StructId, VTable)>>,
}

// KLUDGE
static mut STRUCT_INFO_REGISTRY: *const StructInfoRegistry = 0 as *const StructInfoRegistry;
static mut TRAIT_INFO_REGISTRY: *const TraitInfoRegistry = 0 as *const TraitInfoRegistry;
static mut VTABLE_REGISTRY: *const VTableRegistry = 0 as *const VTableRegistry;

// KLUDGE
pub fn init_struct_info_registry(registry: Vec<(StructId, StructInfo)>) {
    static ONCE: std::sync::Once = std::sync::ONCE_INIT;
    unsafe {
        ONCE.call_once(|| {
            let registry = StructInfoRegistry { inner: std::sync::Arc::new(registry) };
            STRUCT_INFO_REGISTRY = mem::transmute(Box::new(registry));
        });
    }
}

// KLUDGE
pub fn init_trait_info_registry(registry: Vec<(TraitId, TraitInfo)>) {
    static ONCE: std::sync::Once = std::sync::ONCE_INIT;
    unsafe {
        ONCE.call_once(|| {
            let registry = TraitInfoRegistry { inner: std::sync::Arc::new(registry) };
            TRAIT_INFO_REGISTRY = mem::transmute(Box::new(registry));
        });
    }
}

// KLUDGE
pub fn init_vtable_registry(registry: Vec<(TraitId, StructId, VTable)>) {
    static ONCE: std::sync::Once = std::sync::ONCE_INIT;
    unsafe {
        ONCE.call_once(|| {
            let registry = VTableRegistry { inner: std::sync::Arc::new(registry) };
            VTABLE_REGISTRY = mem::transmute(Box::new(registry));
        });
    }
}

pub fn struct_info<Struct>() -> &'static StructInfo
    where Struct: marker::Reflect + 'static
{
    let struct_id = struct_id::<Struct>();
    struct_info_by_id(struct_id)
}

pub fn struct_info_by_id(struct_id: StructId) -> &'static StructInfo {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if STRUCT_INFO_REGISTRY.is_null() {
            panic!("Call init_struct_info_registry before the first call to struct_info.")
        }

        for &(s_id, ref struct_info) in &*(*STRUCT_INFO_REGISTRY).inner {
            if s_id == struct_id { return &struct_info; }
        }

        panic!("No such struct_info registered.");
    }
}

pub fn trait_info<Trait: ?Sized>() -> &'static TraitInfo
    where Trait: marker::Reflect + 'static
{
    let trait_id = trait_id::<Trait>();
    trait_info_by_id(trait_id)
}

pub fn trait_info_by_id(trait_id: TraitId) -> &'static TraitInfo {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if TRAIT_INFO_REGISTRY.is_null() {
            panic!("Call init_trait_info_registry before the first call to trait_info.")
        }

        for &(t_id, ref trait_info) in &*(*TRAIT_INFO_REGISTRY).inner {
            if t_id == trait_id { return &trait_info; }
        }

        panic!("No such trait_info registered.");
    }
}

pub fn v_table<Trait: ?Sized, Struct>() -> &'static VTable
    where Trait: marker::Reflect + 'static,
          Struct: ExtendTrait<Trait> + marker::Reflect + 'static
{
    let trait_id = trait_id::<Trait>();
    let struct_id = struct_id::<Struct>();
    v_table_by_id(trait_id, struct_id).unwrap()
}

pub fn v_table_by_id(trait_id: TraitId, struct_id: StructId) -> Option<&'static VTable> {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if VTABLE_REGISTRY.is_null() {
            panic!("Call init_vtable_registry before the first call to v_table.")
        }

        for &(t_id, s_id, ref vtable) in &*(*VTABLE_REGISTRY).inner {
            if t_id == trait_id && s_id == struct_id { return Some(vtable); }
        }

        None
    }
}


//
//  "Manual" marker traits
//
//  Those intrinsics should be automatically implemented by the compiler, based on the traits and types properties.
//
pub unsafe trait ExtendTrait<T: ?Sized> {}
pub unsafe trait ExtendStruct<T> {}
pub unsafe trait FirstExtendTrait<T: ?Sized>: ExtendTrait<T> {}
pub unsafe trait FirstExtendStruct<T>: ExtendStruct<T> {}


//
//  Raw representation of type info data in ROM.
//
#[repr(C)]
pub struct StructInfo {
    size_align: u64,        // high 8 bits: log2(align), low 56 bits: size
    struct_id: StructId,
    v_table_getter: fn (TraitId) -> Option<&'static VTable>,
    offsets_getter: fn (StructId) -> &'static [isize],
    dropper: fn (&mut ()) -> (),
}

#[repr(C)]
pub struct TraitInfo {
    trait_id: TraitId,
    v_table_getter: fn (StructId) -> Option<&'static VTable>,
}

#[repr(C)]
pub struct VTable {
    struct_info: &'static StructInfo,
    trait_info: &'static TraitInfo,
    table: *mut (),         // KLUDGE
}

impl StructInfo {
    const ALIGN_MASK: u64 = 18374686479671623680_u64;
    const ALIGN_SHIFT: u64 = 56;

    pub fn new<S>(
        vt: fn (TraitId) -> Option<&'static VTable>,
        off: fn (StructId) -> &'static [isize],
        drop: fn (&mut ()) -> ()
    ) -> StructInfo
        where S: marker::Reflect + 'static
    {
        fn log2(n: u64) -> u64 {
            let mut n = n;
            let mut acc = 0;
            while n != 1 {
                assert!(n % 2 == 0, "Only works on powers of 2");
                n /= 2;
                acc += 1;
            }
            acc
        }

        let size = mem::size_of::<S>() as u64;
        let align = log2(mem::align_of::<S>() as u64) << StructInfo::ALIGN_SHIFT;
        StructInfo {
            size_align: size | align,
            struct_id: struct_id::<S>(),
            v_table_getter: vt,
            offsets_getter: off,
            dropper: drop,
        }
    }

    pub fn size(&self) -> usize { (self.size_align & !StructInfo::ALIGN_MASK) as usize }

    pub fn log2_align(&self) -> usize { (self.size_align >> StructInfo::ALIGN_SHIFT) as usize }

    pub fn struct_id(&self) -> StructId { self.struct_id }

    pub fn v_table(&self, id: TraitId) -> Option<&'static VTable> {
        (self.v_table_getter)(id)
    }

    pub fn offsets(&self, id: StructId) -> &'static [isize] {
        (self.offsets_getter)(id)
    }

    pub fn drop(&self, data: &mut ()) {
        (self.dropper)(data)
    }
} // impl StructInfo

impl fmt::Debug for StructInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "StructInfo {{ size: {}, align: {}, struct_id: {:?} }}",
            self.size(),
            1_u64 << self.log2_align(),
            self.struct_id
        )
    }
} // impl Debug for StructInfo

impl TraitInfo {
    pub fn new<T: ?Sized>(vt: fn (StructId) -> Option<&'static VTable>) -> TraitInfo
        where T: marker::Reflect + 'static
    {
        TraitInfo {
            trait_id: trait_id::<T>(),
            v_table_getter: vt,
        }
    }

    pub fn trait_id(&self) -> TraitId { self.trait_id }

    pub fn v_table(&self, id: StructId) -> Option<&'static VTable> {
        (self.v_table_getter)(id)
    }
} // impl TraitInfo

impl fmt::Debug for TraitInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "TraitInfo {{ trait_id: {:?} }}",
            self.trait_id
        )
    }
} // impl Debug for TraitInfo

impl VTable {   
    pub fn new<T: ?Sized, S>(table: *mut ()) -> VTable
        where T: marker::Reflect + 'static,
              S: ExtendTrait<T> + marker::Reflect + 'static,
    {
        VTable {
            struct_info: struct_info::<S>(),
            trait_info: trait_info::<T>(),
            table: table,
        }
    }

    pub fn struct_info(&self) -> &'static StructInfo { self.struct_info }

    pub fn trait_info(&self) -> &'static TraitInfo { self.trait_info }

    pub fn table(&self) -> *mut () { self.table }

    pub fn cast_to_trait<T: ?Sized>(&self) -> Option<&'static VTable>
        where T: marker::Reflect + 'static,
    {
        let trait_info = trait_info::<T>();
        let struct_info = self.struct_info;

        if let Some(vt) = trait_info.v_table(struct_info.struct_id) {
            Some(vt)
        } else {
            struct_info.v_table(trait_info.trait_id)
        }
    }
} // impl VTable

impl fmt::Debug for VTable {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VTable {{ struct_info: {:?}, trait_id: {:?} }}",
            self.struct_info,
            self.trait_info
        )
    }
} // impl Debug for VTable


//
//  Library
//

//
//  Cast Traits
//
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

//
//  &T and &mut T
//
//  FIXME: once &T and &mut T are wired into VTable, implement UpCastRef/DownCastRef.
//

//
//  Bricks
//
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UntypedVRef {
    v_table: &'static VTable,
}

#[repr(C)]
pub struct VRef<T: ?Sized>
    where T: marker::Reflect + 'static
{
    untyped: UntypedVRef,
    _0: marker::PhantomData<*const T>,
}

impl UntypedVRef {
    pub fn new(v_table: &'static VTable) -> UntypedVRef {
        UntypedVRef { v_table: v_table }
    }

    pub fn v_table(&self) -> &'static VTable { self.v_table }

    pub fn struct_info(&self) -> &'static StructInfo {
        self.v_table().struct_info
    }

    pub fn trait_info(&self) -> &'static TraitInfo {
        self.v_table().trait_info
    }

    pub fn up_cast<T: ?Sized, B: ?Sized>(&self) -> UntypedVRef
        where B: marker::Reflect + 'static,
              T: ExtendTrait<B> + marker::Reflect + 'static
    {
        UntypedVRef::new(self.v_table().cast_to_trait::<B>().unwrap())
    }

    pub fn down_cast<T: ?Sized, D: ?Sized>(&self) -> Option<UntypedVRef>
        where T: marker::Reflect + 'static,
              D: ExtendTrait<T> + marker::Reflect + 'static
    {
        if trait_id::<T>() == trait_id::<D>() { return Some(*self); }

        self.v_table().cast_to_trait::<D>().map(|vt| {
            UntypedVRef::new(vt)
        })
    }

    pub fn drop(&self, it: &mut ()) {
        self.v_table().struct_info().drop(it)
    }
} // impl UntypedVRef

impl fmt::Debug for UntypedVRef {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "UntypedVRef {{ v_table: {:?} }}",
            self.v_table()
        )
    }
}

impl<T: ?Sized> VRef<T>
    where T: marker::Reflect + 'static
{
    pub fn new<S>() -> VRef<T>
        where S: ExtendTrait<T> + marker::Reflect + 'static
    {
        VRef {
            untyped: UntypedVRef::new(v_table::<T, S>()),
            _0: marker::PhantomData
        }
    }

    pub fn v_table(&self) -> &'static VTable {
        self.untyped.v_table()
    }

    pub fn struct_info(&self) -> &'static StructInfo {
        self.untyped.struct_info()
    }

    pub fn trait_info(&self) -> &'static TraitInfo {
        self.untyped.trait_info()
    }

    pub fn up_cast<B: ?Sized>(&self) -> VRef<B>
        where B: marker::Reflect + 'static,
              T: ExtendTrait<B>
    {
        VRef { untyped: self.untyped.up_cast::<T, B>(), _0: marker::PhantomData }
    }

    pub fn down_cast<D: ?Sized>(&self) -> Option<VRef<D>>
        where D: ExtendTrait<T> + marker::Reflect + 'static
    {
        self.untyped.down_cast::<T, D>().map(|u| {
            VRef { untyped: u, _0: marker::PhantomData }
        })
    }

    pub fn offsets<S>(&self) -> &'static [isize]
        where S: marker::Reflect + 'static
    {
        self.struct_info().offsets(struct_id::<S>())
    }

    pub fn drop(&self, it: &mut ()) {
        self.untyped.drop(it)
    }
} // impl VRef

impl<T: ?Sized> clone::Clone for VRef<T>
    where T: marker::Reflect + 'static
{
    fn clone(&self) -> Self {
        VRef { untyped: self.untyped.clone(), _0: marker::PhantomData }
    }
}

impl<T: ?Sized> marker::Copy for VRef <T>
    where T: marker::Reflect + 'static
{
}

impl<T: ?Sized> fmt::Debug for VRef<T>
    where T: marker::Reflect + 'static
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VRef {{ untyped: {:?} }}",
            self.untyped
        )
    }
}


//
//  Class
//
#[repr(C)]
#[derive(Debug)]
pub struct Class<T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    offset: isize,
    data: S,
}

#[repr(C)]
pub struct DynClass<T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    offset: isize,
    _0: marker::PhantomData<S>,
}

pub type Dyn<T> = DynClass<T, ()>;

//
//  Class & ExtendedClass
//
impl<T: ?Sized, S> Class<T, S>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    pub fn new(data: S) -> Class<T, S> {
        let o = offset_of!(Self, data);

        Class { v_ref: VRef::new::<S>(), offset: o, data: data }
    }
} // impl Class

//  drop_with_repr_extern: drop would be adding hidden state, apparently,
//                         and this layout need to match with that of DynClass
impl<T: ?Sized, S> Drop for Class<T, S>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static
{
    fn drop(&mut self) {}
}

//
//  DynClass
//
impl<T: ?Sized, S> DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    pub fn as_trait(&self) -> &T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table,
            })
        }
    }

    pub fn as_trait_mut(&mut self) -> &mut T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct_mut()),
                vtable: self.v_ref.v_table().table,
            })
        }
    }

    pub fn as_struct(&self) -> &S {
        unsafe { mem::transmute(self.data_ptr()) }
    }

    pub fn as_struct_mut(&mut self) -> &mut S {
        unsafe { mem::transmute(self.data_ptr()) }
    }

    fn data_ptr(&self) -> *mut () {
        unsafe {
            let base: *const u8 = mem::transmute(self);
            mem::transmute(base.offset(self.offset))
        }
    }

    pub fn up_cast_trait<B: ?Sized>(&self) -> VRef<B>
        where T: ExtendTrait<B>,
              B: marker::Reflect + 'static,
    {
        if trait_id::<T>() != trait_id::<B>() {
            self.v_ref.up_cast::<B>()
        } else {
            //  The type-system is not flow sensitive enough
            //  to realize that if T == B, then VRef<T> == VRef<B>.
            unsafe { mem::transmute(self.v_ref) }
        }
    }

    pub fn up_cast_struct<P>(&self) -> isize
        where S: ExtendStruct<P>,
              P: marker::Reflect + 'static,
    {
        if struct_id::<S>() != struct_id::<P>() {
            let offsets = struct_info::<S>().offsets(struct_id::<P>());
            assert!(offsets.len() == 1, "Multiple offsets support not implemented yet");

            self.offset + unsafe { offsets.get_unchecked(0) }
        } else {
            self.offset
        }
    }

    pub fn down_cast_trait<D: ?Sized>(&self) -> Option<VRef<D>>
        where D: ExtendTrait<T> + marker::Reflect + 'static,
    {
        self.v_ref.down_cast::<D>()
    }

    pub fn down_cast_struct<C>(&self) -> Option<isize>
        where C: ExtendStruct<S> + marker::Reflect + 'static,
    {
        if struct_id::<S>() == struct_id::<C>() { return Some(self.offset); }

        let offsets = self.v_ref.offsets::<C>();
        assert!(offsets.len() <= 1, "Support for diamond inheritance is not yet implemented!");

        offsets.first().map(|&o| o)
    }
} // impl DynClass

impl<T: ?Sized, S> fmt::Debug for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: fmt::Debug + marker::Reflect + 'static
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "DynClass {{ v_ref: {:?}, offset: {}, data: {:?} }}",
            self.v_ref,
            self.offset,
            self.as_struct(),
        )
    }
} // impl Debug for DynClass

impl<T: ?Sized, S> Drop for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    fn drop(&mut self) {
        unsafe {
            let v_ref = self.v_ref;
            v_ref.drop(mem::transmute(self.as_struct_mut()));
        }
    }
}

impl<T: ?Sized, S> ops::Deref for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    type Target = T;

    fn deref(&self) -> &T { self.as_trait() }
} // impl Deref

impl<T: ?Sized, S> ops::DerefMut for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    fn deref_mut(&mut self) -> &mut T { self.as_trait_mut() }
} // impl DerefMut

impl<T: ?Sized, S> convert::From<Box<Class<T, S>>> for Box<DynClass<T, S>>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    fn from(t: Box<Class<T, S>>) -> Box<DynClass<T, S>> {
        unsafe { mem::transmute(t) }
    }
}

//
//  Casting
//
impl<T: ?Sized, S, B: ?Sized, P> UpCast<Box<DynClass<B, P>>> for Box<DynClass<T, S>>
    where T: ExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> Box<DynClass<B, P>> {
        //  Compute new v_ref and offset
        let new_v_ref = self.up_cast_trait::<B>();

        let new_offset = self.up_cast_struct::<P>();

        //  Commit result
        let mut s: Box<DynClass<B, P>> = unsafe { mem::transmute(self) };
        s.v_ref = new_v_ref;
        s.offset = new_offset;

        s
    }
}

impl<T: ?Sized, S, B: ?Sized, P> UpCastRef<DynClass<B, P>> for DynClass<T, S>
    where T: FirstExtendTrait<B> + marker::Reflect + 'static,
          S: FirstExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast_ref(&self) -> &DynClass<B, P> {
        unsafe { mem::transmute(self) }
    }

    fn up_cast_ref_mut(&mut self) -> &mut DynClass<B, P> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: ?Sized, S, D: ?Sized, C> DownCast<Box<DynClass<D, C>>> for Box<DynClass<T, S>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: ExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(self) -> Result<Box<DynClass<D, C>>, Box<DynClass<T, S>>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.down_cast_trait::<D>();

        let new_offset = self.down_cast_struct::<C>();

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(o)) = (new_v_ref, new_offset) {
            let mut s: Box<DynClass<D, C>> = unsafe { mem::transmute(self) };
            s.v_ref = v;
            s.offset = o;
            Ok(s)
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_down_cast(self) -> Box<DynClass<D, C>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.down_cast_trait::<D>().unwrap();

        let new_offset = self.down_cast_struct::<C>().unwrap();

        //  Commit result
        let mut s: Box<DynClass<D, C>> = mem::transmute(self);
        s.v_ref = new_v_ref;
        s.offset = new_offset;

        s
    }
}

impl<T: ?Sized, S, D: ?Sized, C> DownCastRef<DynClass<D, C>> for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: FirstExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast_ref(&self) -> Option<&DynClass<D, C>> {
        let is_trait_ok = self.down_cast_trait::<D>().is_some();

        let is_struct_ok = self.down_cast_struct::<C>().is_some();

        if is_trait_ok && is_struct_ok {
            Some(unsafe { mem::transmute(self) })
        } else {
            None
        }
    }

    fn down_cast_ref_mut(&mut self) -> Option<&mut DynClass<D, C>> {
        let is_trait_ok = self.down_cast_trait::<D>().is_some();

        let is_struct_ok = self.down_cast_struct::<C>().is_some();

        if is_trait_ok && is_struct_ok {
            Some(unsafe { mem::transmute(self) })
        } else {
            None
        }
    }

    unsafe fn unchecked_down_cast_ref(&self) -> &DynClass<D, C> {
        mem::transmute(self)
    }

    unsafe fn unchecked_down_cast_ref_mut(&mut self) -> &mut DynClass<D, C> {
        mem::transmute(self)
    }
}

