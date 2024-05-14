use ctor::ctor;

pub mod reexport {
    pub use ctor::ctor;
}

pub fn init() {}

#[ctor]
fn canary() {}
