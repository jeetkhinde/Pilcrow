#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated_routes.rs"));
}

pub use generated::*;
