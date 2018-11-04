#[cfg(not(feature = "thread"))]
mod inner {
    use std::{
        cell::RefCell,
        ops::{Deref, DerefMut},
        rc::Rc
    };
    #[derive(Debug)]
    pub struct RefCount<T>(Rc<T>);
    impl<T> RefCount<T> {
        pub fn new(inner: T) -> Self {
            RefCount(Rc::new(inner))
        }
    }
    impl<T> Clone for RefCount<T> {
        fn clone(&self) -> Self {
            RefCount(self.0.clone())
        }
    }
    impl<T> Deref for RefCount<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[derive(Debug)]
    pub struct Lock<T>(RefCell<T>);
    impl<T> Lock<T> {
        pub fn new(inner: T) -> Self {
            Lock(RefCell::new(inner))
        }
        pub fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a {
            self.0.borrow()
        }
        pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
            self.0.borrow_mut()
        }
    }
}
#[cfg(feature = "thread")]
mod inner {
    use std::{
        ops::{Deref, DerefMut},
        sync::{Arc, RwLock}
    };
    #[derive(Debug)]
    pub struct RefCount<T>(Arc<T>);
    impl<T> RefCount<T> {
        pub fn new(inner: T) -> Self {
            RefCount(Arc::new(inner))
        }
    }
    impl<T> Clone for RefCount<T> {
        fn clone(&self) -> Self {
            RefCount(self.0.clone())
        }
    }
    impl<T> Deref for RefCount<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[derive(Debug)]
    pub struct Lock<T>(RwLock<T>);
    impl<T> Lock<T> {
        pub fn new(inner: T) -> Self {
            Lock(RwLock::new(inner))
        }
        pub fn read<'a>(&'a self) -> impl Deref<Target = T> + 'a {
            self.0.read().unwrap()
        }
        pub fn write<'a>(&'a self) -> impl DerefMut<Target = T> + 'a {
            self.0.write().unwrap()
        }
    }
}
pub(crate) use self::inner::*;
