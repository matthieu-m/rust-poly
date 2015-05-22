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
    ($T:ty, $S:ty, $drop:ident, $cast:ident) => (
        {
            (
                $crate::rtti::trait_id::<$T>(),
                $crate::rtti::struct_id::<$S>(),
                $crate::rtti::VTable {
                    struct_info: $crate::rtti::get_struct_info::<$S>(),
                    trait_info: $crate::rtti::get_trait_info::<$T>(),
                    drop: $drop,
                    cast: $cast,
                    table: make_vptr!($T, $S),
                }
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

pub fn get_struct_info<Struct>() -> &'static StructInfo
    where Struct: marker::Reflect + 'static
{
    let struct_id = struct_id::<Struct>();
    get_struct_info_by_id(struct_id)
}

pub fn get_struct_info_by_id(struct_id: StructId) -> &'static StructInfo {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if STRUCT_INFO_REGISTRY.is_null() {
            panic!("Call init_struct_info_registry before the first call to get_struct_info.")
        }

        for &(s_id, ref struct_info) in &*(*STRUCT_INFO_REGISTRY).inner {
            if s_id == struct_id { return &struct_info; }
        }

        panic!("No such struct_info registered.");
    }
}

pub fn get_trait_info<Trait: ?Sized>() -> &'static TraitInfo
    where Trait: marker::Reflect + 'static
{
    let trait_id = trait_id::<Trait>();
    get_trait_info_by_id(trait_id)
}

pub fn get_trait_info_by_id(trait_id: TraitId) -> &'static TraitInfo {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if TRAIT_INFO_REGISTRY.is_null() {
            panic!("Call init_trait_info_registry before the first call to get_trait_info.")
        }

        for &(t_id, ref trait_info) in &*(*TRAIT_INFO_REGISTRY).inner {
            if t_id == trait_id { return &trait_info; }
        }

        panic!("No such trait_info registered.");
    }
}

pub fn get_vtable<Trait: ?Sized, Struct>() -> &'static VTable
    where Trait: marker::Reflect + 'static,
          Struct: DerivedFromTrait<Trait> + marker::Reflect + 'static
{
    let trait_id = trait_id::<Trait>();
    let struct_id = struct_id::<Struct>();
    get_vtable_by_id(trait_id, struct_id)
}

pub fn get_vtable_by_id(trait_id: TraitId, struct_id: StructId) -> &'static VTable {
    unsafe {
        // This function is supposed to be an intrinsic substituted by the compiler,
        // so I won't be thinking too hard about race conditions...
        if VTABLE_REGISTRY.is_null() {
            panic!("Call init_vtable_registry before the first call to get_vtable.")
        }

        for &(t_id, s_id, ref vtable) in &*(*VTABLE_REGISTRY).inner {
            if t_id == trait_id && s_id == struct_id { return vtable; }
        }

        panic!("No such vtable registered.");
    }
}


//
//  "Manual" marker traits
//
//  Those intrinsics should be automatically implemented by the compiler, based on the traits and types properties.
//
pub unsafe trait DerivedFromTrait<T: ?Sized> {}
pub unsafe trait DerivedFromStruct<T> {}
pub unsafe trait FirstDerivedFromTrait<T: ?Sized>: DerivedFromTrait<T> {}
pub unsafe trait FirstDerivedFromStruct<T>: DerivedFromStruct<T> {}


//
//  Cast Traits
//
pub trait UpCast<Target> {
    fn up_cast(self) -> Target;
}

pub trait UpCastRef<Target> {
    fn up_cast_ref(&self) -> &Target;
}

pub trait UpCastRefMut<Target>: UpCastRef<Target> {
    fn up_cast_ref_mut(&mut self) -> &mut Target;
}


pub trait DownCast<Target> {
    fn down_cast(self) -> Result<Target, Self>;

    unsafe fn unchecked_down_cast(self) -> Target;
}

pub trait DownCastRef<Target> {
    fn down_cast_ref(&self) -> Option<&Target>;

    unsafe fn unchecked_down_cast_ref(&self) -> &Target;
}

pub trait DownCastRefMut<Target>: DownCastRef<Target> {
    fn down_cast_ref_mut(&mut self) -> Option<&mut Target>;

    unsafe fn unchecked_down_cast_ref_mut(&mut self) -> &mut Target;
}


//
//  Raw representation of type info data in ROM.
//
#[repr(C)]
pub struct StructInfo {
    pub size: usize,
    pub align: usize,
    pub struct_id: StructId,
    pub is_first_derived: fn (StructId) -> bool,
}

#[repr(C)]
pub struct TraitInfo {
    pub trait_id: TraitId,
    //  FIXME: requires knowing the relative placement of the vtables for a given trait...
    //  pub cast: fn (TraitId) -> Option<isize>
}

