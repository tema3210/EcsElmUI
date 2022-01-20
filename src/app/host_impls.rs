pub mod default {
    use crate::traits::{AllocError, Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;
    use crate::traits::Context;
    use traits::{NoSuchIndice, GlobalState};

    pub struct Host {
        /// free ids
        last_id: bitmaps::Bitmap<1024>,
        /// a map from Systems to their subscribers and global states.
        data: BTreeMap<TypeId,(BTreeMap<Self::Indice,Box<dyn Any>>, Box<dyn Any>)>,
    }

    impl Host {
        pub fn new() -> Self {
            Self {
                last_id: bitmaps::Bitmap::new(),
                data: BTreeMap::new(),
            }
        }
    }

    impl crate::traits::Host for Host {
        type Indice = usize;

        fn allocate_entity(&mut self) -> Result<Self::Indice, AllocError> {
            self.last_id.first_index().map(|idx| {self.last_id.set(idx,false); idx}).ok_or(AllocError)
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            for (_,m) in self.data.iter_mut() {
                m.remove(&which);
            };
            self.last_id.set(which,true);
        }
    }

    impl<'h,S: crate::traits::System<'h,Self>> Hosts<'h,S> for Host {
        fn reduce<'s, 'd>(&'h mut self, which: Self::Indice, with: &'d mut impl Iterator<Item=<S as System<'h, Self>>::Message>, ctx: &'s mut impl Context<'h, Self>) -> Result<(),crate::traits::NoSuchIndice> where 's: 'd, 'h: 's {
            self.data
                .get_mut(&std::any::TypeId::of::<S>())
                .map(|(map,gs)| (map.get_mut(&which).map(|d| d.downcast_mut::<S>()).flatten(),gs.downcast_mut::<S::State>()))
                .map(|arg| {
                    for msg in with {
                        match arg {
                            (Some(mut data),Some(state)) => S::update(&mut data,state,msg,ctx),
                            _ => {}
                        }
                    };
                })
                .ok_or(NoSuchIndice)
        }

        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.data
                .get_mut(&(std::any::TypeId::of::<S>()))
                .map(|m| m.get_mut(&which)).flatten()
                .map(|a| a.downcast_mut::<S>()).flatten()
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<'h, Self>>::Props) {
            self.data.entry(TypeId::of::<S>())
                .and_modify(|a| {
                    a.entry(who).and_modify(|a|{
                        (*a).downcast_mut::<S>().map(|it| S::changed(Some(it),&with));
                    });
                })
                .or_insert_with(|| {
                    let mut map =  BTreeMap::new();
                    map.insert(who, Box::new(S::changed(None,&with)) as Box<dyn Any>);
                    (map,Box::new(S::State::init()) as Box<dyn Any>)
                });
        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            self.data.entry(TypeId::of::<S>())
                .and_modify(|m| {m.remove(&who);});
        }
    }
}
