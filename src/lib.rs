//! Modal Rust client library for calling Modal deployed functions remotely.
//!
//! This library provides a Rust interface to call Modal functions that have been
//! deployed to Modal's cloud platform. It handles the serialization of arguments,
//! remote execution, and result deserialization.
//!
//! # Using as a dependency
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! modal-rust = { git = "https://github.com/your-username/modal-rust" }
//! ```
//!
//! # Authentication
//!
//! The client can be configured either through environment variables or explicit credentials:
//!
//! 1. Using environment variables (recommended):
//!    - Set `MODAL_TOKEN_ID` and `MODAL_TOKEN_SECRET` environment variables
//!    - Optionally set `MODAL_SERVER_URL` (defaults to https://api.modal.com:443)
//!
//! 2. Using explicit configuration:
//!    - Pass credentials directly to `ModalClient::connect()`
//!
//! 3. Using the Modal profile file (`~/.modal.toml`)
//!
//! The library will also look for a `~/.modal.toml` (on Windows: `%USERPROFILE%\.modal.toml`).
//! The file contains one or more TOML tables keyed by profile name, for example:
//!
//! ```toml
//! [myprofile]
//! token_id = "ak-..."
//! token_secret = "as-..."
//!
//! [other]
//! token_id = "ak-..."
//! token_secret = "as-..."
//! active = true
//! ```
//!
//! The client picks the profile with `active = true`, or the first profile if none are active.
//!
//! # Examples
//!
//! ```no_run
//! use modal_rust::ModalClient;
//! use serde::{Serialize, Deserialize};
//! use anyhow::Result;
//!
//! #[derive(Serialize, Deserialize)]
//! struct EchoArgs {
//!     msg: String,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let mut client = ModalClient::from_env().await?;
//!     
//!     let function_id = client.function_get("my-app", "echo").await?;
//!     let args = EchoArgs { msg: "hello".to_string() };
//!     
//!     let result: EchoArgs = client.call_function(&function_id, &args).await?;
//!     println!("Echo response: {}", result.msg);
//!     Ok(())
//! }
//! ```

mod client;
mod cls;
mod proto;
mod serialization;

// Re-export the main types
pub use client::ModalClient;
pub use cls::{Cls, ClsInstance};

// Convenience type alias
pub type Error = anyhow::Error;
