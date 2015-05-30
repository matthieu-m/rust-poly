//
//  A tentative implementation of RFC: Disjoint Polymorphism
//
//  [Library] part
//
#![allow(dead_code)]
#![allow(drop_with_repr_extern)]

use alloc::boxed::Box;
use core::clone;
use core::convert;
use core::fmt;
use core::marker;
use core::mem;
use core::ops;
use core::raw;
use core::result::Result;

use std;

use internal::{ExtendStruct, ExtendTrait, FirstExtendStruct, FirstExtendTrait, TraitExtendTrait};
use internal::{StructInfo, TraitInfo, VTable};
use internal::{struct_id, trait_id, v_table};


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

pub trait Cast<Target> {
    fn cast(self) -> Result<Target, Self>;

    unsafe fn unchecked_cast(self) -> Target;
}

//  If Cast is restricted to cross-casts, then CastRef makes little sense...
//  ... to have a shareable v-ptr, two traits must be related.

//
//  &T and &mut T
//
//  FIXME: once &T and &mut T are wired into VTable, implement UpCast/DownCast/Cast
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

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct VOffset {
    bundled_offset: u64,
}

struct VData<'a, S>
    where S: Sized + marker::Reflect + 'static
{
    v_offset: VOffset,
    ptr: &'a S,
}

struct VDataMut<'a, S>
    where S: Sized + marker::Reflect + 'static
{
    v_offset: VOffset,
    ptr: &'a mut S,
}

impl UntypedVRef {
    pub fn new(v_table: &'static VTable) -> UntypedVRef {
        UntypedVRef { v_table: v_table }
    }

    pub fn v_table(&self) -> &'static VTable { self.v_table }

