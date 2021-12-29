pub mod default {
    use crate::traits::{AllocError, Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;
    use crate::traits::Context;

    pub struct Host {
        last_id: u64,
        map: BTreeMap<TypeId,BTreeMap<<Self as crate::traits::Host>::Indice,Box<dyn Any>>>,
        states: BTreeMap<TypeId,Box<dyn Any>>,
    }

    impl crate::traits::Host for Host {
        type Indice = u64;

        fn allocate_entity(&mut self) -> Result<Self::Indice, AllocError> {
            let ret = self.last_id;
            if ret == u64::MAX {
                Err(crate::traits::AllocError)
            } else {
                self.last_id += 1;
                Ok(ret)
            }
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            for (_,m) in self.map.iter_mut() {
                m.remove(&which);
            }
        }
    }

    impl<'h,S: crate::traits::System<'h,Self>> Hosts<'h,S> for Host {
        fn reduce<'s, 'd>(&'h mut self, which: Self::Indice, with: &'d mut impl Iterator<Item=<S as System<'h, Self>>::Message>, ctx: &'s mut impl Context<'h, Self>) -> Result<(),crate::traits::NoSuchIndice> where 's: 'd, 'h: 's {
            let (map,states) = (&mut self.map,&mut self.states);
            let state: Option<&mut S> = map
                .get_mut(&(std::any::TypeId::of::<S>()))
                .map(|m| m.get_mut(&which)).flatten()
                .map(|a| a.downcast_mut::<S>()).flatten();
            let gs = states.get_mut(&TypeId::of::<S>()).map(|i| i.downcast_mut::<S::State>()).flatten();
            match (state,gs) {
                (Some(state),Some(global_state)) => {
                    with.for_each(move |msg| state.update(global_state,msg,ctx));
                    Ok(())
                },
                _ => {
                    Err(crate::traits::NoSuchIndice)
                },
            }
        }

        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.map
                .get_mut(&(std::any::TypeId::of::<S>()))
                .map(|m| m.get_mut(&which)).flatten()
                .map(|a| a.downcast_mut::<S>()).flatten()
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<'h, Self>>::Props) {
            self.map.entry(TypeId::of::<S>())
                .and_modify(|a| {
                    a.entry(who).and_modify(|a|{
                        (*a).downcast_mut::<S>().map(|it| S::changed(Some(it),&with));
                    });
                })
                .or_insert_with(|| {
                    let mut map =  BTreeMap::new();
                    map.insert(who, Box::new(S::changed(None,&with)) as Box<dyn Any>);
                    map
                });
        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            self.map.entry(TypeId::of::<S>())
                .and_modify(|m| {m.remove(&who);});
        }
    }
}
