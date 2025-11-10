use anyhow::Result;
use modal_rust::{Cls, ModalClient};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
struct Args {
    name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize client from modal profile (~/.modal.toml) or environment
    let mut client = ModalClient::from_env().await?;

    let app_name =
        std::env::var("MODAL_APP").unwrap_or_else(|_| "MyApp".to_string());
    let class_name = std::env::var("MODAL_CLASS").unwrap_or_else(|_| "MyClass".to_string());

    println!("Looking up class {}::{}", app_name, class_name);
    let mut cls = client.cls_from_name(&app_name, &class_name).await?;

    // Instantiate with parameters. Example uses a simple string parameter 'name'.
    let mut params = HashMap::new();
    params.insert(
        "name".to_string(),
        serde_cbor::Value::Text("example".to_string()),
    );

    let mut inst = cls.instance(params).await?;

    // Call a method named 'echo' on the instance with an argument that will be CBOR-serialized.
    let args = Args {
        name: "Hello".to_string(),
    };
    let resp: Args = inst.call_method("echo", &args).await?;

    println!("method response: {:?}", resp);
    Ok(())
}
