extern crate capnpc;

fn main () {
  capnpc::CompilerCommand::new()
    .output_path(std::env::var("OUT_DIR").unwrap())
    .src_prefix("schema/")
    .file("schema/message_schema.capnp")
    .run().unwrap();
}