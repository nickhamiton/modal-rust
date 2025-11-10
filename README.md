# modal_rust

Minimal Rust client skeleton for invoking deployed Modal functions.

How to build

You need protoc and a working Rust toolchain. The build script will invoke tonic-build to compile the `modal_proto/api.proto` into Rust types.

Set environment variables to point at your deployed Modal server and credentials (if needed):

```powershell
$env:MODAL_SERVER_URL = "https://api.modal.com:443"
$env:MODAL_TOKEN_ID = "..."
$env:MODAL_TOKEN_SECRET = "..."
$env:MODAL_APP = "my-app"
$env:MODAL_FUNCTION = "function"
cargo run -p modal_rust
```

Notes

- This crate is a minimal starting point: it uses CBOR for payloads and does not yet implement blob upload/download, input-plane, or robust retry logic. It demonstrates the control-plane sync call path (FunctionMap -> FunctionPutInputs -> FunctionGetOutputs).
