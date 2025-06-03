use core::hash::Hash;

use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use hashbrown::HashSet;
use spin::rwlock::RwLock;

use crate::components::component::Component;

pub type ComponentRef = Rc<RwLock<Box<dyn Component>>>; // Referenz zu einer Komponente
pub type Stateful<T> = Rc<Signal<T>>;

pub trait ComponentRefExt {
    fn from_component(component: Box<dyn Component>) -> ComponentRef;
}

impl ComponentRefExt for ComponentRef {
    fn from_component(component: Box<dyn Component>) -> ComponentRef {
        Rc::new(RwLock::new(component))
    }
}

pub struct Signal<T> {
    value: RwLock<T>,
    dependents: RwLock<HashSet<HashedWeak<RwLock<Box<dyn Component>>>>>, // Abhängige Komponenten
    is_updating: RwLock<bool>,
}

impl<T: Clone> Signal<T> {
    pub fn new(value: T) -> Rc<Self> {
        Rc::new(Self {
            value: RwLock::new(value),
            dependents: RwLock::new(HashSet::new()),
            is_updating: RwLock::new(false),
        })
    }

    /// Registers a dependent component that will automatically be marked as 'dirty'
    /// when the value changes.
    pub fn register_component(&self, component: ComponentRef) {
        let weak = Rc::downgrade(&component);
        self.dependents.write().insert(HashedWeak::new(weak));
    }

    pub fn get(&self) -> T {
        self.value.read().clone()
    }

    /// Sets a new value and marks all dependent components as 'dirty'.
    ///
    /// Note: Make sure that you don't call this function while a lock to one of the
    /// dependent components is active (e.g. during an interaction), as this would result
    /// in a deadlock!
    pub fn set(&self, new_value: T) {
        let mut updating = self.is_updating.write();

        if *updating {
            return;
        }

        *updating = true;

        *self.value.write() = new_value;

        // Benachrichtige alle abhängigen Komponenten
        self.dependents.write().retain(|dependent| {
            if let Some(component) = dependent.upgrade() {
                component.write().mark_dirty();
                true
            } else {
                false
            }
        });

        *updating = false;
    }
}

// Wrapper für
pub struct HashedWeak<T: ?Sized> {
    inner: Weak<T>,
}

impl<T: ?Sized> HashedWeak<T> {
    pub fn new(inner: Weak<T>) -> Self {
        Self { inner }
    }

    pub fn upgrade(&self) -> Option<Rc<T>> {
        self.inner.upgrade()
    }
}

impl<T: ?Sized> Clone for HashedWeak<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: ?Sized> PartialEq for HashedWeak<T> {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: ?Sized> Eq for HashedWeak<T> {}

impl<T: ?Sized> Hash for HashedWeak<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.inner.as_ptr().hash(state);
    }
}
