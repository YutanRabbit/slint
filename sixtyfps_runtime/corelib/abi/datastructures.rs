use core::ptr::NonNull;
use vtable::*;

#[vtable]
#[repr(C)]
pub struct ComponentVTable {
    /// Allocate an instance of this component
    pub create: extern "C" fn(*const ComponentVTable) -> VBox<ComponentVTable>,

    /// Destruct this component.
    pub drop: extern "C" fn(VRefMut<ComponentVTable>),

    /// Returns an array that represent the item tree
    pub item_tree: extern "C" fn(VRef<ComponentVTable>) -> *const ItemTreeNode,
}

/// From the ItemTreeNode and a ComponentImpl, you can get a pointer to the instance data
/// ItemImpl via the offset field.
pub struct ItemImpl;
// Example memory representation:
// offset| type | value
// 0     | f32 | x
// 4     | f32 | y
// ...
// 64    | CachedRenderingData | struct

#[repr(C)]
#[derive(Default)]
pub struct CachedRenderingData {
    /// Used and modified by the backend, should be initialized to 0 by the user code
    pub(crate) cache_index: usize,
    /// Set to false initially and when changes happen that require updating the cache
    pub(crate) cache_ok: bool,
}

/// The item tree is an array of ItemTreeNode representing a static tree of items
/// within a component.
#[repr(C)]
pub enum ItemTreeNode {
    /// Static item
    Item {
        /// byte offset where we can find the item (from the *ComponentImpl)
        offset: isize,
        /// virtual table of the item
        vtable: *const ItemVTable,

        /// number of children
        chilren_count: u32,

        /// index of the first children within the item tree
        children_index: u32,
    },
    /// A placeholder for many instance of item in their own component which
    /// are instentiated according to a model.
    DynamicTree {
        /// Component vtable.
        /// This component is going to be instantiated as many time as the model tells
        component_type: *const ComponentVTable,

        /// vtable of the model
        model_type: *const super::model::ModelType,

        /// byte offset of the ModelImpl within the component.
        /// The model is an instance of the model described by model_type and must be
        /// stored within the component
        model_offset: isize,

        /// byte offset of the vector of components within the parent component
        /// (ComponentVec)
        /// a ComponentVec must be stored within the component to represent this tree
        components_holder_offset: isize,
    },
}

/// It is supposed to be in static array
unsafe impl Sync for ItemTreeNode {}

#[repr(C)]
#[derive(Default)]
pub struct ItemVTable {
    /// Rectangle: x/y/width/height ==> (path -> vertices/indicies(triangle))
    pub geometry: Option<unsafe extern "C" fn(*const ItemImpl) -> ()>, // like kurbo::Rect

    /// offset in bytes fromthe *const ItemImpl.
    /// isize::MAX  means None
    pub cached_rendering_data_offset: isize,

    /// Return a rendering info
    pub rendering_info: Option<unsafe extern "C" fn(*const ItemImpl) -> RenderingInfo>,

    /// We would need max/min/preferred size, and all layout info
    pub layouting_info: Option<unsafe extern "C" fn(*const ItemImpl) -> LayoutInfo>,

    /// input event
    pub input_event: Option<unsafe extern "C" fn(*const ItemImpl, MouseEvent)>,
}

// given an ItemImpl & ItemVTable
// (1) Identify that the item *is* a rectangle or has everything a rectangle would have
// (2) change the width