    pub fn struct_info(&self) -> &'static StructInfo {
        self.v_table().struct_info()
    }

    pub fn trait_info(&self) -> &'static TraitInfo {
        self.v_table().trait_info()
    }

    pub fn up_cast<T: ?Sized, B: ?Sized>(&self) -> UntypedVRef
        where B: marker::Reflect + 'static,
              T: TraitExtendTrait<B> + marker::Reflect + 'static
    {
        let v_table: &'static VTable = unsafe {
            let raw: *const u8 = mem::transmute(self.v_table);
            mem::transmute(raw as isize + <T as TraitExtendTrait<B>>::offset())
        };

        UntypedVRef::new(v_table)
    }

    pub fn down_cast<T: ?Sized, D: ?Sized>(&self) -> Option<UntypedVRef>
        where T: marker::Reflect + 'static,
              D: TraitExtendTrait<T> + marker::Reflect + 'static
    {
        if trait_id::<T>() == trait_id::<D>() { return Some(*self); }

        self.v_table().cast_to_trait::<D>().map(|vt| {
            UntypedVRef::new(vt)
        })
    }

    pub fn cast<T: ?Sized, X: ?Sized>(&self) -> Option<UntypedVRef>
        where T: marker::Reflect + 'static,
              X: marker::Reflect + 'static
    {
        if trait_id::<T>() == trait_id::<X>() { return Some(*self); }

        self.v_table().cast_to_trait::<X>().map(|vt| {
            UntypedVRef::new(vt)
        })
    }

    pub fn drop(&self, it: *mut ()) {
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
              T: TraitExtendTrait<B>
    {
        VRef { untyped: self.untyped.up_cast::<T, B>(), _0: marker::PhantomData }
    }

    pub fn down_cast<D: ?Sized>(&self) -> Option<VRef<D>>
        where D: TraitExtendTrait<T> + marker::Reflect + 'static
    {
        self.untyped.down_cast::<T, D>().map(|u| {
            VRef { untyped: u, _0: marker::PhantomData }
        })
    }

    pub fn cast<X: ?Sized>(&self) -> Option<VRef<X>>
        where X: marker::Reflect + 'static
    {
        self.untyped.cast::<T, X>().map(|u| {
            VRef { untyped: u, _0: marker::PhantomData }
        })
    }

    pub fn is<S>(&self) -> bool
        where S: marker::Reflect + 'static
    {
        self.struct_info().offsets(struct_id::<S>()).len() > 0
    }

    pub fn drop(&self, it: *mut ()) {
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

impl VOffset {
    const ALIGN_MASK    : u64 = 0xffffffffffffff_u64;
    const ALIGN_SHIFT   : u64 = 56;
    const MULT_MASK     : u64 = 0xffffffffffff_u64;
    const MULT_SHIFT    : u64 = 48;

    pub fn new(log2_align: u8, multiplier: u8, offset: isize) -> VOffset {
        assert!(0 <= offset && (offset as i64) < (1_i64 << VOffset::MULT_SHIFT), "Out-of-bounds offset");

        let offset =
            ((log2_align as u64) << VOffset::ALIGN_SHIFT) +
            ((multiplier as u64) << VOffset::MULT_SHIFT) +
            (offset as u64);

        VOffset { bundled_offset: offset }
    }

    pub fn new_offset(&self, offset: isize) -> VOffset {
        VOffset::new(self.log2_align() as u8, self.multiplier() as u8, offset)
    }

    pub fn base_offset(&self) -> isize { self.multiplier() << self.log2_align() }

    pub fn offset(&self) -> isize { (self.bundled_offset & VOffset::MULT_MASK) as isize }

    fn log2_align(&self) -> isize { (self.bundled_offset >> VOffset::ALIGN_SHIFT) as isize }

    fn multiplier(&self) -> isize {
        ((self.bundled_offset & VOffset::ALIGN_MASK) >> VOffset::MULT_SHIFT) as isize
    }
}

impl<'a, S> VData<'a, S>
    where S: marker::Reflect + 'static
{
    fn new(offset: VOffset, ptr: &'a S) -> VData<'a, S> {
        VData { v_offset: offset, ptr: ptr }
    }

    fn base_offset(&self) -> isize { self.v_offset.base_offset() }

    fn offset(&self) -> isize { self.v_offset.offset() }

    fn as_struct(&self) -> &S { self.ptr }
} // impl VData

impl<'a, S> VDataMut<'a, S>
    where S: marker::Reflect + 'static
{
    fn new(offset: VOffset, ptr: &'a mut S) -> VDataMut<'a, S> {
        VDataMut { v_offset: offset, ptr: ptr }
    }

    fn base_offset(&self) -> isize { self.v_offset.base_offset() }

    fn offset(&self) -> isize { self.v_offset.offset() }

    fn as_struct(&self) -> &S { self.ptr }

    fn as_struct_mut(&mut self) -> &mut S { self.ptr }
}

impl<'a, S> clone::Clone for VData<'a, S>
    where S: marker::Reflect + 'static,
{
    fn clone(&self) -> Self {
        VData { v_offset: self.v_offset, ptr: self.ptr }
    }
}

impl<'a, S> fmt::Debug for VData<'a, S>
    where S: fmt::Debug + marker::Reflect + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VData {{ v_offset: {:?}, ptr: {:?} }}",
            self.v_offset,
            self.ptr,
        )
    }
}

impl<'a, S> fmt::Debug for VDataMut<'a, S>
    where S: fmt::Debug + marker::Reflect + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VDataMut {{ v_offset: {:?}, ptr: {:?} }}",
            self.v_offset,
            self.ptr,
        )
    }
}

trait VDataImpl<Pointer> {
    type Inner: Sized + marker::Reflect + 'static;

    unsafe fn make(v_offset: VOffset, ptr: Pointer) -> Self;

    unsafe fn add_offset<Target>(self, off: isize) -> Target
        where Target: VDataImpl<Pointer> + Sized;
}

impl<'a, S> VDataImpl<*const u8> for VData<'a, S>
    where S: Sized + marker::Reflect + 'static
{
    type Inner = S;

    unsafe fn make(v_offset: VOffset, ptr: *const u8) -> Self {
        VData::new(v_offset, mem::transmute(ptr))
    }

    unsafe fn add_offset<Target>(self, o: isize) -> Target
        where Target: VDataImpl<*const u8> + Sized
    {
        let new_v_offset = self.v_offset.new_offset(self.offset() + o);
        let ptr: *const u8 = mem::transmute(self.ptr);

        Target::make(new_v_offset, ptr.offset(o))
    }
}

impl<'a, S> VDataImpl<*mut u8> for VDataMut<'a, S>
    where S: Sized + marker::Reflect + 'static
{
    type Inner = S;

    unsafe fn make(v_offset: VOffset, ptr: *mut u8) -> Self {
        VDataMut::new(v_offset, mem::transmute(ptr))
    }

    unsafe fn add_offset<Target>(self, o: isize) -> Target
        where Target: VDataImpl<*mut u8> + Sized
    {
        let new_v_offset = self.v_offset.new_offset(self.offset() + o);
        let ptr: *mut u8 = mem::transmute(self.ptr);

        Target::make(new_v_offset, ptr.offset(o))
    }
}

