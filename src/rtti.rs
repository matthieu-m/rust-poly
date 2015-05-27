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

use std;

use internal::{FirstExtendStruct, FirstExtendTrait, ExtendStruct, ExtendTrait};
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

struct VData<S>
    where S: marker::Reflect + 'static
{
    offset: isize,
    ptr: *mut (),
    _0: marker::PhantomData<*const S>,
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
              T: ExtendTrait<B> + marker::Reflect + 'static
    {
        if trait_id::<T>() == trait_id::<B>() { return *self; }

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

    pub fn cast<T: ?Sized, X: ?Sized>(&self) -> Option<UntypedVRef>
        where T: marker::Reflect + 'static,
              X: marker::Reflect + 'static
    {
        if trait_id::<T>() == trait_id::<X>() { return Some(*self); }

        self.v_table().cast_to_trait::<X>().map(|vt| {
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

impl<S> VData<S>
    where S: marker::Reflect + 'static
{
    unsafe fn new(offset: isize, ptr: *mut ()) -> VData<S> {
        VData { offset: offset, ptr: ptr, _0: marker::PhantomData }
    }

    fn offset(&self) -> isize { self.offset }

    unsafe fn as_struct<'a>(&self) -> &'a S { mem::transmute(self.ptr) }

    unsafe fn as_struct_mut<'a>(&self) -> &'a mut S { mem::transmute(self.ptr) }

    fn up_cast<P>(&self) -> VData<P>
        where S: ExtendStruct<P>,
              P: marker::Reflect + 'static,
    {
        if struct_id::<S>() == struct_id::<P>() { return unsafe { self.add_offset::<P>(0) }; }

        let offsets = <S as ExtendStruct<P>>::offsets();
        assert!(offsets.len() == 1, "Multiple offsets support not implemented yet");

        unsafe { self.add_offset::<P>(*offsets.get_unchecked(0)) }
    }

    fn down_cast<C, T: ?Sized>(&self, v_ref: VRef<T>) -> Option<VData<C>>
        where T: marker::Reflect + 'static,
              C: ExtendStruct<S> + marker::Reflect + 'static,
    {
        if struct_id::<S>() == struct_id::<C>() { return unsafe { Some(self.add_offset::<C>(0)) }; }

        if !v_ref.is::<C>() { return None; }

        let offsets = <C as ExtendStruct<S>>::offsets();
        assert!(offsets.len() == 1, "Support for diamond inheritance is not yet implemented!");

        unsafe { Some(self.add_offset::<C>(*offsets.get_unchecked(0))) }
    }

    fn cast<Y, T: ?Sized>(&self, v_ref: VRef<T>) -> Option<VData<Y>>
        where T: marker::Reflect + 'static,
              Y: marker::Reflect + 'static,
    {
        if struct_id::<S>() == struct_id::<Y>() { return unsafe { Some(self.add_offset::<Y>(0)) }; }

        let offsets = v_ref.struct_info().offsets(struct_id::<Y>());
        assert!(offsets.len() <= 1, "Support for diamond inheritance is not yet implemented!");

        unsafe { offsets.first().map(|&o| self.add_offset::<Y>(o)) }
    }

    unsafe fn add_offset<Y>(&self, o: isize) -> VData<Y>
        where Y: marker::Reflect + 'static
    {
        assert!(self.offset + o > 0, "Out-of-bounds offset");

        let new_ptr = mem::transmute(self.ptr as isize - o);
        VData { offset: self.offset + o, ptr: new_ptr, _0: marker::PhantomData }
    }
} // impl VData

impl<S> clone::Clone for VData<S>
    where S: marker::Reflect + 'static,
{
    fn clone(&self) -> Self {
        VData { offset: self.offset, ptr: self.ptr, _0: marker::PhantomData }
    }
}

impl<S> fmt::Debug for VData<S>
    where S: fmt::Debug + marker::Reflect + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "VData {{ offset: {:?}, ptr: {:?} }}",
            self.offset(),
            unsafe { self.as_struct::<'static>() }
        )
    }
}


//
//  Class, DynClass (& Dyn), DynRef, DynRefMut
//
#[repr(C)]
#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct DynRef<'a, T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    v_data: VData<S>,
    _0: marker::PhantomData<&'a S>
}

#[derive(Clone, Debug)]
pub struct DynRefMut<'a, T: ?Sized, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    v_ref: VRef<T>,
    v_data: VData<S>,
    _0: marker::PhantomData<&'a S>
}

//
//  Class
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
        unsafe { mem::transmute(self.data_ptr()) }
    }

    //  Invariant: data_ptr() = base_ptr() + offset_into_struct()
    fn data_ptr(&self) -> *mut () {
        unsafe {
            let base: *const u8 = mem::transmute(self);
            mem::transmute(base.offset(self.offset))
        }
    }

    fn offset_into_struct(&self) -> isize {
        let offsets = self.v_ref.struct_info().offsets(struct_id::<S>());
        assert!(offsets.len() == 1, "Support for diamond inheritance is not yet implemented!");

        unsafe { *offsets.get_unchecked(0) }
    }

    fn base_ptr(&self) -> *mut () {
        unsafe {
            let base: *const u8 = mem::transmute(self);
            mem::transmute(base.offset(self.offset - self.offset_into_struct()))
        }
    }

    fn up_cast_struct<P>(&self) -> isize
        where S: ExtendStruct<P>,
              P: marker::Reflect + 'static,
    {
        let current = unsafe { VData::<S>::new(self.offset, self.data_ptr()) };

        current.up_cast::<P>().offset
    }

    fn down_cast_struct<C>(&self) -> Option<isize>
        where C: ExtendStruct<S> + marker::Reflect + 'static,
    {
        let current = unsafe { VData::<S>::new(self.offset, self.data_ptr()) };

        current.down_cast::<C, T>(self.v_ref).map(|vd| vd.offset )
    }

    fn cast_struct<Y>(&self) -> Option<isize>
        where Y: marker::Reflect + 'static,
    {
        let current = unsafe { VData::<S>::new(self.offset, self.data_ptr()) };

        current.cast::<Y, T>(self.v_ref).map(|vd| vd.offset )
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
            v_ref.drop(mem::transmute(self.base_ptr()));
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
        let new_v_ref = self.v_ref.up_cast::<B>();

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
        let new_v_ref = self.v_ref.down_cast::<D>();

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
        let new_v_ref = self.v_ref.down_cast::<D>().unwrap();

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
        let is_trait_ok = self.v_ref.down_cast::<D>().is_some();

        let is_struct_ok = self.down_cast_struct::<C>().is_some();

        if is_trait_ok && is_struct_ok {
            Some(unsafe { mem::transmute(self) })
        } else {
            None
        }
    }

    fn down_cast_ref_mut(&mut self) -> Option<&mut DynClass<D, C>> {
        let is_trait_ok = self.v_ref.down_cast::<D>().is_some();

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

impl<T: ?Sized, S, X: ?Sized, Y> Cast<Box<DynClass<X, Y>>> for Box<DynClass<T, S>>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(self) -> Result<Box<DynClass<X, Y>>, Box<DynClass<T, S>>> {
        let new_v_ref = self.v_ref.cast::<X>();

        let new_offset = self.cast_struct::<Y>();

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(o)) = (new_v_ref, new_offset) {
            let mut s: Box<DynClass<X, Y>> = unsafe { mem::transmute(self) };
            s.v_ref = v;
            s.offset = o;
            Ok(s)
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_cast(self) -> Box<DynClass<X, Y>> {
        let new_v_ref = self.v_ref.cast::<X>().unwrap();

        let new_offset = self.cast_struct::<Y>().unwrap();

        let mut s: Box<DynClass<X, Y>> = mem::transmute(self);
        s.v_ref = new_v_ref;
        s.offset = new_offset;
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
    pub fn new(c: &'a DynClass<T, S>) -> DynRef<'a, T, S>{
        DynRef {
            v_ref: c.v_ref,
            v_data: unsafe { VData::new(c.offset_into_struct(), c.data_ptr()) },
            _0: marker::PhantomData
        }
    }

    pub fn as_trait(&self) -> &'a T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_struct(&self) -> &'a S {
        unsafe { self.v_data.as_struct::<'a>() }
    }
}

impl<'a, T: ?Sized, S> DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
{
    pub fn new(c: &'a mut DynClass<T, S>) -> DynRefMut<'a, T, S> {
        DynRefMut {
            v_ref: c.v_ref,
            v_data: unsafe { VData::new(c.offset_into_struct(), c.data_ptr()) },
            _0: marker::PhantomData
        }
    }

    pub fn as_trait(&self) -> &'a T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_trait_mut(&self) -> &'a mut T {
        unsafe {
            mem::transmute_copy(&raw::TraitObject {
                data:   mem::transmute(self.as_struct()),
                vtable: self.v_ref.v_table().table(),
            })
        }
    }

    pub fn as_struct(&self) -> &'a S {
        unsafe { self.v_data.as_struct::<'a>() }
    }

    pub fn as_struct_mut(&self) -> &'a mut S {
        unsafe { self.v_data.as_struct_mut::<'a>() }
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
    fn from(r: DynRefMut<T, S>) -> DynRef<'a, T, S> {
        DynRef { v_ref: r.v_ref, v_data: r.v_data, _0: marker::PhantomData }
    }
}


//
//  Casting
//
impl<'a, T: ?Sized, S, B: ?Sized, P> UpCast<DynRef<'a, B, P>> for DynRef<'a, T, S>
    where T: ExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> DynRef<'a, B, P> {
        //  Compute new v_ref and offset
        let new_v_ref = self.v_ref.up_cast();

        let new_v_data = self.v_data.up_cast();

        DynRef { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

impl<'a, T: ?Sized, S, B: ?Sized, P> UpCast<DynRefMut<'a, B, P>> for DynRefMut<'a, T, S>
    where T: ExtendTrait<B> + marker::Reflect + 'static,
          S: ExtendStruct<P> + marker::Reflect + 'static,
          B: marker::Reflect + 'static,
          P: marker::Reflect + 'static,
{
    fn up_cast(self) -> DynRefMut<'a, B, P> {
        //  Compute new v_ref and offset
        let new_v_ref = self.v_ref.up_cast();

        let new_v_data = self.v_data.up_cast();

        DynRefMut { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

impl<'a, T: ?Sized, S, D: ?Sized, C> DownCast<DynRef<'a, D, C>> for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: ExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(self) -> Result<DynRef<'a, D, C>, DynRef<'a, T, S>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast();

        let new_v_data = self.v_data.down_cast(self.v_ref);

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(d)) = (new_v_ref, new_v_data) {
            Ok(DynRef { v_ref: v, v_data: d, _0: marker::PhantomData })
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_down_cast(self) -> DynRef<'a, D, C> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast().unwrap();

        let new_v_data = self.v_data.down_cast(self.v_ref).unwrap();

        //  Commit result
        DynRef { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

impl<'a, T: ?Sized, S, D: ?Sized, C> DownCast<DynRefMut<'a, D, C>> for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          D: ExtendTrait<T> + marker::Reflect + 'static,
          C: FirstExtendStruct<S> + marker::Reflect + 'static,
{
    fn down_cast(self) -> Result<DynRefMut<'a, D, C>, DynRefMut<'a, T, S>> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast();

        let new_v_data = self.v_data.down_cast(self.v_ref);

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(d)) = (new_v_ref, new_v_data) {
            Ok(DynRefMut { v_ref: v, v_data: d, _0: marker::PhantomData })
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_down_cast(self) -> DynRefMut<'a, D, C> {
        //  Compute new v_ref and offset, while checking whether they do apply.
        let new_v_ref = self.v_ref.down_cast().unwrap();

        let new_v_data = self.v_data.down_cast(self.v_ref).unwrap();

        //  Commit result
        DynRefMut { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

impl<'a, T: ?Sized, S, X: ?Sized, Y> Cast<DynRef<'a, X, Y>> for DynRef<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(self) -> Result<DynRef<'a, X, Y>, DynRef<'a, T, S>> {
        let new_v_ref = self.v_ref.cast();

        let new_v_data = self.v_data.cast(self.v_ref);

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(d)) = (new_v_ref, new_v_data) {
            Ok(DynRef { v_ref: v, v_data: d, _0: marker::PhantomData })
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_cast(self) -> DynRef<'a, X, Y> {
        let new_v_ref = self.v_ref.cast().unwrap();

        let new_v_data = self.v_data.cast(self.v_ref).unwrap();

        DynRef { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

impl<'a, T: ?Sized, S, X: ?Sized, Y> Cast<DynRefMut<'a, X, Y>> for DynRefMut<'a, T, S>
    where T: marker::Reflect + 'static,
          S: marker::Reflect + 'static,
          X: marker::Reflect + 'static,
          Y: marker::Reflect + 'static,
{
    fn cast(self) -> Result<DynRefMut<'a, X, Y>, DynRefMut<'a, T, S>> {
        let new_v_ref = self.v_ref.cast();

        let new_v_data = self.v_data.cast(self.v_ref);

        //  Check whether the conversion makes sense,
        //  return the result appropriately.
        if let (Some(v), Some(d)) = (new_v_ref, new_v_data) {
            Ok(DynRefMut { v_ref: v, v_data: d, _0: marker::PhantomData })
        } else {
            Err(self)
        }
    }

    unsafe fn unchecked_cast(self) -> DynRefMut<'a, X, Y> {
        let new_v_ref = self.v_ref.cast().unwrap();

        let new_v_data = self.v_data.cast(self.v_ref).unwrap();

        DynRefMut { v_ref: new_v_ref, v_data: new_v_data, _0: marker::PhantomData }
    }
}

