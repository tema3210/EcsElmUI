use crate::traits::Host;

pub struct Font;
pub struct Texture<'t>(
    &'t [&'t [std::sync::atomic::AtomicU64]]
);

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
pub enum Color{
    Plain(RGBAColor),
    Functional(fn(Point)->RGBAColor)
}

pub trait Painter {
    fn texturize<'t>(&'t mut self, text: Texture<'t>);
    fn line(&mut self, point1: Point,point2: Point, color: Color);
    fn bezier_curve(&mut self, points: Vec<Point>, color: Color);

    fn text<'s,'c: 's>(&'c mut self,text: &'s str, font: &'c Font, color: Color, at: Point);
    fn rectangle(&mut self,left_upper: Point,right_lower: Point, fill: Color);
    fn figure(&mut self, points: Vec<Point>, at: Point, fill: Color);
}

//Command to place something in the layout
pub enum FillCommand<H: Host> {
    //We put there a component
    Component(H::Indice),
    //We directly paint something here
    Data( fn(&mut dyn Painter,&dyn StyleTable) ),
    //We don't touch this space.
    Empty,
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

pub struct Layout {
    /// Size
    pub dims: Viewport,
    /// Data format: (beginning of and itself a sub-layout)
    /// begins in left upper corner (x,y)
    parts: Vec<((u32,u32),Layout)>
}

impl Layout {
    pub fn carve(&mut self,cmd: CarveCommand) {
        let coefficient = match &cmd {
            CarveCommand::Up(s) => *s,
            CarveCommand::Down(s) => *s,
            CarveCommand::Left(s) => *s,
            CarveCommand::Right(s) => *s,
        };
        let dh = (self.dims.height as f32 * coefficient) as u32;
        let dw = (self.dims.width as f32 * coefficient) as u32;

        let new_part = Self {
            dims: match cmd {
                CarveCommand::Up(_) => {
                    Viewport {
                        height: dh,
                        width: self.dims.width,
                    }
                },
                CarveCommand::Down(_) => {
                    Viewport {
                        height: self.dims.height - dh,
                        width: self.dims.width,
                    }
                },
                CarveCommand::Left(_) => {
                    Viewport {
                        height: self.dims.height,
                        width: self.dims.width - dw,
                    }
                },
                CarveCommand::Right(_) => {
                    Viewport {
                        height: self.dims.height,
                        width: dw,
                    }
                },
            },
            parts: vec![],
        };
        let new_point = {
            match &cmd {
                CarveCommand::Up(_) => {
                    (0,0)
                },
                CarveCommand::Down(_) => {
                    (0,self.dims.height - dh)
                },
                CarveCommand::Left(_) => {
                    (0,0)
                },
                CarveCommand::Right(_) => {
                    (self.dims.width - dw,0)
                },
            }
        };
        let rest_point = {
            match &cmd {
                CarveCommand::Up(_) => {
                    (0,dh)
                },
                CarveCommand::Down(_) => {
                    (0,0)
                },
                CarveCommand::Left(_) => {
                    (dw,0)
                },
                CarveCommand::Right(_) => {
                    (0,0)
                },
            }
        };
        let resize_arg = {
            match &cmd {
                CarveCommand::Up(_) | CarveCommand::Down(_) => {
                    (1.0f32,(self.dims.height - dh) as f32 / self.dims.height as f32)
                },
                CarveCommand::Left(_) | CarveCommand::Right(_) => {
                    ((self.dims.width - dw) as f32 / self.dims.width as f32,1.0f32)
                },
            }
        };
        self.resize(resize_arg);
        *self = Self{dims: self.dims, parts: vec![
            (new_point,new_part), //new part
            (rest_point,unsafe {std::ptr::read(self)})] // old part
        };
    }

    pub fn inner_parts(&self) -> impl Iterator<Item=&Layout> {
        self.parts.iter().map(|t| &t.1)
    }

    pub fn inner_parts_mut(&mut self) -> impl Iterator<Item=&mut Layout> {
        self.parts.iter_mut().map(|t| &mut t.1)
    }

    fn resize(&mut self, dims: (f32,f32)) {
        self.dims.width = (self.dims.width as f32 * dims.0) as u32;
        self.dims.height = (self.dims.height as f32 * dims.1) as u32;

        self.parts.iter_mut().for_each(|(begin,cont)|{
            begin.0 = (begin.0 as f32 * dims.0) as u32;
            begin.1 = (begin.1 as f32 * dims.1) as u32;
            cont.resize(dims);
        })
    }
    pub fn viewport(&self) -> Viewport {
        self.dims.clone()
    }
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

/// This is scoped API.
pub trait StyleTable {
    fn get(&self, which: &std::path::Path) -> Style;
    fn update(&mut self, cmd: StyleCommand);
}

pub trait Renderer<H: Host>{
    /// Here we tell how much space we want to take from initial VP.
    /// This gets called again if carving failed.
    /// Returning `None` means that system doesn't need screen space.
    fn carve(&mut self, vp: Viewport) -> Option<CarveCommand>;
    /// Here we create a data layout for it.
    fn layout(&mut self, vp: Viewport) -> Layout;
    /// Here we can modify styling.
    fn style(&mut self,table: &dyn StyleTable) -> Vec<StyleCommand>;
    /// Here we fill data into our layout.
    /// This gets called for all sub layouts defined via `Renderer::layout`
    fn fill(&mut self,layout: &Layout) -> FillCommand<H>;
}
