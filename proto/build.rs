fn main() {
    let proto = "src/state_store.proto";
    println!("cargo:rerun-if-changed={}", proto);

    tonic_build::configure()
        .compile(&[proto], &["src"])
        .unwrap();
}
