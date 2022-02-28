pub mod default {
    use crate::traits::{Hosts, System};
    use std::any::{Any, TypeId};
    use std::collections::BTreeMap;
    use crate::traits::{Context, GlobalState};
    use std::future::Future;
    use crate::errors::traits::{AllocError,ReduceError::NoSuchIndice};
    use futures::task::SpawnExt;
    use futures::FutureExt;
    use std::marker::PhantomData;


    pub struct Host {
        /// free ids
        last_id: bitmaps::Bitmap<1024>,
        /// a map from Systems to their (subscribers, states) and global states.
        data: typemap::TypeMap,
        /// a function for dropping an entity.
        drop_ent: Option<Box<dyn FnOnce(&mut Self,usize)>>,
        ///a futures runtime.
        runtime: futures::executor::ThreadPool,
    }

    pub struct SystemData<S: System<Host>> {
        state: S::State,
        messages: Vec<(usize,Option<S::Message>)>,
        data: BTreeMap<usize,S>,
        //TODO: get rid of boxing somehow
        future_handles: Vec<(usize,Box<dyn Future<Output = S::Message>>)>,
    }
    
    impl<S: System<Host>> Default for SystemData<S>{
        fn default() -> Self {
            Self{
                state: S::State::default(),
                messages: vec![],
                data: Default::default(),
                future_handles: vec![],
            }
        }
    }

    struct SystemHolder<S: System<Host> + Any>(PhantomData<S>);

    impl<S: System<Host> + Any + 'static> typemap::Key for SystemHolder<S>
    {
        type Value = SystemData<S>;
    }

    impl Host {
        pub fn new() -> Self {
            let runtime = futures::executor::ThreadPoolBuilder::new()
                .pool_size(4)
                .create().expect("failed to create a thread pool");
            Self {
                last_id: bitmaps::Bitmap::new(),
                data: typemap::TypeMap::new(),
                drop_ent: Some(Box::new(|_,_|{})),
                runtime,
            }
        }

        pub(crate) fn system_data<S: System<Host>>(&mut self) -> &mut SystemData<S> where Self: Hosts<S>
        {
            self.data.entry::<SystemHolder<S>>().or_insert_with(|| {
                S::State::init(self);
                Default::default()
            })
        }
    }

    impl crate::traits::Host for Host {
        type Indice = usize;

        fn allocate_entity(&mut self) -> Result<Self::Indice, crate::errors::traits::AllocError> {
            self.last_id.first_index().map(|idx| {self.last_id.set(idx,false); idx}).ok_or(crate::errors::traits::AllocError)
        }

        fn drop_entity(&mut self, which: Self::Indice) {
            (self.drop_ent.unwrap())(self,which);
        }

        fn register_entity_component_drop(&mut self, func: fn(&mut Self, Self::Indice)) {
            let mut payload: Box<dyn FnOnce(&mut Self,usize)>;
            let old_cb = self.drop_ent.take().expect("Ill-formed host");
            payload = Box::new(move |s,ind| {
                (old_cb)(s,ind);
                func(s,ind);
            });
            self.drop_ent = Some(payload);
        }
    }

    impl<S: crate::traits::System<Self>> Hosts<S> for Host
    {
        fn reduce(&mut self, which: Self::Indice) -> Result<(),crate::errors::traits::ReduceError> {
            let system = self.system_data::<S>();
            if let Some(state) = system.data.get_mut(&which) {
                system.messages.iter_mut()
                    .filter(|(el,_)| *el == which)
                    .map(|(_,el)| el)
                    .fold(state,|st,msg| {st.update(&mut system.state,msg.take().expect("found empty message"),&mut HostCtx{host: self}); state});
                Ok(())
            } else {
                Err(NoSuchIndice)
            }

        }

        fn get_state(&mut self, which: Self::Indice) -> Option<&mut S> {
            self.system_data::<S>()
                .data.get_mut(&which)
        }

        fn subscribe(&mut self, who: Self::Indice, with: <S as System<Self>>::Props) {
            let entry = self.system_data::<S>();
            entry.data.entry(who)
                .and_modify(|old| *old = S::changed(Some(old),&with))
                .or_insert(S::changed(None,&with));

        }

        fn unsubscribe(&mut self, who: Self::Indice) {
            use typemap::Entry::*;
            match self.data.entry::<SystemHolder<S>>() {
                Occupied(mut c) => {
                    let s = c.get_mut();
                    s.messages.retain(|(ind,_)| *ind != who);
                    s.data.remove(&who);
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

        fn send<S: System<Host>>(&mut self, msg: <S as System<Host>>::Message, whom: usize) where Host: Hosts<S> {
            self.host.system_data::<S>().messages.push((whom,Some(msg)))
        }

        fn spawn<T: 'static + Send, F, Fut, S: System<Host>>(&mut self, fut: Fut, f: F,whom: usize) where Fut: Future<Output=T> + Send + 'static, F: FnOnce(T) -> S::Message + 'static, Host: Hosts<S> {
            let data = self.host.system_data::<S>();
            let handle = self.host.runtime.spawn_with_handle(fut).unwrap().map(f);
            data.future_handles.push((whom, Box::new(handle) ));
        }

        fn state<S: System<Host>>(&'h mut self) -> &'h mut <S as System<Host>>::State where Host: Hosts<S> {
            &mut self.host.system_data::<S>().state
        }
    }
}