trait VDataCast<Target, Pointer>
    where Self: VDataImpl<Pointer> + Sized,
          Target: VDataImpl<Pointer> + Sized
{
    fn up_cast(self) -> Target
        where Self::Inner: ExtendStruct<Target::Inner>,
    {
        if struct_id::<Self::Inner>() == struct_id::<Target::Inner>() {
            return unsafe { self.add_offset(0) };
        }

        let offsets = <Self::Inner as ExtendStruct<Target::Inner>>::offsets();
        assert!(offsets.len() == 1, "Multiple offsets support not implemented yet");

        unsafe { self.add_offset(*offsets.get_unchecked(0)) }
    }

    fn down_cast<T: ?Sized>(self, v_ref: VRef<T>) -> Result<Target, Self>
        where T: marker::Reflect + 'static,
              Target::Inner: ExtendStruct<Self::Inner>,
    {
        if struct_id::<Self::Inner>() == struct_id::<Target::Inner>() {
            return unsafe { Ok(self.add_offset(0)) };
        }

        if !v_ref.is::<Target::Inner>() { return Err(self); }

        let offsets = <Target::Inner as ExtendStruct<Self::Inner>>::offsets();
        assert!(offsets.len() == 1, "Support for diamond inheritance is not yet implemented!");

        unsafe { Ok(self.add_offset(*offsets.get_unchecked(0))) }
    }

    fn cast<T: ?Sized>(self, v_ref: VRef<T>) -> Result<Target, Self>
        where T: marker::Reflect + 'static,
    {
        if struct_id::<Self::Inner>() == struct_id::<Target::Inner>() {
            return unsafe { Ok(self.add_offset(0)) };
        }

        let offsets = v_ref.struct_info().offsets(struct_id::<Target::Inner>());
        assert!(offsets.len() <= 1, "Support for diamond inheritance is not yet implemented!");

        match offsets.first() {
        Some(o) => unsafe { Ok(self.add_offset(*o)) },
        None    => Err(self),
        }
    }
}

impl<'a, S, Y> VDataCast<VData<'a, Y>, *const u8> for VData<'a, S>
    where S: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{}

impl<'a, S, Y> VDataCast<VDataMut<'a, Y>, *mut u8> for VDataMut<'a, S>
    where S: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{}


//
//  Capacities: Clonable
//
#[derive(Copy, Clone, Debug)]
pub struct Clonable;


//
//  Class, DynClass (& Dyn), DynRef, DynRefMut
//
#[repr(C)]
#[derive(Debug)]
pub struct Class<T: ?Sized, S, CP>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    dyn: DynClass<T, S, CP>,
    data: S,
}

#[repr(C)]
pub struct DynClass<T: ?Sized, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    v_offset: VOffset,
    _0: marker::PhantomData<(S, CP)>,
}

pub type Dyn<T> = DynClass<T, (), ()>;

#[derive(Clone, Debug)]
pub struct DynRef<'a, T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    v_data: VData<'a, S>,
}

#[derive(Debug)]
pub struct DynRefMut<'a, T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    v_data: VDataMut<'a, S>,
}

//
//  Class
//
impl<T: ?Sized, S> Class<T, S, ()>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    pub fn new(data: S) -> Class<T, S, ()> {
        assert!(offset_of!(Self, dyn) == 0, "Essential for &Class -> &DynClass conversion!");

        let offset = offset_of!(Self, data);

        fn compact(n: isize) -> (u8, u8) {
            assert!(n > 0);

            let mut n = n;
            let mut acc = 0;
            while n % 2 == 0 {
                n /= 2;
                acc += 1;
            }

            assert!(n < 256);

            (acc, n as u8)
        }

        let (log2_align, mult) = compact(offset);
        let v_offset = VOffset::new(log2_align, mult, offset);

        Class { dyn: unsafe { DynClass::new(VRef::new::<S>(), v_offset) }, data: data }
    }

    pub fn into_clonable(self) -> Class<T, S, (Clonable)>
        where S: clone::Clone
    {
        let dyn = unsafe { DynClass::new(self.dyn.v_ref, self.dyn.v_offset) };
        Class { dyn: dyn, data: self.data }
    }
} // impl Class