#[repr(C)]
pub struct VTable {
    pub struct_info: &'static StructInfo,
    pub trait_info: &'static TraitInfo,
    pub drop: fn (&mut ()) -> (),
    pub cast: fn (TraitId) -> Option<&'static VTable>,
    pub table: *mut (),         // KLUDGE
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct UntypedVRef {
    vtable: &'static VTable,
}

#[repr(C)]
pub struct VRef<T: ?Sized>
    where T: marker::Reflect + 'static
{
    untyped: UntypedVRef,
    _0: marker::PhantomData<*const T>,
}

impl StructInfo {
    pub fn new<S>(ifd: fn (StructId) -> bool) -> StructInfo
        where S: marker::Reflect + 'static
    {
        StructInfo {
            size: mem::size_of::<S>(),
            align: mem::align_of::<S>(),
            struct_id: struct_id::<S>(),
            is_first_derived: ifd,
        }
    }
} // impl StructInfo

impl fmt::Debug for StructInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "StructInfo {{ size: {}, align: {}, struct_id: {:?} }}",
            self.size,
            self.align,
            self.struct_id
        )
    }
}

impl fmt::Debug for TraitInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "TraitInfo {{ trait_id: {:?} }}",
            self.trait_id
        )
    }
}

impl fmt::Debug for VTable {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VTable {{ struct_info: {:?}, trait_id: {:?} }}",
            self.struct_info,
            self.trait_info
        )
    }
}

impl UntypedVRef {
    pub fn new<T: ?Sized, S>() -> UntypedVRef
        where T: marker::Reflect + 'static,
              S: DerivedFromTrait<T> + marker::Reflect + 'static,
    {
        UntypedVRef { vtable: get_vtable::<T, S>() }
    }

    pub fn get_vtable(&self) -> &'static VTable {
        self.vtable
    }

    pub fn get_struct_info(&self) -> &'static StructInfo {
        self.vtable.struct_info
    }

    pub fn get_trait_info(&self) -> &'static TraitInfo {
        self.vtable.trait_info
    }

    pub fn up_cast<T: ?Sized, B: ?Sized>(&self) -> UntypedVRef
        where B: marker::Reflect + 'static,
              T: DerivedFromTrait<B> + marker::Reflect + 'static
    {
        UntypedVRef { vtable: (self.vtable.cast)(trait_id::<B>()).unwrap() }
    }

    pub fn cast<T: ?Sized, D: ?Sized>(&self) -> Option<UntypedVRef>
        where T: marker::Reflect + 'static,
              D: marker::Reflect + 'static
    {
        (self.vtable.cast)(trait_id::<D>()).map(|vt| {
            UntypedVRef { vtable: vt }
        })
    }

    pub fn drop(&self, it: &mut ()) {
        (self.vtable.drop)(it)
    }
} // impl UntypedVRef

impl<T: ?Sized> VRef<T>
    where T: marker::Reflect + 'static
{
    pub fn new<S>() -> VRef<T>
        where S: DerivedFromTrait<T> + marker::Reflect + 'static
    {
        VRef { untyped: UntypedVRef::new::<T, S>(), _0: marker::PhantomData }
    }

    pub fn get_vtable(&self) -> &'static VTable {
        self.untyped.get_vtable()
    }

    pub fn get_struct_info(&self) -> &'static StructInfo {
        self.untyped.get_struct_info()
    }

    pub fn get_trait_info(&self) -> &'static TraitInfo {
        self.untyped.get_trait_info()
    }

    pub fn up_cast<B: ?Sized>(&self) -> VRef<B>
        where B: marker::Reflect + 'static,
              T: DerivedFromTrait<B>
    {
        VRef { untyped: self.untyped.up_cast::<T, B>(), _0: marker::PhantomData }
    }

    pub fn cast<D: ?Sized>(&self) -> Option<VRef<D>>
        where D: DerivedFromTrait<T> + marker::Reflect + 'static
    {
        self.untyped.cast::<T, D>().map(|u| {
            VRef { untyped: u, _0: marker::PhantomData }
        })
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
          S: DerivedFromTrait<T> + marker::Reflect + 'static,
{
    current: VRef<T>,
    original: UntypedVRef,
    data: S,
}

#[repr(C)]
pub struct DynClass<T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    current: VRef<T>,
    original: UntypedVRef,
    data: (),
    _0: marker::PhantomData<S>,
}

pub type Dyn<T> = DynClass<T, ()>;

//
// Class
//
impl<T: ?Sized, S> Class<T, S>
    where T: marker::Reflect + 'static,
          S: DerivedFromTrait<T> + marker::Reflect + 'static,
{
    pub fn new(data: S) -> Class<T, S> {
        Class { current: VRef::new::<S>(), original: UntypedVRef::new::<T, S>(), data: data }
    }
} // impl Class

impl<T: ?Sized, S> Drop for Class<T, S>
    where T: marker::Reflect + 'static,
          S: DerivedFromTrait<T> + marker::Reflect + 'static
{
    fn drop(&mut self) {
        drop(&mut self.data);
    }
}

impl<T: ?Sized, S> convert::From<Box<Class<T, S>>> for Box<DynClass<T, S>>
    where T: marker::Reflect + 'static,
          S: DerivedFromTrait<T> + marker::Reflect + 'static,
{
    fn from(t: Box<Class<T, S>>) -> Box<DynClass<T, S>> {
        unsafe { mem::transmute(t) }
    }
}

