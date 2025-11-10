fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use a vendored protoc binary so users don't need to install protoc system-wide.
    // This sets the PROTOC environment variable for prost/tonic build steps.
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .map_err(|e| format!("failed to locate vendored protoc: {}", e))?;
    std::env::set_var("PROTOC", protoc);

    // Compile the Modal proto into Rust types using tonic/prost.
    tonic_build::configure()
        .build_server(false)
        .compile(&["../modal_proto/api.proto"], &["../modal_proto"])?;
    Ok(())
}
