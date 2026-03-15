use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let proto_root = PathBuf::from(manifest_dir).join("../../proto");
    let proto_file = proto_root.join("ocypode/pubsub/v1/pubsub.proto");

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&[proto_file], &[proto_root])?;

    Ok(())
}
