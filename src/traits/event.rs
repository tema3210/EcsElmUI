pub trait Event: Clone {
    fn get_pos(&self) -> (u32,u32);
    fn mouse(&self) -> Option<MouseEvent>;
    fn keyboard(&self) -> Option<KeyboardEvent>;
    fn special(&self) -> Option<SpecialInput>;
}

pub enum MouseEvent {
    ButtonPress { which: u32 },
    Scroll(f64),
    At { x: u32,y: u32 },
}

// TODO: Finish this
pub enum KeyboardEvent {
    /// glyph and raw key values
    ButtonPress(char,u8),
    Signal(),
}

// TODO: Finish this
pub enum SpecialInput {
    DragNDrop()
}