use crate::traits::Host;
use std::ops::{Range};

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

impl Point<u32> {
    pub fn absolute(x: u32,y: u32) -> Self {
        Point(x,y)
    }
}

impl Point<f32> {
    pub fn relative(x: f32,y: f32) -> Self {
        Point(x,y)
    }
}

/// An rectangular of format (upper left corner,down right corner)
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Rect<L = u32,R =u32>(Point<L>,Point<R>);

impl Rect<f32,f32> {
    pub fn full_box() -> Self {
        Rect(Point::relative(0.,0.),Point::relative(0.,0.))
    }
}

impl Rect<u32,u32> {
    pub fn get_absolute_rect(&self, part: Rect<f32,f32>) -> Rect<u32,u32>{
        let ul = Point::<u32>::absolute(
            self.0.0 + ((self.1.0 - self.0.0) as f32 * part.0.0) as u32,
            self.0.1 + ((self.1.1 - self.0.1) as f32 * part.0.1) as u32
        );
        let dr = Point::<u32>::absolute(
            self.0.0 + ((self.1.0 - self.0.0) as f32 * part.1.0) as u32,
            self.0.1 + ((self.1.1 - self.0.1) as f32 * part.1.1) as u32
        );
        Rect::<(),()>::zero()
            .upper_left_absolute(ul)
            .down_right_absolute(dr)
    }

    pub fn get_viewport(&self) -> Viewport {
        let height = if self.1.1 >= self.0.1 { self.1.1 - self.0.1 } else { self.0.1 - self.1.1};
        let width = if self.1.0 >= self.0.0 { self.1.0 - self.0.0 } else { self.0.0 - self.1.0};
        Viewport {
            height,
            width,
        }
    }
}

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
    Current(isize),
    /// We want to override everything else in a layout
    Top,
}

impl ZIndex {
    pub fn normalize(&self,regarding: &mut Range<isize>) -> Option<isize> {
        match self {
            ZIndex::Bottom => {
                regarding.start -= 1;
                Some(regarding.start)
            }
            ZIndex::Current(c) => {
                if regarding.contains(c) {
                    Some(*c)
                } else {
                    None
                }
            }
            ZIndex::Top => {
                regarding.end += 1;
                Some(regarding.end)
            }
        }
    }
}

/// Command to place something in the layout
pub enum Filling<H: Host + ?Sized> {
    //We put there a component
    Component(H::Index,usize),
    //We directly paint something here
    Data(H::Primitive),
}

/// An Visitor for producing render-able primitives
pub trait Visitor<P: Primitive> {
    /// A type for ctx of visitor
    type Ctx: Default;

    fn visit(&self, ctx: Self::Ctx) -> P;
}

/// A collection of types and methods for render necessary things
pub trait Primitive {
    /// Type of color used with this primitive
    type Color: Copy;
    /// Copy another primitive into a part of current one; edge cases ruled out as follows:
    /// * In case of `src` being smaller than `place` scaling up takes a place;
    /// * In case of `src` being larger than `place` `src` is first resized to fit given place
    /// This operation should respect transparency of `src`
    fn copy_from(&mut self,place: Rect<f32,f32>,src: &Self);
    /// Copy a part of primitive
    fn cut(&self,part: Rect) -> Self;
    /// Rescale a primitive; `scale` is FP32 vec2.
    fn resize(&self,scale: (f32,f32)) -> Self;
    /// Associated function returning blank (an empty and fully transparent) primitive;
    fn blank(size: Viewport) -> Self;
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

    /// as point
    fn as_point(&self) -> Point<u32> {
        Point(self.width,self.height)
    }
}

pub struct Layout<H: Host + ?Sized> {
    /// Size (relative to entities viewport)
    pub dims: Rect<f32,f32>,
    /// Data format: (containment rect,its filling)
    /// begins in left upper corner (x,y)
    pub parts: Vec<(Rect<f32,f32>,Filling<H>)>,
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