impl<T: ?Sized, S, CP> clone::Clone for Class<T, S, CP>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + clone::Clone + marker::Reflect + 'static,
{
    fn clone(&self) -> Self {
        let new_dyn = unsafe { DynClass::new(self.dyn.v_ref, self.dyn.v_offset) };
        Class { dyn: new_dyn, data: self.data.clone() }
    }
}

//
//  DynClass
//
impl<T: ?Sized, S, CP> DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    unsafe fn new(v_ref: VRef<T>, v_offset: VOffset) -> DynClass<T, S, CP> {
        DynClass { v_ref: v_ref, v_offset: v_offset, _0: marker::PhantomData }
    }

    pub fn as_trait(&self) -> &T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_trait_mut(&mut self) -> &mut T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct_mut()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_struct(&self) -> &S {
        unsafe { mem::transmute(self.data_ptr()) }
    }

    pub fn as_struct_mut(&mut self) -> &mut S {
        unsafe { mem::transmute(self.data_ptr_mut()) }
    }

    //  Invariant: data_ptr() = base_ptr() + offset_into_struct()
    fn data_ptr(&self) -> *const () {
        unsafe {
            let offset = self.v_offset.offset();
            let base: *const u8 = mem::transmute(self);
            mem::transmute(base.offset(offset))
        }
    }

    fn data_ptr_mut(&mut self) -> *mut () {
        unsafe {
            let offset = self.v_offset.offset();
            let base: *mut u8 = mem::transmute(self);
            mem::transmute(base.offset(offset))
        }
    }

    fn base_ptr(&self) -> *const () {
        unsafe {
            let offset = self.v_offset.base_offset();
            let base: *const u8 = mem::transmute(self);
            mem::transmute(base.offset(offset))
        }
    }

    fn base_ptr_mut(&mut self) -> *mut () {
        unsafe {
            let offset = self.v_offset.base_offset();
            let base: *mut u8 = mem::transmute(self);
            mem::transmute(base.offset(offset))
        }
    }

    fn offset_into_struct(&self) -> isize {
        self.v_offset.offset() - self.v_offset.base_offset()
    }

    fn up_cast_struct<P>(&self) -> VOffset
        where S: ExtendStruct<P>,
              P: marker::Reflect + 'static,
    {
        let current = VData::new(self.v_offset, self.as_struct());
        let target: VData<P> = current.up_cast();

        target.v_offset
    }

    fn down_cast_struct<C>(&self) -> Option<VOffset>
        where C: ExtendStruct<S> + marker::Reflect + 'static,
    {
        let current = VData::new(self.v_offset, self.as_struct());

        current.down_cast(self.v_ref).ok().map(|vd: VData<C>| vd.v_offset)
    }

    fn cast_struct<Y>(&self) -> Option<VOffset>
        where Y: marker::Reflect + 'static,
    {
        let current = VData::new(self.v_offset, self.as_struct());

        current.cast(self.v_ref).ok().map(|vd: VData<Y>| vd.v_offset)
    }
} // impl DynClass

impl<T: ?Sized, S> DynClass<T, S, (Clonable)>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    pub fn clone_to_box(&self) -> Box<DynClass<T, S, (Clonable)>> {
        let struct_info = self.v_ref.struct_info();
        assert!(struct_info.is_clonable(), "(Clonable) should have guaranteed that...");

        //  Okay... so we need to estimate how much memory we will need for this,
        //  and what the alignment of this memory should be.
        let (size, align) = {
            use core::cmp;

            assert!(self.v_offset.base_offset() >= 0);

            let size = self.v_offset.base_offset() as usize + struct_info.size();
            let align = cmp::max(
                mem::align_of::<DynClass<T, S, (Clonable)>>(),
                (1_usize << struct_info.log2_align())
            );
            (size, align)
        };

        let original: &DynClass<T, S, (Clonable)> = &*self;

        unsafe {
            use alloc::heap;
            use core::ptr;

            let raw = heap::allocate(size, align);

            let head    : *const u8 = mem::transmute(original);
            let head_raw: *mut u8   = raw;
            ptr::copy_nonoverlapping(head, head_raw, mem::size_of::<DynClass<T, S, (Clonable)>>());

            let tail    : *const u8 = mem::transmute(original.base_ptr());
            let tail_raw: *mut u8   = head_raw.offset(original.v_offset.offset());
            struct_info.clone(tail, tail_raw);

            mem::transmute(raw)
        }
    }
}

