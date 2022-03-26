use crate::traits::Host;
use std::ops::Deref;

/// StyleData
#[derive(Clone,Copy)]
pub struct Style {
    pub weight: u16,
    pub color: Color,
}

/// An (x,y) point
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct Point(u32,u32);

/// An RGBA color
#[derive(Clone,Copy,Hash,Eq, PartialEq)]
pub struct RGBAColor(u8,u8,u8,u8);

/// An abstract, dependent color
// TODO: maybe make bgc to be some kind of fragment shader?
#[derive(Clone,Copy)]
pub enum Color {
    Plain(RGBAColor),
    Functional(fn(Point)->RGBAColor)
}

//Command to place something in the layout
pub enum Filling<H: Host + ?Sized> {
    //We put there a component
    Component(H::Index,usize),
    //We directly paint something here //todo: make interactions with wgpu
    Data( fn(&mut dyn Painter,&dyn StyleTable) ),
    //Plain color
    Empty(Color),
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
    pub bgc: Color,
}

pub enum StyleChange<'p> {
    OverwriteColor {
        /// A path to modified style
        what: &'p std::path::Path,
        color: Color,
    },
    OverwriteWeight {
        /// A path to modified style
        what: &'p std::path::Path,
        new_weight: u16,
    },
    AppendStyle {
        /// A path to modified style
        what: &'p std::path::Path,
        style: Style,
    }
}

pub struct StyleShadow<'p>(pub &'p std::path::Path);

/// This is scoped API.
pub trait StyleTable {
    fn get(&self, which: &std::path::Path) -> Option<Style>;
    fn update(&mut self, cmd: StyleChange);
    fn scope(&mut self, shadow_commands: &[StyleShadow]) -> Box<dyn StyleTable>;
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
    fn style(&mut self) -> &mut dyn StyleTable;
    /// Set styling scope for consuming by underlying drawing process
    fn set_style_scope(&mut self, scope: Option<Box<dyn StyleTable>>);
}
