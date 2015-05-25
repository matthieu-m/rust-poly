//
//  Alright let's implement that DOM's example
//
use rtti;
use rtti::{Class,DynClass};
use rtti::{DownCastRef,UpCast};

//  KLUDGE: should be automatically implemented
use rtti::{DerivedFromTrait,DerivedFromStruct,FirstDerivedFromTrait,FirstDerivedFromStruct};

use std::boxed::Box;
use std::collections::HashMap;


trait Node {}

#[derive(Debug)]
struct NodeData {
    parent: Option<Box<DynClass<Node, NodeData>>>,
    first_child: Option<Box<DynClass<Node, NodeData>>>,
}

impl Node for NodeData {}

#[derive(Debug)]
struct TextNode {
    _first_parent: NodeData,
}

impl Node for TextNode {}


trait Element: Node {
    fn do_the_thing(&self);

    fn before_set_attr(&mut self, _key: &str, _val: &str) {}
    fn after_set_attr(&mut self, _key: &str, _val: &str) {}
}

#[derive(Debug)]
struct ElementData {
    _first_parent: NodeData,
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
struct HTMLImageElement {
    _first_parent: ElementData,
}

impl Node for HTMLImageElement {}

impl Element for HTMLImageElement {
    fn do_the_thing(&self) { println!("HTMLImageElement is in da place!"); }

    fn before_set_attr(&mut self, key: &str, val: &str) {
        if key == "src" {
            // remove cached image
        }
        <ElementData as Element>::before_set_attr(&mut self._first_parent, key, val);
    }
}

#[derive(Debug)]
struct HTMLVideoElement {
    _first_parent: ElementData,
    cross_origin: bool,
}

impl Node for HTMLVideoElement {}

impl Element for HTMLVideoElement {
    fn do_the_thing(&self) { println!("HTMLVideoElement is in da place!"); }

    fn after_set_attr(&mut self, key: &str, value: &str) {
        if key == "crossOrigin" {
            self.cross_origin = value == "true";
        }
        <ElementData as Element>::after_set_attr(&mut self._first_parent, key, value);
    }
}

fn process_any_element<'a>(element: &'a Element) {
    println!("Process an element!");
    element.do_the_thing();
}

pub fn doit() {
    let text_node: Box<DynClass<Node, TextNode>> = {
        let nd = NodeData { parent: None, first_child: None };
        Box::new(Class::new(TextNode { _first_parent: nd })).into()
    };
    println!("text_node built");

    let video_element: Box<DynClass<Element, HTMLVideoElement>> = {
        let tn: Box<DynClass<Node, NodeData>> = text_node.up_cast();
        let nd = NodeData { parent: None, first_child: Some(tn) };
        let mut ed = ElementData { _first_parent: nd, attrs: HashMap::new() };
        ed.set_attribute("it is", "so used");
        let hve = HTMLVideoElement { _first_parent: ed, cross_origin: false };
        Box::new(Class::new(hve)).into()
    };
    println!("video_element built");

    process_any_element((*video_element).as_trait());

    let node = video_element.as_struct()._first_parent._first_parent.first_child.as_ref().unwrap();

    if let Some(text) = { let t: Option<&DynClass<Node, TextNode>> = (*node).down_cast_ref(); t } {
        println!("I got me some {:?}", &text);
    } else if let Some(element) = { let t: Option<&DynClass<Element, ElementData>> = (*node).down_cast_ref(); t } {
        println!("I got me some {:?}", &element);
    } else {
        println!("Oh shoot, nothing I know!");
    }
}


//
//  KLUDGE: Hand-rolled marker traits for traits
//
unsafe impl DerivedFromTrait<Node> for Node {}
unsafe impl FirstDerivedFromTrait<Node> for Node {}

unsafe impl DerivedFromTrait<Node> for Element {}
unsafe impl FirstDerivedFromTrait<Node> for Element {}
unsafe impl DerivedFromTrait<Element> for Element {}
unsafe impl FirstDerivedFromTrait<Element> for Element {}

//
//  KLUDGE: Hand-rolled marker traits for structs
//
unsafe impl DerivedFromStruct<NodeData> for NodeData {}
unsafe impl FirstDerivedFromStruct<NodeData> for NodeData {}

unsafe impl DerivedFromTrait<Node> for NodeData {}

unsafe impl DerivedFromStruct<NodeData> for TextNode {}
unsafe impl FirstDerivedFromStruct<NodeData> for TextNode {}
unsafe impl DerivedFromStruct<TextNode> for TextNode {}
unsafe impl FirstDerivedFromStruct<TextNode> for TextNode {}

unsafe impl DerivedFromTrait<Node> for TextNode {}

unsafe impl DerivedFromStruct<NodeData> for ElementData {}
unsafe impl FirstDerivedFromStruct<NodeData> for ElementData {}
unsafe impl DerivedFromStruct<ElementData> for ElementData {}
unsafe impl FirstDerivedFromStruct<ElementData> for ElementData {}

unsafe impl DerivedFromTrait<Node> for ElementData {}
unsafe impl DerivedFromTrait<Element> for ElementData {}