impl<T: ?Sized, S, CP> fmt::Debug for DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: fmt::Debug + marker::Reflect + 'static
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "DynClass {{ v_ref: {:?}, v_offset: {:?}, data: {:?} }}",
            self.v_ref,
            self.v_offset,
            self.as_struct(),
        )
    }
} // impl Debug for DynClass

impl<T: ?Sized, S, CP> Drop for DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    fn drop(&mut self) {
        let v_ref = self.v_ref;
        v_ref.drop(self.base_ptr_mut());
    }
}

impl<T: ?Sized, S, CP> ops::Deref for DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    type Target = T;

    fn deref(&self) -> &T { self.as_trait() }
} // impl Deref

impl<T: ?Sized, S, CP> ops::DerefMut for DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    fn deref_mut(&mut self) -> &mut T { self.as_trait_mut() }
} // impl DerefMut

impl<T: ?Sized, S> clone::Clone for Box<DynClass<T, S, (Clonable)>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    fn clone(&self) -> Self {
        self.clone_to_box()
    }
} // impl Clone

impl<T: ?Sized, S, CP> convert::From<Box<Class<T, S, CP>>> for Box<DynClass<T, S, CP>>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    fn from(t: Box<Class<T, S, CP>>) -> Box<DynClass<T, S, CP>> {
        unsafe { mem::transmute(t) }
    }
}

//
//  Casting
//
impl<T: ?Sized, S, B: ?Sized, P, CP> UpCast<Box<DynClass<B, P, CP>>> for Box<DynClass<T, S, CP>>
    where T: TraitExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> Box<DynClass<B, P, CP>> {
        //  Compute new v_ref and offset
        let new_v_ref = self.v_ref.up_cast::<B>();

        let new_v_offset = self.up_cast_struct::<P>();

        //  Commit result
        let mut s: Box<DynClass<B, P, CP>> = unsafe { mem::transmute(self) };
        s.v_ref = new_v_ref;
        s.v_offset = new_v_offset;

        s
    }
}

impl<T: ?Sized, S, B: ?Sized, P, CP> UpCastRef<DynClass<B, P, CP>> for DynClass<T, S, CP>
    where T: FirstExtendTrait<B> + marker::Reflect + 'static,
          S: FirstExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast_ref(&self) -> &DynClass<B, P, CP> {
        unsafe { mem::transmute(self) }
    }

    fn up_cast_ref_mut(&mut self) -> &mut DynClass<B, P, CP> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: ?Sized, S, B: ?Sized, P, CP> UpCastRef<Box<DynClass<B, P, CP>>> for Box<DynClass<T, S, CP>>
    where T: FirstExtendTrait<B> + marker::Reflect + 'static,
          S: FirstExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast_ref(&self) -> &Box<DynClass<B, P, CP>> {
        unsafe { mem::transmute(self) }
    }

    fn up_cast_ref_mut(&mut self) -> &mut Box<DynClass<B, P, CP>> {
        unsafe { mem::transmute(self) }
    }
}

impl<T: ?Sized, S, D: ?Sized, C, CP> DownCast<Box<DynClass<D, C, CP>>> for Box<DynClass<T, S, CP>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: TraitExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(self) -> Result<Box<DynClass<D, C, CP>>, Box<DynClass<T, S, CP>>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast::<D>();

        let new_v_offset = self.down_cast_struct::<C>();

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(r), Some(o)) = (new_v_ref, new_v_offset) {
            let mut s: Box<DynClass<D, C, CP>> = unsafe { mem::transmute(self) };
            s.v_ref = r;
            s.v_offset = o;
            Ok(s)
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_down_cast(self) -> Box<DynClass<D, C, CP>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast::<D>().unwrap();

        let new_v_offset = self.down_cast_struct::<C>().unwrap();

        //  Commit result
        let mut s: Box<DynClass<D, C, CP>> = mem::transmute(self);
        s.v_ref = new_v_ref;
        s.v_offset = new_v_offset;

        s
    }
}

impl<T: ?Sized, S, D: ?Sized, C, CP> DownCastRef<DynClass<D, C, CP>> for DynClass<T, S, CP>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: FirstExtendTrait<T> + TraitExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast_ref(&self) -> Option<&DynClass<D, C, CP>> {
        let is_trait_ok = self.v_ref.down_cast::<D>().is_some();

