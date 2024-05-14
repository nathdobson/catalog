#![allow(unused_imports)]
#![deny(unused_must_use)]
#![allow(unused_mut)]

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use catalog::{Builder, BuilderFrom, Registry};
use catalog_macros::register;
use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_browser);

struct TestRegistry(HashMap<&'static str, &'static str>);

struct TestRegistryLazy(Vec<&'static TestEntryLazy>);

struct TestEntry {
    key: &'static str,
    value: &'static str,
}

pub struct TestEntryLazy {
    cell: OnceLock<usize>,
}

static TEST_REGISTRY: Registry<TestRegistry> = Registry::new();

static TEST_REGISTRY_LAZY: Registry<TestRegistryLazy> = Registry::new();

impl Builder for TestRegistry {
    type Output = HashMap<&'static str, &'static str>;
    fn new() -> Self {
        TestRegistry(HashMap::new())
    }
    fn build(self) -> HashMap<&'static str, &'static str> {
        self.0
    }
}

impl Builder for TestRegistryLazy {
    type Output = ();
    fn new() -> Self {
        TestRegistryLazy(vec![])
    }
    fn build(mut self) {
        for (i, x) in self.0.into_iter().enumerate() {
            x.cell.set(i).unwrap();
        }
    }
}

impl BuilderFrom<TestEntry> for TestRegistry {
    fn insert(&mut self, element: TestEntry) {
        self.0.insert(element.key, element.value);
    }
}

impl BuilderFrom<&TestEntry> for TestRegistry {
    fn insert(&mut self, element: &TestEntry) {
        self.0.insert(element.key, element.value);
    }
}

impl BuilderFrom<&'static TestEntryLazy> for TestRegistryLazy {
    fn insert(&mut self, element: &'static TestEntryLazy) {
        self.0.push(element)
    }
}

impl TestEntryLazy {
    fn new() -> Self {
        TestEntryLazy {
            cell: OnceLock::new(),
        }
    }
}

#[register(TEST_REGISTRY)]
fn register_fn() -> TestEntry {
    TestEntry {
        key: "a",
        value: "a",
    }
}

#[register(TEST_REGISTRY, crate = catalog)]
static TEST_ENTRY: TestEntry = TestEntry {
    key: "c",
    value: "c",
};

#[register(TEST_REGISTRY_LAZY, lazy = true)]
static TEST_ENTRY_LAZY1: TestEntryLazy = TestEntryLazy::new();

#[register(TEST_REGISTRY_LAZY, lazy = true)]
static TEST_ENTRY_LAZY2: TestEntryLazy = TestEntryLazy::new();

mod foo {
    use catalog_macros::register;

    use crate::TestEntry;

    #[register(crate::TEST_REGISTRY)]
    fn register_fn2() -> TestEntry {
        crate::TestEntry {
            key: "b",
            value: "b",
        }
    }
}

#[wasm_bindgen_test::wasm_bindgen_test]
#[test]
fn test() {
    assert_eq!(
        *TEST_REGISTRY,
        vec![("a", "a"), ("b", "b"), ("c", "c")]
            .into_iter()
            .collect()
    );
    assert_eq!(
        vec![0, 1].into_iter().collect::<HashSet<_>>(),
        vec![
            *TEST_ENTRY_LAZY1.cell.get().unwrap(),
            *TEST_ENTRY_LAZY2.cell.get().unwrap()
        ]
        .into_iter()
        .collect()
    );
}