#[repr(C)]
pub struct LayoutInfo {
    min_size: f32,
    //...
    width_offset: isize,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub enum RenderingInfo {
    NoContents,
    Rectangle(f32, f32, f32, f32, u32), // Should be a beret structure
    Image(&'static str),
    /*Path(Vec<PathElement>),
    Image(OpaqueImageHandle, AspectRatio),
    Text(String)*/
}

type MouseEvent = ();

/* -- Safe wrappers*/

/*trait Item {
    fn geometry(&self) -> ();
    fn cached_rendering_data(&self) -> &CachedRenderingData;
    fn cached_rendering_data_mut(&mut self) -> &mut CachedRenderingData;
    fn rendering_info(&self) -> CachedRenderingData;
}*/

// To be used as &'x Item
pub struct Item {
    vtable: NonNull<ItemVTable>,
    inner: NonNull<ItemImpl>,
}

impl Item {
    /// One should create only one instance of item at the time to keep the &mut invariant
    pub unsafe fn new(vtable: NonNull<ItemVTable>, inner: NonNull<ItemImpl>) -> Self {
        Self { vtable, inner }
    }

    pub fn cached_rendering_data(&self) -> &CachedRenderingData {
        unsafe {
            &*((self.inner.as_ptr() as *const u8)
                .offset(self.vtable.as_ref().cached_rendering_data_offset)
                as *const CachedRenderingData)
        }
    }

    pub fn cached_rendering_data_mut(&mut self) -> &mut CachedRenderingData {
        unsafe {
            &mut *((self.inner.as_ptr() as *mut u8)
                .offset(self.vtable.as_ref().cached_rendering_data_offset)
                as *mut CachedRenderingData)
        }
    }

    pub fn rendering_info(&self) -> Option<RenderingInfo> {
        unsafe { self.vtable.as_ref().rendering_info.map(|x| x(self.inner.as_ptr())) }
    }
}

/// Visit each items recursively
///
/// The state parametter returned by the visitor is passed to each children.
pub fn visit_items<State>(
    component: VRef<'_, ComponentVTable>,
    mut visitor: impl FnMut(&Item, &State) -> State,
    state: State,
) {
    visit_internal(component, &mut visitor, 0, &state)
}

fn visit_internal<State>(
    component: VRef<'_, ComponentVTable>,
    visitor: &mut impl FnMut(&Item, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let item = unsafe {
                Item::new(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(
                        (VRef::get_ptr(&component).as_ptr()).offset(*offset) as *mut _,
                    ),
                )
            };
            let state = visitor(&item, state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal(component, visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

pub fn visit_items_mut<State>(
    component: VRefMut<'_, ComponentVTable>,
    mut visitor: impl FnMut(&mut Item, &State) -> State,
    state: State,
) {
    visit_internal_mut(component, &mut visitor, 0, &state)
}

fn visit_internal_mut<State>(
    mut component: VRefMut<'_, ComponentVTable>,
    visitor: &mut impl FnMut(&mut Item, &State) -> State,
    index: isize,
    state: &State,
) {
    let item_tree = component.item_tree();
    match unsafe { &*item_tree.offset(index) } {
        ItemTreeNode::Item { vtable, offset, children_index, chilren_count } => {
            let mut item = unsafe {
                Item::new(
                    NonNull::new_unchecked(*vtable as *mut _),
                    NonNull::new_unchecked(
                        (VRefMut::get_ptr(&component).as_ptr() as *mut u8).offset(*offset) as *mut _,
                    ),

                )
            };
            let state = visitor(&mut item, state);
            for c in *children_index..(*children_index + *chilren_count) {
                visit_internal_mut(component.borrow_mut(), visitor, c as isize, &state)
            }
        }
        ItemTreeNode::DynamicTree { .. } => todo!(),
    }
}

/*

/*
Button { visible: false; text: "foo"}

 -> ProxyWithVisibility<NativeItem>

// Qt style selected:
fn render_button(item: *const ItemImpl) -> RenderingInfo {
    let button = reinterpret_cast<&Button>(button)
    let text = b.text();
    let isPressed = b.isPressed();
    // ...
    let image = qt_render_button(width, height, isPressed, text)

    return RenderingInfo::Image(image)
}

// Basic style selected:

 -> Rectangle / Text

 fn render_rectangle(item: *const ItemImpl) -> RenderingInfo {
     let rect = reinterpret_cast<&Rectangle>(item)
     ...
     return RenderingInfo::Path(rect_path)
 }

 fn render_text(item: *const ItemImpl) -> RenderInfo {

 }

*/

// in corelib/primitives.rs

bitflags! {
    enum ItemExtensions {
        HasVisibility,
        HasOpacity
    }
}

struct ItemBase {
extensions: ItemExtensions,
x: Property<f32>,
y: Property<f32>,
// visible, opacity, ?
extraData: Vec<...>
}

impl ItemBase {
    pub fn is_visible(&self) -> bool {
        if self.extensions & HasVisibility {
            return self.extraData
        } else {
            return true;
        }
    }
}

#[derive(SixtyFpsItem)]
/// ```
/// width: f32
/// height: f32
/// ```
fn render_rectangle(item: *const ItemImpl) -> RenderingInfo {
    //let width = property_at_offset(item, 1);
    let rect : &Rectnalge = unsafe { std::mem::transmute(item) };
    let width = rect.width.get()
}


pub static RECTANGLE_VTABLE: ItemVTable = ItemVTable {
    rendering_info: render_rectangle,
}

// in styles/qt.rs

//#[derive(SixtyFpsItem)]
struct QtButton {
    text: String,
    is_pressed: bool,
}


pub static QT_BUTTON_VTABLE: ItemVTable = ItemVTable {
    rendering_info: render_qt_button,
};
*/
