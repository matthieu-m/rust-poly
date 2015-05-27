//
//  A tentative implementation of RFC: Disjoint Polymorphism
//
//  [Library & Compiler] part
//
#![allow(dead_code)]

use alloc::boxed::Box;
use core::fmt;
use core::intrinsics;
use core::marker;
use core::mem;

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
                $crate::internal::trait_id::<$T>(),
                $crate::internal::struct_id::<$S>(),
                $crate::internal::VTable::new::<$T, $S>(
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
pub unsafe trait ExtendStruct<T> { fn offsets() -> &'static [isize]; }
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