//
// DynClass
//
impl<T: ?Sized, S> DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
/*
 *  FIXME: error: transmute called on types with different sizes: core::raw::TraitObject (128 bits) to &T (64 bits)

    pub fn as_trait(&self) -> &T {
        unsafe {
            mem::transmute(raw::TraitObject {
                data:   mem::transmute(&self.data),
                vtable: self.current.get_vtable().table,
            })
        }
    }

    pub fn as_trait_mut(&mut self) -> &mut T {
        unsafe {
            mem::transmute(raw::TraitObject {
                data:   mem::transmute(&mut self.data),
                vtable: self.current.get_vtable().table,
            })
        }
    }
*/

    pub fn as_struct(&self) -> &S {
        unsafe { mem::transmute(&self.data) }
    }

    pub fn as_struct_mut(&mut self) -> &mut S {
        unsafe { mem::transmute(&mut self.data) }
    }
} // impl DynClass

impl<T: ?Sized, S> fmt::Debug for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: fmt::Debug + marker::Reflect + 'static
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let data: &S = unsafe { mem::transmute(&self.data) };
        write!(
            formatter,
            "DynClass {{ current: {:?}, original: {:?}, data: {:?} }}",
            self.current,
            self.original,
            data
        )
    }
}

impl<T: ?Sized, S> Drop for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    fn drop(&mut self) {
        let o = self.original;
        o.drop(&mut self.data);
    }
}

/*
 *  FIXME: find a way to transmute a raw::TraitObject into a &T

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
*/

impl<T: ?Sized, S, B: ?Sized, P> UpCast<Box<DynClass<B, P>>> for Box<DynClass<T, S>>
    where T: DerivedFromTrait<B> + marker::Reflect + 'static,
          S: FirstDerivedFromStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> Box<DynClass<B, P>> {
        //  Adjust v-ptr, then return
        let mut s = self;

        unsafe {
            s.current = mem::transmute(s.current.up_cast::<B>());
            mem::transmute(s)
        }
    }
}

impl<T: ?Sized, S, B: ?Sized, P> UpCastRef<DynClass<B, P>> for DynClass<T, S>
    where T: FirstDerivedFromTrait<B> + marker::Reflect + 'static,
          S: FirstDerivedFromStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast_ref(&self) -> &DynClass<B, P> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: ?Sized, S, B: ?Sized, P> UpCastRefMut<DynClass<B, P>> for DynClass<T, S>
    where T: FirstDerivedFromTrait<B> + marker::Reflect + 'static,
          S: FirstDerivedFromStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast_ref_mut(&mut self) -> &mut DynClass<B, P> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: ?Sized, S, D: ?Sized, C> DownCast<Box<DynClass<D, C>>> for Box<DynClass<T, S>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: DerivedFromTrait<T> + marker::Reflect + 'static,
          C: FirstDerivedFromStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(self) -> Result<Box<DynClass<D, C>>, Box<DynClass<T, S>>> {
        if !(self.original.get_struct_info().is_first_derived)(struct_id::<C>()) {
            return Err(self);
        }

        //  Adjust v-ptr, then return
        let mut s = self;
        let new_table = s.original.cast::<T, D>();

        match new_table {
        None => Err(s),
        Some(v) => { unsafe { s.current = mem::transmute(v); Ok(mem::transmute(s)) } },
        }
    }

    unsafe fn unchecked_down_cast(self) -> Box<DynClass<D, C>> {
        //  Adjust v-ptr, then return
        let mut s = self;

        s.current = mem::transmute(s.original.cast::<T, D>().unwrap());
        mem::transmute(s)
    }
}

impl<T: ?Sized, S, D: ?Sized, C> DownCastRef<DynClass<D, C>> for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: FirstDerivedFromTrait<T> + marker::Reflect + 'static,
          C: FirstDerivedFromStruct<S> + marker::Reflect + 'static,
{
    fn down_cast_ref(&self) -> Option<&DynClass<D, C>> {
        if !(self.current.get_struct_info().is_first_derived)(struct_id::<C>()) {
            return None;
        }

        let vt = self.current.cast::<D>();
        vt.map(|_| {
            unsafe { mem::transmute(self) }
        })
    }

    unsafe fn unchecked_down_cast_ref(&self) -> &DynClass<D, C> {
        mem::transmute(self)
    }
}

impl<T: ?Sized, S, D: ?Sized, C> DownCastRefMut<DynClass<D, C>> for DynClass<T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: FirstDerivedFromTrait<T> + marker::Reflect + 'static,
          C: FirstDerivedFromStruct<S> + marker::Reflect + 'static
{
    fn down_cast_ref_mut(&mut self) -> Option<&mut DynClass<D, C>> {
        if !(self.current.get_struct_info().is_first_derived)(struct_id::<C>()) {
            return None;
        }

        let vt = self.current.cast::<D>();
        match vt {
        Some(_) => unsafe { mem::transmute(self) },
        None => None,
        }
    }

    unsafe fn unchecked_down_cast_ref_mut(&mut self) -> &mut DynClass<D, C> {
        mem::transmute(self)
    }
}
