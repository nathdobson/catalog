#![allow(dead_code, unused_imports)]
#![feature(const_mut_refs)]
#![feature(thread_id_value)]
#![allow(unused_mut)]

//! An alternative to [ctor](https://crates.io/crates/ctor) and [inventory](https://crates.io/crates/inventory) that supports WASM.
//! ```
//! use catalog::Registry;
//!
//! # mod hax {
//! pub mod interface_crate {
//!     # use catalog::{Registry, Builder, BuilderFrom};
//!     # use catalog::register;
//!     # use std::collections::HashSet;
//!     pub struct Impls(HashSet<&'static str>);
//!     impl Builder for Impls {
//!         type Output = HashSet<&'static str>;
//!         fn new() -> Self { Impls(HashSet::new()) }
//!         fn build(self) -> Self::Output { self.0 }
//!     }
//!     impl BuilderFrom<&'static &'static str> for Impls{
//!         fn insert(&mut self, element: &'static &'static str) {
//!             self.0.insert(element);
//!         }
//!     }
//!     // Define a point where impls can be collected.
//!     pub static IMPLS : Registry<Impls> = Registry::new();
//!     // The original crate can add impls.
//!     #[register(IMPLS)]
//!     static VALUE: &str = "native";
//! }
//!
//! pub mod impl_crate {
//!     # use catalog::register;
//!     // External crates and modules can add impls.
//!     use super::interface_crate::IMPLS;
//!     #[register(IMPLS)]
//!     static VALUE: &str = "external";
//! }
//! # } //hax
//! # use hax::interface_crate;
//! # use hax::impl_crate;
//!
//! use interface_crate::IMPLS;
//! use catalog::register;
//! #[register(IMPLS)]
//! static VALUE: &str = "internal";
//! assert_eq!(*IMPLS, vec!["native", "external", "internal"].into_iter().collect());
//! ```

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::hint::black_box;
use std::ops::Deref;
use std::sync::Once;

pub use catalog_macros::register;
use cfg_if::cfg_if;
use parking_lot::Mutex;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use safe_once::cell::OnceCell;
use safe_once::sync::{LazyLock, OnceLock};

cfg_if!(
    if #[cfg(any(target_arch = "wasm32", target_arch = "wasi"))] {
        #[path = "wasm_imp.rs"]
        mod imp;
    } else {
        #[path = "ctor_imp.rs"]
        mod imp;
    }
);

pub mod reexport {
    pub use cfg_if;

    pub use crate::imp::reexport::*;
}

pub trait Builder {
    type Output;
    fn new() -> Self;
    fn build(self) -> Self::Output;
}

pub trait BuilderFrom<T> {
    fn insert(&mut self, element: T);
}

pub struct Registry<B: Builder> {
    inputs: Mutex<Option<Vec<fn(&mut B)>>>,
    output: OnceLock<B::Output>,
}

pub struct LazyEntry<T: 'static> {
    private: LazyLock<T>,
    public: LazyLock<&'static T>,
}

impl<T> LazyEntry<T> {
    #[doc(hidden)]
    pub const fn new(private: fn() -> T, public: fn() -> &'static T) -> Self {
        LazyEntry {
            private: LazyLock::new(private),
            public: LazyLock::new(public),
        }
    }
    #[doc(hidden)]
    pub fn __private(this: &Self) -> &T {
        &*this.private
    }
}

impl<B: Builder> Registry<B> {
    pub const fn new() -> Self {
        Registry {
            inputs: Mutex::new(Some(vec![])),
            output: OnceLock::new(),
        }
    }
    #[doc(hidden)]
    pub fn register(&self, entry: fn(&mut B)) {
        self.inputs
            .lock()
            .as_mut()
            .expect("Registry already initialized")
            .push(entry);
    }
}

impl<B: Builder> Deref for Registry<B> {
    type Target = B::Output;

    fn deref(&self) -> &Self::Target {
        self.output.get_or_init(|| {
            imp::init();
            let mut vec = self.inputs.lock().take().unwrap();
            vec.shuffle(&mut thread_rng());
            let mut result = B::new();
            for x in vec {
                x(&mut result);
            }
            result.build()
        })
    }
}

impl<T> Deref for LazyEntry<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.public.deref()
    }
}

impl<B: Builder> Debug for Registry<B>
where
    B::Output: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("output", self.deref())
            .finish_non_exhaustive()
    }
}

impl<A: Eq + Hash + Debug, B> BuilderFrom<(A, B)> for HashMap<A, B> {
    fn insert(&mut self, (k, v): (A, B)) {
        match self.entry(k) {
            Entry::Occupied(e) => panic!("{:?}", e.key()),
            Entry::Vacant(e) => {
                e.insert(v);
            }
        }
    }
}

impl<A, B> Builder for HashMap<A, B> {
    type Output = Self;
    fn new() -> Self {
        HashMap::new()
    }
    fn build(self) -> Self::Output {
        self
    }
}
