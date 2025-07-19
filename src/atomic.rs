use std::{
    fmt::Debug,
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Atomic<T: Into<usize> + From<usize>> {
    inner: AtomicUsize,
    phantom: PhantomData<T>,
}

impl<T: Into<usize> + From<usize>> Atomic<T> {
    pub fn new<U: Into<T>>(value: U) -> Self {
        Self {
            inner: AtomicUsize::new(value.into().into()),
            phantom: PhantomData,
        }
    }

    pub fn load(&self, order: Ordering) -> T {
        T::from(self.inner.load(order))
    }

    pub fn store(&self, val: T, order: Ordering) {
        self.inner.store(val.into(), order)
    }

    pub fn fetch_add(&self, val: usize, order: Ordering) -> T {
        T::from(self.inner.fetch_add(val, order))
    }

    pub fn fetch_max(&self, val: T, order: Ordering) -> T {
        T::from(self.inner.fetch_max(val.into(), order))
    }
}

impl<T: Into<usize> + From<usize> + Default> Default for Atomic<T> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            phantom: PhantomData,
        }
    }
}

impl<T: Into<usize> + From<usize> + Debug> Debug for Atomic<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atomic")
            .field("inner", &self.load(Ordering::Relaxed))
            .finish()
    }
}
