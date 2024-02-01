extern crate capnp;

pub mod message_schema_capnp {
    include!(concat!(env!("OUT_DIR"), "/message_schema_capnp.rs"));
}