        let is_struct_ok = self.down_cast_struct::<C>().is_some();

        if is_trait_ok && is_struct_ok {
            Some(unsafe { mem::transmute(self) })
        } else {
            None
        }
    }

    fn down_cast_ref_mut(&mut self) -> Option<&mut DynClass<D, C, CP>> {
        let is_trait_ok = self.v_ref.down_cast::<D>().is_some();

        let is_struct_ok = self.down_cast_struct::<C>().is_some();

        if is_trait_ok && is_struct_ok {
            Some(unsafe { mem::transmute(self) })
        } else {
            None
        }
    }

    unsafe fn unchecked_down_cast_ref(&self) -> &DynClass<D, C, CP> {
        mem::transmute(self)
    }

    unsafe fn unchecked_down_cast_ref_mut(&mut self) -> &mut DynClass<D, C, CP> {
        mem::transmute(self)
    }
}

impl<T: ?Sized, S, X: ?Sized, Y, CP> Cast<Box<DynClass<X, Y, CP>>> for Box<DynClass<T, S, CP>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(self) -> Result<Box<DynClass<X, Y, CP>>, Box<DynClass<T, S, CP>>> {
        let new_v_ref = self.v_ref.cast::<X>();

        let new_v_offset = self.cast_struct::<Y>();

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(r), Some(o)) = (new_v_ref, new_v_offset) {
            let mut s: Box<DynClass<X, Y, CP>> = unsafe { mem::transmute(self) };
            s.v_ref = r;
            s.v_offset = o;
            Ok(s)
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_cast(self) -> Box<DynClass<X, Y, CP>> {
        let new_v_ref = self.v_ref.cast::<X>().unwrap();

        let new_v_offset = self.cast_struct::<Y>().unwrap();

        let mut s: Box<DynClass<X, Y, CP>> = mem::transmute(self);
        s.v_ref = new_v_ref;
        s.v_offset = new_v_offset;
        s
    }
}

//
//  DynRef & DynRefMut
//
impl<'a, T: ?Sized, S> DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    pub fn new<CP>(c: &'a DynClass<T, S, CP>) -> DynRef<'a, T, S>{
        let v_offset = VOffset::new(0, 0, c.offset_into_struct());
        DynRef {
            v_ref: c.v_ref,
            v_data: VData::new(v_offset, c.as_struct()),
        }
    }

    pub fn as_trait(&self) -> &T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_struct(&self) -> &S {
        self.v_data.as_struct()
    }
}

impl<'a, T: ?Sized, S> DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    pub fn new<CP>(c: &'a mut DynClass<T, S, CP>) -> DynRefMut<'a, T, S> {
        let v_offset = VOffset::new(0, 0, c.offset_into_struct());
        DynRefMut {
            v_ref: c.v_ref,
            v_data: VDataMut::new(v_offset, c.as_struct_mut()),
        }
    }

    pub fn as_trait(&self) -> &T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_trait_mut(&mut self) -> &mut T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct_mut()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_struct(&self) -> &S {
        self.v_data.as_struct()
    }

    pub fn as_struct_mut(&mut self) -> &mut S {
        self.v_data.as_struct_mut()
    }
}

impl<'a, T: ?Sized, S> ops::Deref for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    type Target = T;

    fn deref(&self) -> &T { self.as_trait() }
} // impl Deref

impl<'a, T: ?Sized, S> ops::Deref for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    type Target = T;

    fn deref(&self) -> &T { self.as_trait() }
} // impl Deref

impl<'a, T: ?Sized, S> ops::DerefMut for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static
{
    fn deref_mut(&mut self) -> &mut T { self.as_trait_mut() }
} // impl DerefMut

impl<'a, T: ?Sized, S> convert::From<DynRefMut<'a, T, S>> for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: ExtendTrait<T> + marker::Reflect + 'static,
{
    fn from(r: DynRefMut<'a, T, S>) -> DynRef<'a, T, S> {
        DynRef { v_ref: r.v_ref, v_data: VData::new(r.v_data.v_offset, r.v_data.ptr) }
    }
}


//
//  Casting
//
impl<'a, T: ?Sized, S, B: ?Sized, P> UpCast<DynRef<'a, B, P>> for DynRef<'a, T, S>
    where T: TraitExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> DynRef<'a, B, P> {
        //  Compute new v_ref and offset
        let new_v_ref = self.v_ref.up_cast();

