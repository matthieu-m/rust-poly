#![feature(alloc)]
#![feature(core)]
#![feature(associated_consts)]

#[macro_use]
mod internal;
mod rtti;
mod dom;

extern crate alloc;
extern crate core;

fn main() {
    {
        // KLUDGE
        let mut tables = Vec::new();
        dom::register_struct_info(&mut tables);
        // ...
        internal::init_struct_info_registry(tables);
        println!("StructInfo registered");
    }
    {
        // KLUDGE
        let mut tables = Vec::new();
        dom::register_trait_info(&mut tables);
        // ...
        internal::init_trait_info_registry(tables);
        println!("TraitInfo registered");
    }
    {
        // KLUDGE
        let mut tables = Vec::new();
        dom::register_vtables(&mut tables);
        // ...
        internal::init_vtable_registry(tables);
        println!("VTable registered");
    }

    dom::doit();
}
