mod client;
mod proto;
mod serialization;

use crate::client::ModalClient;
use crate::serialization::to_cbor;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct EchoArgs {
    msg: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Example usage: read config from env
    let server = std::env::var("MODAL_SERVER_URL")
        .ok()
        .or_else(|| Some("https://api.modal.com:443".to_string()));
    let token_id = std::env::var("MODAL_TOKEN_ID").ok();
    let token_secret = std::env::var("MODAL_TOKEN_SECRET").ok();

    let mut client = ModalClient::connect(
        server.as_deref(),
        token_id.as_deref(),
        token_secret.as_deref(),
    )
    .await?;

    // Replace these with your deployed app and function names
    let app_name = std::env::var("MODAL_APP").unwrap_or_else(|_| "my-app".to_string());
    let function_name = std::env::var("MODAL_FUNCTION").unwrap_or_else(|_| "function".to_string());

    println!("Looking up function {}::{}", app_name, function_name);
    let function_id = client.function_get(&app_name, &function_name).await?;
    println!("Found function id {}", function_id);

    let args = EchoArgs {
        msg: "hello from rust".to_string(),
    };
    let cbor = to_cbor(&args)?;
    let out_bytes = client.call_function_sync(&function_id, cbor).await?;

    // Try to decode returned CBOR into a serde_json::Value for demo
    let decoded: serde_cbor::Value = serde_cbor::from_slice(&out_bytes)?;
    println!("Result from function: {:#?}", decoded);

    Ok(())
}