        let new_v_data = self.v_data.up_cast();

        DynRef { v_ref: new_v_ref, v_data: new_v_data }
    }
}

impl<'a, T: ?Sized, S, B: ?Sized, P> UpCast<DynRefMut<'a, B, P>> for DynRefMut<'a, T, S>
    where T: TraitExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> DynRefMut<'a, B, P> {
        //  Compute new v_ref and offset
        let new_v_ref = self.v_ref.up_cast();

        let new_v_data = self.v_data.up_cast();

        DynRefMut { v_ref: new_v_ref, v_data: new_v_data }
    }
}

impl<'a, T: ?Sized, S, D: ?Sized, C> DownCast<DynRef<'a, D, C>> for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: TraitExtendTrait<T> + marker::Reflect + 'static,
          C: ExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(mut self) -> Result<DynRef<'a, D, C>, DynRef<'a, T, S>> {
        if let Some(r) = self.v_ref.down_cast() {
            let new_v_data = self.v_data.down_cast(self.v_ref);

            if let Ok(d) = new_v_data { return Ok(DynRef { v_ref: r, v_data: d }); }

            self.v_data = new_v_data.err().unwrap();
        }

        Err(self)
    }

    unsafe fn unchecked_down_cast(self) -> DynRef<'a, D, C> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast().unwrap();

        let new_v_data = self.v_data.down_cast(self.v_ref).ok().unwrap();

        //  Commit result
        DynRef { v_ref: new_v_ref, v_data: new_v_data }
    }
}

impl<'a, T: ?Sized, S, D: ?Sized, C> DownCast<DynRefMut<'a, D, C>> for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: TraitExtendTrait<T> + marker::Reflect + 'static,
          C: ExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(mut self) -> Result<DynRefMut<'a, D, C>, DynRefMut<'a, T, S>> {
        if let Some(r) = self.v_ref.down_cast() {
            let new_v_data = self.v_data.down_cast(self.v_ref);

            if let Ok(d) = new_v_data { return Ok(DynRefMut { v_ref: r, v_data: d }); }

            self.v_data = new_v_data.err().unwrap();
        }

        Err(self)
    }

    unsafe fn unchecked_down_cast(self) -> DynRefMut<'a, D, C> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast().unwrap();

        let new_v_data = self.v_data.down_cast(self.v_ref).ok().unwrap();

        //  Commit result
        DynRefMut { v_ref: new_v_ref, v_data: new_v_data }
    }
}

impl<'a, T: ?Sized, S, X: ?Sized, Y> Cast<DynRef<'a, X, Y>> for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(mut self) -> Result<DynRef<'a, X, Y>, DynRef<'a, T, S>> {
        if let Some(r) = self.v_ref.cast() {
            let new_v_data = self.v_data.cast(self.v_ref);

            if let Ok(d) = new_v_data { return Ok(DynRef { v_ref: r, v_data: d }); }

            self.v_data = new_v_data.err().unwrap();
        }

        Err(self)
    }

    unsafe fn unchecked_cast(self) -> DynRef<'a, X, Y> {
        let new_v_ref = self.v_ref.cast().unwrap();

        let new_v_data = self.v_data.cast(self.v_ref).ok().unwrap();

        DynRef { v_ref: new_v_ref, v_data: new_v_data }
    }
}

impl<'a, T: ?Sized, S, X: ?Sized, Y> Cast<DynRefMut<'a, X, Y>> for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(mut self) -> Result<DynRefMut<'a, X, Y>, DynRefMut<'a, T, S>> {
        if let Some(r) = self.v_ref.cast() {
            let new_v_data = self.v_data.cast(self.v_ref);

            if let Ok(d) = new_v_data { return Ok(DynRefMut { v_ref: r, v_data: d }); }

            self.v_data = new_v_data.err().unwrap();
        }

        Err(self)
    }

    unsafe fn unchecked_cast(self) -> DynRefMut<'a, X, Y> {
        let new_v_ref = self.v_ref.cast().unwrap();

        let new_v_data = self.v_data.cast(self.v_ref).ok().unwrap();

        DynRefMut { v_ref: new_v_ref, v_data: new_v_data }
    }
}

