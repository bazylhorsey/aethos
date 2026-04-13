use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A typed key-value store, similar to `anymap`. Used for assigns and private conn data.
#[derive(Default, Debug)]
pub struct TypeMap(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl TypeMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(val));
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    pub fn remove<T: Any + Send + Sync>(&mut self) -> Option<T> {
        self.0
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.0.contains_key(&TypeId::of::<T>())
    }
}
