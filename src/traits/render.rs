use crate::traits::Host;
use std::ops::Deref;

pub struct Font;

/// StyleData
#[derive(Clone,Copy)]
pub struct Style {
    pub weight: u16,
    pub color: Color,
}

/// An (x,y) point
#[derive(Clone,Copy)]
pub struct Point(u32,u32);

/// An RGBA color
#[derive(Clone,Copy)]
pub struct RGBAColor(u8,u8,u8,u8);

/// An abstract, dependent color
#[derive(Clone,Copy)]
pub enum Color {
    Plain(RGBAColor),
    Functional(fn(Point)->RGBAColor)
}

pub trait Painter {
    fn line(&mut self, point1: Point,point2: Point, color: Color);
    fn bezier_curve(&mut self, points: &[Point], color: Color);

    fn text<'s,'c: 's>(&'s mut self,text: &'s str, font: &'c Font, color: Color, at: Point);
    fn rectangle(&mut self,left_upper: Point,right_lower: Point, fill: Color);
    fn figure(&mut self, points: Vec<Point>, at: Point, fill: Color);
}

//Command to place something in the layout
pub enum Filling<H: Host> {
    //We put there a component
    Component(H::Index,usize),
    //We directly paint something here //todo: make interactions with wgpu
    Data( fn(&mut dyn Painter,&dyn StyleTable) ),
    //Plain color
    Empty(Color),
}

//A side and how much space we want to cut off.
pub enum CarveCommand {
    Up(f32),
    Down(f32),
    Left(f32),
    Right(f32),
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

// TODO: maybe make bgc to be some kind of fragment shader?
pub struct Layout<H: Host> {
    /// Size
    pub dims: Viewport,
    /// Data format: (beginning of and itself a sub-layout)
    /// begins in left upper corner (x,y)
    pub(crate) parts: Vec<(Point,Filling<H>)>,
    /// background color, can be transparent
    pub bgc: Color,
}

pub enum StyleCommand<'p> {
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
    Shadow {
        what: &'p std::path::Path,
    },
}

/// This is global API.
pub trait StyleTable {
    fn get(&self, which: &std::path::Path) -> Style;
    fn update(&mut self, cmd: StyleCommand);
}

#[derive(Clone)]
pub struct Anchor(std::borrow::Cow<'static,str>, Point);

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

pub trait Renderer<H: Host> {
    /// Get a set of anchors, to which we attach layouts
    fn anchors(&mut self) -> &[Anchor];
    /// We attach layouts to labels
    fn layout(&mut self, layout: Layout<H>, label: Anchor, z_index: u32);
    /// Here we can modify styling.
    fn style(&mut self,commands: &[StyleCommand]);
}
