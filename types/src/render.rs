use crate::traits::Host;
use std::ops::Deref;

/// StyleData
pub struct Style<H: crate::traits::Host + ?Sized> {
    pub weight: u16,
    pub color: <<H as crate::traits::Host>::Primitive as Primitive>::Color,
}

impl<H: Host + ?Sized> Clone for Style<H> {
    fn clone(&self) -> Self {
        Self {
            weight: self.weight,
            color: self.color,
        }
    }
}

impl<H: Host + ?Sized> Copy for Style<H> {}


/// An (x,y) point
/// * Integer types serve for absolute screen space addressing
/// * FP types server for logical addressing
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Point<T = u32>(T,T);

/// An rectangular of format (upper left corner,down right corner)
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Rect<L = u32,R =u32>(Point<L>,Point<R>);

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
pub trait Visitor<P: Primitive> {
    /// A type for ctx of visitor
    type Ctx: Default;

    fn visit(&self, result: &mut P, ctx: &mut Self::Ctx);
}

/// An edge of a Primitive
pub enum Edge {
    UpperLeft,
    UpperRight,
    DownLeft,
    DownRight,
}

/// A collection of types and methods for render necessary things
/// todo: think of introducing physical vs logical coordinates =>  logical
pub trait Primitive {
    type Color: Copy;
    /// Copy another primitive into a part of current one; edge cases ruled out as follows:
    /// * In case of `src` being smaller than `place` scaling up takes a place;
    /// * In case of `src` being larger than `place` `src` is first resized to fit given place
    fn copy_from(&mut self,place: Rect,src: Self);
    /// Copy a part of primitive
    fn cut(&self,part: Rect) -> Self;
    /// Rescale a primitive; `scale` is FP32 vec2.
    fn resize(&self,scale: (f32,f32)) -> Self;
}

/// A data structure describing absolute size of some part of screen space
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

/// this is API for placing components onto anchors
pub trait Renderer<H: Host + ?Sized> {
    /// Get a set of anchors, to which we attach layouts
    fn anchors(&mut self) -> &[Anchor];
    /// We attach layouts to labels
    fn layout(&mut self, layout: Layout<H>, label: Anchor, z_index: ZIndex);
    /// Here we can interact with styling.
    fn style(&mut self) -> &mut dyn StyleTable<H>;
    /// Set styling scope for consuming by underlying drawing process
    fn set_style_scope(&mut self, scope: Option<Box<dyn StyleTable<H>>>);
}
