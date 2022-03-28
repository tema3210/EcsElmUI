use crate::traits::Host;
use std::ops::Deref;

/// StyleData
#[derive(Clone,Copy)]
pub struct Style<H: Host + ?Sized> {
    pub weight: u16,
    pub color: <<H as Host>::Primitive as Primitive>::Color,
}

/// An (x,y) point
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Point(u32,u32);

/// This is the type for describing level of a layout
/// Note, actual z-index used in rendering may differ from logical one because of portals, etc.
pub enum ZIndex {
    /// This is chosen when we want to position a layer below anything else
    Bottom,
    /// We know an absolute value of an z-index for current layout
    Current(usize),
    /// We want to override everything else in a layout
    Top,
}

/// Command to place something in the layout
pub enum Filling<H: Host + ?Sized> {
    //We put there a component
    Component(H::Index,usize),
    //We directly paint something here
    Data(H::Primitive),
    //Plain color
    Empty(<<H as Host>::Primitive as Primitive>::Color),
}

/// An Visitor for producing renderable primitives
/// todo: is THIS enough?
pub trait Visitor<P: Primitive> {
    fn visit(&self, result: &mut P);
}
/// A collection of types for render necessary things
pub trait Primitive {
    type Color;
}

#[derive(Copy,Clone)]
pub struct Viewport {
    pub height: u32,
    pub width: u32,
}

impl Viewport {
    /// width to height ratio
    fn ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

pub struct Layout<H: Host + ?Sized> {
    /// Size
    pub dims: Viewport,
    /// Data format: (beginning of and itself a sub-layout)
    /// begins in left upper corner (x,y)
    pub(crate) parts: Vec<(Point,Filling<H>)>,
    /// background color, can be transparent
    pub bgc: <<H as Host>::Primitive as Primitive>::Color,
}

pub enum StyleChange<'p,H: Host + ?Sized> {
    OverwriteColor {
        /// A path to modified style
        what: &'p std::path::Path,
        color: <<H as Host>::Primitive as Primitive>::Color,
    },
    OverwriteWeight {
        /// A path to modified style
        what: &'p std::path::Path,
        new_weight: u16,
    },
    AppendStyle {
        /// A path to modified style
        what: &'p std::path::Path,
        style: Style<H>,
    }
}

pub struct StyleShadow<'p>(pub &'p std::path::Path);

/// This is scoped API.
pub trait StyleTable<H: Host + ?Sized> {
    fn get(&self, which: &std::path::Path) -> Option<Style<H>>;
    fn update(&mut self, cmd: StyleChange<H>);
    fn scope(&mut self, shadow_commands: &[StyleShadow]) -> Box<dyn StyleTable<H>>;
}

#[derive(Clone,Hash,Eq, PartialEq)]
pub struct Anchor(pub std::borrow::Cow<'static,str>, pub Point);

impl Anchor {
    fn from<S: Into<String>>(s: S,p: Point) -> Self {
        Self(std::borrow::Cow::Owned(s.into()),p)
    }
    fn point(&self) -> Point {
        self.1
    }
}

impl Deref for Anchor {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

/// this is API for rendering components onto anchors
pub trait Renderer<H: Host + ?Sized> {
    /// Get a set of anchors, to which we attach layouts
    fn anchors(&mut self) -> &[Anchor];
    /// We attach layouts to labels
    fn layout(&mut self, layout: Layout<H>, label: Anchor, z_index: u32);
    /// Here we can interact with styling.
    fn style(&mut self) -> &mut dyn StyleTable<H>;
    /// Set styling scope for consuming by underlying drawing process
    fn set_style_scope(&mut self, scope: Option<Box<dyn StyleTable<H>>>);
}
