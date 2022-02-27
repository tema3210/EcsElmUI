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
        data: typemap::SendMap,
        /// a function for dropping an entity.
        drop_ent:  fn(&mut Self,usize),
    }

    pub struct SystemData<'h,S: System<'h,Host>> {
        state: S::State,
        messages: Vec<(usize,Option<S::Message>)>,
        data: BTreeMap<usize,S>,
    }

    impl<'a,S: System<'a,Host> + Any> typemap::Key for S {
        type Value = SystemData<'a,S>;
    }

    impl Host {
        pub fn new() -> Self {
            Self {
                last_id: bitmaps::Bitmap::new(),
                data: typemap::SendMap::new(),
                drop_ent: |_,_| {},
            }
        }

        pub(crate) fn system_data<S>(&mut self) -> &mut SystemData<S>{
            self.data.entry::<S>().or_insert(SystemData{state: S::State::init(self), ..Default::default()})
        }
    }

    impl crate::traits::Host for Host {
        type Indice = usize;

        fn allocate_entity(&mut self) -> Result<Self::Indice, crate::errors::traits::AllocError> {
            self.last_id.first_index().map(|idx| {self.last_id.set(idx,false); idx}).ok_or(crate::errors::traits::AllocError)
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            self.drop_ent(self,which);
        }

        fn register_entity_component_drop(&mut self, func: fn(&mut Self, Self::Indice)) {
            self.drop_ent = |s,ind| {
                self.drop_ent(ind);
                func(s,ind);
            }
        }
    }

    impl<'h,S: crate::traits::System<'h,Self>> Hosts<'h,S> for Host
        where Self: 'h,
    {
        fn reduce<'s, 'd>(&'h mut self, which: Self::Indice) -> Result<(),crate::errors::traits::ReduceError> where 's: 'd, 'h: 's {
            let system = self.system_data::<S>();
            if let Some(state) = system.data.get_mut(&which) {
                system.messages.iter_mut()
                    .filter(|(el,_)| el == which)
                    .map(|(_,el)| el)
                    .fold(state,|st,msg| {st.update(msg.take().except("found empty message")); state});
                Ok(())
            } else {
                Err(NoSuchIndice)
            }

        }

        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.system_data::<S>()
                .data.get_mut(&which)
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<'h, Self>>::Props) {
            let entry = self.system_data::<S>();
            entry.data.entry(who)
                .and_modify(|old| S::changed(Some(old),&with))
                .or_insert(S::changed(None,&with));

        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            use typemap::Entry::*;
            match self.data.entry::<S>() {
                Occupied(mut c) => {
                    c.get_mut().map(|s: &mut SystemData<S>| {
                        s.messages.retain(|(ind,_)| ind != who);
                        s.data.remove(&who);
                    })
                },
                _ => {},
            };
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
            self.host.system_data::<S>().messages.push((whom,Some(msg)))
        }

        fn spawn<T: 'static, F, Fut, S: System<'h, Host>>(&mut self, fut: Fut, f: F,whom: usize) where Fut: Future<Output=T> + 'static, F: Fn(T) -> S::Message + 'static, Host: Hosts<'h, S> {
            unimplemented!()
        }

        fn state<S: System<'h, Host>>(&'h mut self) -> &'h mut <S as System<'h, Host>>::State where Host: Hosts<'h, S> {
            &mut self.host.system_data::<S>().state
        }
    }
}
