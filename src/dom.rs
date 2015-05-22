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
    element.do_the_thing();
}

pub fn doit() {
    let text_node: Box<DynClass<Node, TextNode>> = {
        let nd = NodeData { parent: None, first_child: None };
        Box::new(Class::new(TextNode { _first_parent: nd })).into()
    };

    let video_element: Box<DynClass<Element, HTMLVideoElement>> = {
        let tn: Box<DynClass<Node, NodeData>> = text_node.up_cast();
        let nd = NodeData { parent: None, first_child: Some(tn) };
        let mut ed = ElementData { _first_parent: nd, attrs: HashMap::new() };
        ed.set_attribute("it is", "so used");
        let hve = HTMLVideoElement { _first_parent: ed, cross_origin: false };
        Box::new(Class::new(hve)).into()
    };

    //  FIXME: cannot implement as_trait() generically
    //process_any_element((*video_element).as_trait());

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
    use rtti::{StructId, StructInfo, struct_id};

    fn insert_struct_info_for<S>(collector: &mut Vec<(StructId, StructInfo)>, is_first_derived: fn (StructId) -> bool)
        where S: marker::Reflect + 'static
    {
        let struct_info = StructInfo::new::<S>(is_first_derived);
        collector.push((struct_id::<S>(), struct_info));
    }

    fn is_first_derived_of_node_data(_: StructId) -> bool { false }

    fn is_first_derived_of_text_node(id: StructId) -> bool { id == struct_id::<NodeData>() }

    fn is_first_derived_of_element_data(id: StructId) -> bool { id == struct_id::<NodeData>() }

    fn is_first_derived_of_html_element(id: StructId) -> bool {
        id == struct_id::<ElementData>() || is_first_derived_of_element_data(id)
    }

    insert_struct_info_for::<NodeData>(collector, is_first_derived_of_node_data);
    insert_struct_info_for::<TextNode>(collector, is_first_derived_of_text_node);
    insert_struct_info_for::<ElementData>(collector, is_first_derived_of_element_data);
    insert_struct_info_for::<HTMLImageElement>(collector, is_first_derived_of_html_element);
    insert_struct_info_for::<HTMLVideoElement>(collector, is_first_derived_of_html_element);
} // fn register_struct_info

pub fn register_trait_info(collector: &mut Vec<(rtti::TraitId, rtti::TraitInfo)>) {
    use rtti::{TraitInfo, trait_id};
    collector.push((trait_id::<Node>(), TraitInfo { trait_id: trait_id::<Node>() }));
    collector.push((trait_id::<Element>(), TraitInfo { trait_id: trait_id::<Element>() }));
} // fn register_trait_info

pub fn register_vtables(collector: &mut Vec<(rtti::TraitId, rtti::StructId, rtti::VTable)>) {
    use std::mem::{transmute};
    use rtti::{TraitId, VTable, get_vtable_by_id, struct_id, trait_id};

    fn generic_drop<T>(raw: &mut ()) {
        let n: &mut T = unsafe { transmute(raw) };
        drop(n);
    }

    fn cast_node_impl_node_data(t: TraitId) -> Option<&'static VTable> {
        if t == trait_id::<Node>() { Some(get_vtable_by_id(t, struct_id::<NodeData>())) }
        else                       { None }
    }

    fn cast_node_impl_text_node(t: TraitId) -> Option<&'static VTable> {
        if t == trait_id::<Node>() { Some(get_vtable_by_id(t, struct_id::<TextNode>())) }
        else                       { None }
    }

    fn cast_node_impl_element_data(t: TraitId) -> Option<&'static VTable> {
        if t == trait_id::<Node>() || t == trait_id::<Element>() {
            Some(get_vtable_by_id(t, struct_id::<ElementData>()))
        } else {
            None
        }
    }
    
    fn cast_element_impl_element_data(t: TraitId) -> Option<&'static VTable> {
        cast_node_impl_element_data(t)
    }

    fn cast_node_impl_html_image_element(t: TraitId) -> Option<&'static VTable> {
        if t == trait_id::<Node>() || t == trait_id::<Element>() {
            Some(get_vtable_by_id(t, struct_id::<HTMLImageElement>()))
        } else {
            None
        }
    }

    fn cast_element_impl_html_image_element(t: TraitId) -> Option<&'static VTable> {
        cast_node_impl_html_image_element(t)
    }

    fn cast_node_impl_html_video_element(t: TraitId) -> Option<&'static VTable> {
        if t == trait_id::<Node>() || t == trait_id::<Element>() {
            Some(get_vtable_by_id(t, struct_id::<HTMLVideoElement>()))
        } else {
            None
        }
    }

    fn cast_element_impl_html_video_element(t: TraitId) -> Option<&'static VTable> {
        cast_node_impl_html_video_element(t)
    }

    let drop_node_data = generic_drop::<NodeData>;
    let drop_text_node = generic_drop::<TextNode>;
    let drop_element_data = generic_drop::<ElementData>;
    let drop_html_image_element = generic_drop::<HTMLImageElement>;
    let drop_html_video_element = generic_drop::<HTMLVideoElement>;

    collector.push(make_vtable_entry!(Node, NodeData, drop_node_data, cast_node_impl_node_data));
    collector.push(make_vtable_entry!(Node, TextNode, drop_text_node, cast_node_impl_text_node));

    collector.push(make_vtable_entry!(Node, ElementData, drop_element_data, cast_node_impl_element_data));
    collector.push(make_vtable_entry!(Element, ElementData, drop_element_data, cast_element_impl_element_data));

    collector.push(make_vtable_entry!(Node, HTMLImageElement, drop_html_image_element, cast_node_impl_html_image_element));
    collector.push(make_vtable_entry!(Element, HTMLImageElement, drop_html_image_element, cast_element_impl_html_image_element));

    collector.push(make_vtable_entry!(Node, HTMLVideoElement, drop_html_video_element, cast_node_impl_html_video_element));
    collector.push(make_vtable_entry!(Element, HTMLVideoElement, drop_html_video_element, cast_element_impl_html_video_element));
} // fn register_vtables

