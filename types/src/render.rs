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

impl<T> Point<T> {
    pub fn absolute(x: u32,y: u32) -> Point<u32> {
        Point(x,y)
    }
    pub fn relative(x: f32,y: f32) -> Point<f32>{
        Point(x,y)
    }
}

/// An rectangular of format (upper left corner,down right corner)
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Rect<L = u32,R =u32>(Point<L>,Point<R>);

//todo: add methods for converting relative to absolute points
impl<L,R> Rect<L,R> {
    pub fn zero() -> Rect<u32,u32> {
        Rect(Point::<u32>::absolute(0,0),Point::<u32>::absolute(0,0))
    }
    pub fn upper_left_absolute(self, p: Point<u32>) -> Rect<u32,R> {
        Rect(p,self.1)
    }
    pub fn upper_left_relative(self, p: Point<f32>) -> Rect<f32,R> {
        Rect(p,self.1)
    }
    pub fn down_right_absolute(self, p: Point<u32>) -> Rect<L,u32> {
        Rect(self.0,p)
    }
    pub fn down_right_relative(self, p: Point<f32>) -> Rect<L,f32> {
        Rect(self.0,p)
    }
}

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

/// An Visitor for producing render-able primitives
pub trait Visitor<P: Primitive> {
    /// A type for ctx of visitor
    type Ctx: Default;

    fn visit(&self, ctx: Self::Ctx) -> P;
}

/// A collection of types and methods for render necessary things
pub trait Primitive {
    type Color: Copy;
    /// Copy another primitive into a part of current one; edge cases ruled out as follows:
    /// * In case of `src` being smaller than `place` scaling up takes a place;
    /// * In case of `src` being larger than `place` `src` is first resized to fit given place
    fn copy_from(&mut self,place: Rect<f32>,src: Self);
    /// Copy a part of primitive
    fn cut(&self,part: Rect) -> Self;
    /// Rescale a primitive; `scale` is FP32 vec2.
    fn resize(&self,scale: (f32,f32)) -> Self;
    /// Associated function returning blank primitive;
    fn blank() -> Self;
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
/// todo: maybe add a method for setting previous table
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

/// this is API for placing components onto anchors
pub trait Renderer<H: Host + ?Sized> {
    /// Get a set of anchors, to which we are allowed to attach layouts
    fn anchors(&mut self) -> &[Anchor];
    /// We attach layouts to labels
    fn layout(&mut self, layout: Option<Layout<H>>, label: Anchor, z_index: ZIndex);
    /// Here we can interact with styling.
    fn styles(&self) -> &dyn StyleTable<H>;
    /// Change StyleTable entity vise, in a new scope.
    fn patch_style_scope(&mut self, patch: &mut dyn FnMut(&mut dyn StyleTable<H>));

}