unsafe impl DerivedFromStruct<NodeData> for HTMLImageElement {}
unsafe impl FirstDerivedFromStruct<NodeData> for HTMLImageElement {}
unsafe impl DerivedFromStruct<ElementData> for HTMLImageElement {}
unsafe impl FirstDerivedFromStruct<ElementData> for HTMLImageElement {}
unsafe impl DerivedFromStruct<HTMLImageElement> for HTMLImageElement {}
unsafe impl FirstDerivedFromStruct<HTMLImageElement> for HTMLImageElement {}

unsafe impl DerivedFromTrait<Node> for HTMLImageElement {}
unsafe impl DerivedFromTrait<Element> for HTMLImageElement {}

unsafe impl DerivedFromStruct<NodeData> for HTMLVideoElement {}
unsafe impl FirstDerivedFromStruct<NodeData> for HTMLVideoElement {}
unsafe impl DerivedFromStruct<ElementData> for HTMLVideoElement {}
unsafe impl FirstDerivedFromStruct<ElementData> for HTMLVideoElement {}
unsafe impl DerivedFromStruct<HTMLVideoElement> for HTMLVideoElement {}
unsafe impl FirstDerivedFromStruct<HTMLVideoElement> for HTMLVideoElement {}

unsafe impl DerivedFromTrait<Node> for HTMLVideoElement {}
unsafe impl DerivedFromTrait<Element> for HTMLVideoElement {}

//
//  KLUDGE: Hand-rolled v-tables, and their init functions.
//
//  It's a bit ugly, but keeps from polluting the demo code
//  with distracting bits.
//
pub fn register_struct_info(collector: &mut Vec<(rtti::StructId, rtti::StructInfo)>) {
    use core::marker;
    use core::mem;
    use core::ptr;
    use rtti::{StructId, StructInfo, TraitId, VTable, struct_id, v_table_by_id};

    fn make<S>(off: fn (StructId) -> &'static [isize]) -> (StructId, StructInfo)
        where S: marker::Reflect + 'static
    {
        fn v_table<S>(id: TraitId) -> Option<&'static VTable>
            where S: marker::Reflect + 'static
        {
            v_table_by_id(id, struct_id::<S>())
        }

        fn drop<S>(raw: &mut ())
            where S: marker::Reflect + 'static
        {
            unsafe {
                let s: *const S = mem::transmute(raw);
                ptr::read(s);
            }
        }

        (
            struct_id::<S>(),
            StructInfo::new::<S>(v_table::<S>, off, drop::<S>)
        )
    } // make

    static NO_OFFSET: [isize; 0] = [];
    static OFFSET_ZERO: [isize; 1] = [0];

    fn offsets_of_node_data(id: StructId) -> &'static [isize] {
        if id == struct_id::<NodeData>() { &OFFSET_ZERO } else { &NO_OFFSET }
    }

    fn offsets_of_text_node(id: StructId) -> &'static [isize] {
        if id == struct_id::<TextNode>() { &OFFSET_ZERO } else { offsets_of_node_data(id) }
    }

    fn offsets_of_element_data(id: StructId) -> &'static [isize] {
        if id == struct_id::<ElementData>() { &OFFSET_ZERO } else { offsets_of_node_data(id) }
    }

    fn offsets_of_html_image_element(id: StructId) -> &'static [isize] {
        if id == struct_id::<HTMLImageElement>() { &OFFSET_ZERO } else { offsets_of_element_data(id) }
    }

    fn offsets_of_html_video_element(id: StructId) -> &'static [isize] {
        if id == struct_id::<HTMLVideoElement>() { &OFFSET_ZERO } else { offsets_of_element_data(id) }
    }

    collector.push(make::<NodeData>(offsets_of_node_data));
    collector.push(make::<TextNode>(offsets_of_text_node));
    collector.push(make::<ElementData>(offsets_of_element_data));
    collector.push(make::<HTMLImageElement>(offsets_of_html_image_element));
    collector.push(make::<HTMLVideoElement>(offsets_of_html_video_element));
} // fn register_struct_info

pub fn register_trait_info(collector: &mut Vec<(rtti::TraitId, rtti::TraitInfo)>) {
    use core::marker;
    use rtti::{StructId, TraitId, TraitInfo, VTable, trait_id, v_table_by_id};

    fn make<T: ?Sized>() -> (TraitId, TraitInfo)
        where T: marker::Reflect + 'static
    {
        fn v_table<T: ?Sized>(id: StructId) -> Option<&'static VTable>
            where T: marker::Reflect + 'static
        {
            v_table_by_id(trait_id::<T>(), id)
        }

        (
            trait_id::<T>(),
            TraitInfo::new::<T>(v_table::<T>)
        )
    } // make

    collector.push(make::<Node>());
    collector.push(make::<Element>());
} // fn register_trait_info

pub fn register_vtables(collector: &mut Vec<(rtti::TraitId, rtti::StructId, rtti::VTable)>) {
    collector.push(make_vtable_entry!(Node, NodeData));
    collector.push(make_vtable_entry!(Node, TextNode));

    collector.push(make_vtable_entry!(Node, ElementData));
    collector.push(make_vtable_entry!(Element, ElementData));

    collector.push(make_vtable_entry!(Node, HTMLImageElement));
    collector.push(make_vtable_entry!(Element, HTMLImageElement));

    collector.push(make_vtable_entry!(Node, HTMLVideoElement));
    collector.push(make_vtable_entry!(Element, HTMLVideoElement));
} // fn register_vtables

