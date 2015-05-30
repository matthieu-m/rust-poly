//
//  Alright let's implement that DOM's example
//
use rtti::{Class,DynClass,Clonable};
use rtti::{DownCastRef,UpCast,UpCastRef};

//  KLUDGE: should be automatically implemented
use internal;
use internal::{ExtendTrait,ExtendStruct,FirstExtendTrait,FirstExtendStruct,TraitExtendTrait};

use std::boxed::Box;
use std::collections::HashMap;

//
//  ClassNode
//
type ClassNode = DynClass<Node, NodeData, (Clonable)>;

trait Node {}

#[derive(Clone, Debug)]
struct NodeData {
    parent: Option<Box<ClassNode>>,
    first_child: Option<Box<ClassNode>>,
}

impl Node for NodeData {}

//
//  ClassText
//
type ClassText = DynClass<Node, TextNode, (Clonable)>;

#[derive(Clone, Debug)]
struct TextNode {
    _first_parent: NodeData,
}

impl Node for TextNode {}


//
//  ClassElement
//
type ClassElement = DynClass<Element, ElementData, (Clonable)>;

trait Element: Node {
    fn do_the_thing(&self);

    fn before_set_attr(&mut self, _key: &str, _val: &str) {}
    fn after_set_attr(&mut self, _key: &str, _val: &str) {}
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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
    let text_node: Box<ClassText> = {
        let nd = NodeData { parent: None, first_child: None };
        Box::new(Class::new(TextNode { _first_parent: nd }).into_clonable()).into()
    };
    println!("text_node built");

    let video_element: Box<DynClass<Element, HTMLVideoElement, (Clonable)>> = {
        let tn = text_node.up_cast();
        let nd = NodeData { parent: None, first_child: Some(tn) };
        let mut ed = ElementData { _first_parent: nd, attrs: HashMap::new() };
        ed.set_attribute("it is", "so used");
        let hve = HTMLVideoElement { _first_parent: ed, cross_origin: false };
        Box::new(Class::new(hve).into_clonable()).into()
    };
    println!("video_element built");

    process_any_element((*video_element).as_trait());

    let child_node = video_element.as_struct()._first_parent._first_parent.first_child.as_ref().unwrap();

    for node in [child_node, video_element.up_cast_ref()].iter() {
        if let Some(text) = { let t: Option<&ClassText> = (*node).down_cast_ref(); t } {
            println!("I got me some text node {:?}", &text);
        } else if let Some(element) = { let t: Option<&ClassElement> = (*node).down_cast_ref(); t } {
            println!("I got me some element {:?}", &element);
            element.do_the_thing();
        } else {
            println!("Oh shoot, nothing I know!");
        }
    }

    println!("I haz teh clone: {:?}", child_node.clone());
}


//
//  KLUDGE: Hand-rolled marker traits for traits
//
macro_rules! extend_trait(
    ( $X:ty ) => {
        unsafe impl ExtendTrait<$X> for $X {}
        unsafe impl FirstExtendTrait<$X> for $X {}
        unsafe impl TraitExtendTrait<$X> for $X { fn offset() -> isize { 0 } }
    };
    ( $X:ty : $( $e:ty => $o:expr ),* ) => {
        extend_trait!($X);
        $(
            unsafe impl ExtendTrait<$e> for $X {}
            unsafe impl FirstExtendTrait<$e> for $X {}
            unsafe impl TraitExtendTrait<$e> for $X {
                fn offset() -> isize { use std; $o * std::mem::size_of::<internal::VTable>() as isize }
            }
        )*
    };
);

extend_trait!(Node);
extend_trait!(Element: Node => 0);

