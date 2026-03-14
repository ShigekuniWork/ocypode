use std::{fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Since this is in crates/tools, we point back to crates/tests/certs
    let certs_dir = manifest_dir.join("../crates/certs");

    if !certs_dir.exists() {
        fs::create_dir_all(&certs_dir).expect("failed to create certs directory");
    }

    let cert_path = certs_dir.join("server.crt");
    let key_path = certs_dir.join("key.pem");

    println!("Generating dev certificates...");
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();

    fs::write(&cert_path, cert.cert.pem()).expect("failed to write server.crt");
    fs::write(&key_path, cert.signing_key.serialize_pem()).expect("failed to write key.pem");

    println!("Generated:");
    println!("  cert: {}", cert_path.display());
    println!("  key:  {}", key_path.display());
}
