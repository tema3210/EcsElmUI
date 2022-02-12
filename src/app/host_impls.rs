pub mod default {
    use crate::traits::{Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;
    use crate::traits::{Context, GlobalState};
    use std::future::Future;
    use crate::errors::traits::{AllocError,ReduceError::NoSuchIndice};

    pub struct Host {
        /// free ids
        last_id: bitmaps::Bitmap<1024>,
        /// a map from Systems to their (subscribers, states) and global states.
        data: BTreeMap<TypeId,(BTreeMap<usize,Box<dyn Any>>, Box<dyn Any>)>,
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

        fn allocate_entity(&mut self) -> Result<Self::Indice, crate::errors::traits::AllocError> {
            self.last_id.first_index().map(|idx| {self.last_id.set(idx,false); idx}).ok_or(crate::errors::traits::AllocError)
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            for (_,(m,_)) in self.data.iter_mut() {
                m.remove(&which);
            };
            self.last_id.set(which,true);
        }
    }

    impl<'h,S: crate::traits::System<'h,Self>> Hosts<'h,S> for Host {

        fn reduce<'s, 'd>(&'h mut self, which: Self::Indice, with: &'d mut impl Iterator<Item=<S as System<'h, Self>>::Message>, ctx: &'s mut impl Context<'h, Self>) -> Result<(),NoSuchIndice> where 's: 'd, 'h: 's {
            self.data
                .get_mut(&std::any::TypeId::of::<S>())
                .map(|(map,gs)| (map.get_mut(&which).map(|d| d.downcast_mut::<(S,Vec<S::Message>)>()).flatten().map(|st| &mut st.0),gs.downcast_mut::<S::State>()))
                .map(|mut arg| {
                    for msg in with {
                        match arg {
                            (Some(ref mut data),Some(ref mut state)) => S::update( data,state,msg,ctx),
                            _ => {
                                break;
                            }
                        }
                    };
                })
                .ok_or(NoSuchIndice)
        }

        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.data
                .get_mut(&(std::any::TypeId::of::<S>()))
                .map(|(m,_)| m.get_mut(&which)).flatten()
                .map(|a| a.downcast_mut::<(S,Vec<S::Message>)>()).flatten()
                .map(|st| &mut st.0)
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<'h, Self>>::Props) {
            self.data.entry(TypeId::of::<S>())
                .and_modify(|(a,_)| {
                    a.entry(who).and_modify(|a|{
                        (*a).downcast_mut::<(S,Vec<S::Message>)>().map(|st| &mut st.0).map(|it| S::changed(Some(it),&with));
                    });
                })
                .or_insert_with(|| {
                    let mut map =  BTreeMap::new();
                    let storage = (S::changed(None,&with),Vec::<S::Message>::new());
                    map.insert(who, Box::new(storage) as Box<dyn Any>);
                    (map,Box::new(S::State::init()) as Box<dyn Any>)
                });
        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            self.data.entry(TypeId::of::<S>())
                .and_modify(|(m,_)| {m.remove(&who);});
        }
    }

    pub struct HostCtx<'h> {
        host: &'h mut Host,
    }

    impl<'h> crate::traits::Context<'h,Host> for HostCtx<'h> {
        fn get_host(&mut self) -> &mut Host {
            self.host
        }

        fn send<S: System<'h, Host>>(&mut self, msg: <S as System<'h, Host>>::Message, whom: usize) where Host: Hosts<'h, S> {
            self.host.data.get_mut(&TypeId::of::<S>())
                .map(|(m,_)|{
                    m.get_mut(&whom).map(|it| it.downcast_mut::<(S,Vec<S::Message>)>()).flatten()
                        .map(|(_,ref mut msgs)| {msgs.push(msg)})
                });
        }

        fn spawn<T: 'static, F, Fut, S: System<'h, Host>>(&mut self, fut: Fut, f: F) where Fut: Future<Output=T> + 'static, F: Fn(T) -> S::Message + 'static, Host: Hosts<'h, S> {
            unimplemented!()
        }

        fn state<S: System<'h, Host>>(&'h mut self) -> &'h mut <S as System<'h, Host>>::State where Host: Hosts<'h, S> {
            self.host.data
                .get_mut(&TypeId::of::<S>())
                .map(|(_,s)| s)
                .map(|s| s.downcast_mut::<S::State>()).flatten().unwrap()
        }
    }
}
