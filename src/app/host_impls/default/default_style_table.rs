use std::path::{Path,PathBuf};
use crate::traits::{Host,render::{StyleShadow, StyleChange, Style, StyleTable}};
use std::collections::HashMap;
use std::borrow::Borrow;

type RcCell<T> = std::rc::Rc<std::cell::RefCell<T>>;

pub struct DefaultStyleTable<H: Host + ?Sized> {
    inner: RcCell<Inner<H>>,
}

impl<H: Host + ?Sized> DefaultStyleTable<H> {
    fn new<'p>(data: impl Iterator<Item = (&'p Path,Style<H>)>) -> Self {
        let mut inner = Inner {previous: None, rules: Default::default()};
        for (p,s) in data {
            inner.rules.insert(p.to_owned(),Some(s));
        };
        Self { inner: std::rc::Rc::new(std::cell::RefCell::new(inner)) }
    }
}

impl<H: Host + ?Sized + 'static> StyleTable<H> for DefaultStyleTable<H> {
    fn get(&self, which: &Path) -> Option<Style<H>> {
        std::cell::RefCell::borrow(&*self.inner).get(which)
    }

    fn update(&mut self, cmd: StyleChange<H>) {
        Inner::update(&mut self.inner,cmd)
    }

    fn scope(&mut self, shadow_commands: &[StyleShadow]) -> Box<dyn StyleTable<H>> {
        Box::new(Self { inner: Inner::scope(&mut self.inner,shadow_commands)})
    }
}

struct Inner<H: Host + ?Sized> {
    // `None` if it is root
    previous: Option<RcCell<Self>>,
    //if key contains `None` - it has been shadowed
    rules: HashMap<PathBuf,Option<Style<H>>>,
}

impl<H: Host + ?Sized> Default for Inner<H> {
    fn default() -> Self {
        Self {previous: None, rules: Default::default()}
    }
}

impl<H: Host + ?Sized> Inner<H> {
    fn get(&self, which: &Path) -> Option<Style<H>> {
        match self.rules.get(which) {
            Some(Some(style)) => {
                let ret = (*style).clone();
                Some(ret)
            },
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

    fn update(this: &mut RcCell<Self>, cmd: StyleChange<H>) {
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