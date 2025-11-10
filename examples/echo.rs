use anyhow::Result;
use modal_rust::ModalClient;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct EchoArgs {
    msg: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Method 1: Initialize from your Modal profile file (preferred)
    // The client looks for `$HOME/.modal.toml` (on Windows: `%USERPROFILE%\.modal.toml`) and
    // picks the profile with `active = true`, or the first profile if none are active.
    // Example `~/.modal.toml`:
    //
    // [default]
    // token_id = "ak-..."
    // token_secret = "as-..."
    //
    // [work]
    // token_id = "ak-..."
    // token_secret = "as-..."
    // active = true
    //
    // If no profile file is found, it falls back to the environment variables `MODAL_TOKEN_ID` and `MODAL_TOKEN_SECRET`.
    let mut client = ModalClient::from_env().await?;

    // Method 2: Initialize with explicit configuration
    // let mut client = ModalClient::connect(
    //     Some("https://api.modal.com:443"),
    //     Some("YOUR_TOKEN_ID"),
    //     Some("YOUR_TOKEN_SECRET")
    // ).await?;

    // Replace these with your deployed app and function names
    let app_name =
        std::env::var("image-api-qwen-fewsteps").unwrap_or_else(|_| "my-app".to_string());
    let function_name = std::env::var("MODAL_FUNCTION").unwrap_or_else(|_| "echo".to_string());

    println!("Looking up function {}::{}", app_name, function_name);
    let function_id = client.function_get(&app_name, &function_name).await?;
    println!("Found function id {}", function_id);

    // Call the remote function with a message
    let args = EchoArgs {
        msg: "hello from rust".to_string(),
    };
    let cbor = serde_cbor::to_vec(&args)?;
    let out_bytes = client.call_function_sync(&function_id, cbor).await?;
    let result: EchoArgs = serde_cbor::from_slice(&out_bytes)?;
    println!("Echo response: {}", result.msg);

    Ok(())
}
