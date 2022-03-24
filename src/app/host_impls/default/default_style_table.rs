use std::path::{Path,PathBuf};
use crate::traits::render::{StyleShadow, StyleChange, Style, StyleTable};
use std::collections::HashMap;
use std::borrow::Borrow;

type RcCell<T> = std::rc::Rc<std::cell::RefCell<T>>;

pub struct DefaultStyleTable {
    inner: RcCell<Inner>,
}

impl DefaultStyleTable {
    fn new<'p>(data: impl Iterator<Item = (&'p Path,Style)>) -> Self {
        let mut inner = Inner {previous: None, rules: Default::default()};
        for (p,s) in data {
            inner.rules.insert(p.to_owned(),Some(s));
        };
        Self { inner: std::rc::Rc::new(std::cell::RefCell::new(inner)) }
    }
}

impl StyleTable for DefaultStyleTable {
    fn get(&self, which: &Path) -> Option<Style> {
        std::cell::RefCell::borrow(&*self.inner).get(which)
    }

    fn update(&mut self, cmd: StyleChange) {
        Inner::update(&mut self.inner,cmd)
    }

    fn scope(&mut self, shadow_commands: &[StyleShadow]) -> Box<dyn StyleTable> {
        Box::new(Self { inner: Inner::scope(&mut self.inner,shadow_commands)})
    }
}

#[derive(Default)]
struct Inner {
    // `None` if it is root
    previous: Option<RcCell<Self>>,
    //if key contains `None` - it has been shadowed
    rules: HashMap<PathBuf,Option<Style>>,
}

impl Inner {
    fn get(&self, which: &Path) -> Option<Style> {
        match self.rules.get(which) {
            Some(Some(style)) => Some(*style),
            Some(None) => {
                if let Some(previous) = &self.previous {
                    std::cell::RefCell::borrow(&*previous).get(which)
                } else {
                    None
                }
            },
            None => None,
        }
    }

    fn scope(this: &mut RcCell<Self>, shadow_commands: &[StyleShadow]) -> RcCell<Self> {
        let mut new = Self::default();
        for  i in shadow_commands {
            new.rules.insert(i.0.to_owned(), None);
        };
        new.previous = Some(this.clone());
        std::rc::Rc::new(std::cell::RefCell::new(new))
    }

    fn update(this: &mut RcCell<Self>, cmd: StyleChange) {
        let mut this = this.borrow_mut();
        match cmd {
            StyleChange::OverwriteColor { what,color } => {
                if let Some(Some(style)) = this.rules.get_mut(what) {
                    style.color = color;
                }
            }
            StyleChange::OverwriteWeight { what,new_weight } => {
                if let Some(Some(style)) =  this.rules.get_mut(what) {
                    style.weight = new_weight;
                }
            }
            StyleChange::AppendStyle {what,style} => {
                this.rules.insert(what.to_owned(),Some(style));
            }
        };
    }
}