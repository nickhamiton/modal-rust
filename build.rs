fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use a vendored protoc binary so users don't need to install protoc system-wide.
    // This sets the PROTOC environment variable for prost/tonic build steps.
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .map_err(|e| format!("failed to locate vendored protoc: {}", e))?;
    std::env::set_var("PROTOC", protoc);

    // Compile the Modal proto into Rust types using tonic/prost.
    // Resolve paths relative to the crate manifest dir so the build works
    // whether the crate is used from a git dependency or a local path.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let proto_dir = std::path::Path::new(&manifest_dir).join("modal_proto");
    let api_proto = proto_dir.join("api.proto");

    // Tell cargo when to rerun the build script.
    println!("cargo:rerun-if-changed={}", api_proto.display());
    println!("cargo:rerun-if-changed={}", proto_dir.display());

    tonic_build::configure()
        .build_server(false)
        .compile(&[api_proto.to_str().expect("invalid proto path")], &[
            proto_dir.to_str().expect("invalid proto dir"),
        ])?;
    Ok(())
}
