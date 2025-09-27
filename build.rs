extern crate vergen;
use vergen::BuildBuilder;

fn main() {
    println!("cargo:rerun-if-changed=main.rs");
    let build = BuildBuilder::default()
        .use_local(true)
        .build_timestamp(true)
        .build()
        .unwrap();
    vergen::Emitter::new()
        .add_instructions(&build)
        .unwrap()
        .emit()
        .unwrap();
}
