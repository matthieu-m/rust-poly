#![feature(alloc)]
#![feature(core)]

#[macro_use]
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
        rtti::init_struct_info_registry(tables);
    }
    {
        // KLUDGE
        let mut tables = Vec::new();
        dom::register_trait_info(&mut tables);
        // ...
        rtti::init_trait_info_registry(tables);
    }
    {
        // KLUDGE
        let mut tables = Vec::new();
        dom::register_vtables(&mut tables);
        // ...
        rtti::init_vtable_registry(tables);
    }

    dom::doit();
}
