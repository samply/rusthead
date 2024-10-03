use std::{any::{Any, TypeId}, collections::HashMap, marker::PhantomData};


struct DepEntryInner {
    on_created: Vec<Box<dyn FnOnce(&mut dyn Any)>>
}

impl std::fmt::Debug for DepEntryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepEntryInner").field("on_created", &self.on_created.len()).finish()
    }
}

pub struct DepEntry<'a, T> {
    service: PhantomData<T>,
    inner: &'a mut DepEntryInner,
}

#[derive(Debug, Default)]
pub struct DepMap {
    type_map: HashMap<TypeId, DepEntryInner>
}

impl DepMap {
    pub fn ensure_installed<T: Any>(&mut self) -> DepEntry<'_, T> {
        let inner = self.type_map.entry(TypeId::of::<T>()).or_insert_with(|| DepEntryInner {
            on_created: Vec::new(),
        });
        DepEntry { service: PhantomData, inner }
    }
}

impl<'a, T: Any> DepEntry<'a, T> {
    pub fn with(&mut self, f: impl FnOnce(&mut T) + 'static) -> &mut Self {
        self.inner.on_created.push(Box::new(move |v| {
            f(v.downcast_mut().expect("Should work"))
        }));
        self
    }
}