use std::any::Any;
use std::collections::BTreeMap;
use crate::traits::Host;

pub mod default {
    use crate::traits::{AllocError, Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;

    pub struct Host {
        map: BTreeMap<TypeId,BTreeMap<<Self as crate::traits::Host>::Indice,Box<dyn Any>>>,
    }

    impl crate::traits::Host for Host {
        type Indice = u64;

        fn allocate_entity(&mut self) -> Result<Self::Indice, AllocError> {
            unimplemented!()
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            unimplemented!()
        }
    }

    impl<'h,S: crate::traits::System<'h,Self>> Hosts<'h,S> for Host {
        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.map
                .get_mut(&(std::any::TypeId::of::<S>()))
                .map(|m| m.get_mut(&which))
                .map(|a| a.downcast_mut::<S>()).flatten()
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<'h, Self>>::Props) {
            self.map.entry(TypeId::of::<S>())
                .and_modify(|a| {
                    a.entry(who).and_modify(|a|{
                        (*a).downcast_mut::<S>().map(|it| S::changed(Some(it),&with));
                    });
                })
                .or_insert_with(S::changed(None,&with));
        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            self.map.entry(TypeId::of::<S>())
                .and_modify(|m| {m.remove(&who);});
        }
    }
}