//
//  KLUDGE: Hand-rolled marker traits for structs
//
macro_rules! extend_struct(
    ( $X:ty ) => {
        unsafe impl ExtendStruct<()> for $X { fn offsets() -> &'static [isize] { static ZERO: [isize; 1] = [0]; &ZERO} }
        unsafe impl FirstExtendStruct<()> for $X {}

        unsafe impl ExtendStruct<$X> for $X { fn offsets() -> &'static [isize] { static ZERO: [isize; 1] = [0]; &ZERO} }
        unsafe impl FirstExtendStruct<$X> for $X {}
    };
    ( $X:ty : $( $e:ty),* ) => {
        extend_struct!($X);
        $(
            unsafe impl ExtendStruct<$e> for $X { fn offsets() -> &'static [isize] { static ZERO: [isize; 1] = [0]; &ZERO} }
            unsafe impl FirstExtendStruct<$e> for $X {}
        )*
    };
);

extend_struct!(NodeData);

unsafe impl ExtendTrait<Node> for NodeData {}

extend_struct!(TextNode: NodeData);

unsafe impl ExtendTrait<Node> for TextNode {}

extend_struct!(ElementData: NodeData);

unsafe impl ExtendTrait<Node> for ElementData {}
unsafe impl ExtendTrait<Element> for ElementData {}

extend_struct!(HTMLImageElement: NodeData, ElementData);

unsafe impl ExtendTrait<Node> for HTMLImageElement {}
unsafe impl ExtendTrait<Element> for HTMLImageElement {}

extend_struct!(HTMLVideoElement: NodeData, ElementData);

unsafe impl ExtendTrait<Node> for HTMLVideoElement {}
unsafe impl ExtendTrait<Element> for HTMLVideoElement {}

//
//  KLUDGE: Hand-rolled v-tables, and their init functions.
//
//  It's a bit ugly, but keeps from polluting the demo code
//  with distracting bits.
//
pub fn register_struct_info(collector: &mut Vec<(internal::StructId, internal::StructInfo)>) {
    use core::marker;
    use core::mem;
    use core::ptr;
    use internal::{StructId, StructInfo, TraitId, VTable, struct_id, v_table_by_id};

    fn make<S>(off: fn (StructId) -> &'static [isize]) -> (StructId, StructInfo)
        where S: Clone + marker::Reflect + 'static
    {
        fn v_table<S>(id: TraitId) -> Option<&'static VTable>
            where S: marker::Reflect + 'static
        {
            v_table_by_id(id, struct_id::<S>())
        }

        fn clone<S>(src: *const u8, dst: *mut u8) -> ()
            where S: Clone
        {
            unsafe {
                let original: &S = mem::transmute(src);

                let clone = original.clone();

                let src: *const S = mem::transmute(&clone);
                let dst: *mut S = mem::transmute(dst);

                ptr::copy_nonoverlapping(src, dst, mem::size_of::<S>());

                mem::forget(clone);
            }
        }

        fn drop<S>(raw: *mut ())
            where S: marker::Reflect + 'static
        {
            unsafe {
                let s: *const S = mem::transmute(raw);
                ptr::read(s);
            }
        }

        (
            struct_id::<S>(),
            StructInfo::new::<S>(v_table::<S>, off, Some(clone::<S>), drop::<S>)
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

pub fn register_trait_info(collector: &mut Vec<(internal::TraitId, internal::TraitInfo)>) {
    use core::marker;
    use internal::{StructId, TraitId, TraitInfo, VTable, trait_id, v_table_by_id};

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

macro_rules! register_struct(
    ($tables:ident, $indices:ident, $S:ty, $HT:ty, $( $T:ty => $off:expr, )* ) => {
        {
            $tables.push(
                (
                    ( internal::trait_id::<$HT>(), internal::struct_id::<$S>(), ),
                    Box::new([make_vtable!($HT, $S) $(,make_vtable!($T, $S))* ]),
                )
            );
            $(
                $indices.push(
                    (
                        ( internal::trait_id::<$T>(), internal::struct_id::<$S>(), ),
                        ( internal::trait_id::<$HT>(), internal::struct_id::<$S>(), ),
                        $off,
                    )
                );
            )*
        }
    };
);

pub fn register_vtables(
    tables: &mut internal::VTableRegistryTables,
    indices: &mut internal::VTableRegistryIndices,
)
{
    register_struct!(tables, indices, NodeData, Node,);
    register_struct!(tables, indices, TextNode, Node,);

    register_struct!(tables, indices, ElementData, Element, Node => 1,);
    register_struct!(tables, indices, HTMLImageElement, Element, Node => 1,);
    register_struct!(tables, indices, HTMLVideoElement, Element, Node => 1,);
} // fn register_vtables

