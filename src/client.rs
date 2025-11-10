use anyhow::{anyhow, Result};
use bytes::Bytes;
use reqwest::Client as HttpClient;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};
use tonic::Request;

use crate::proto::modal::client::modal_client_client::ModalClientClient;
use crate::proto::modal::client::{
    DataFormat, FunctionGetOutputsRequest, FunctionGetRequest, FunctionInput, FunctionMapRequest,
    FunctionPutInputsItem, FunctionPutInputsRequest,
};
use crate::serialization::{from_cbor, to_cbor};

/// The main client for interacting with Modal's API.
///
/// This client handles authentication, serialization, and the RPC protocol details.
#[derive(Clone)]
pub struct ModalClient {
    pub stub: ModalClientClient<Channel>,
    http: HttpClient,
    max_inline: usize,
    token_id: Option<String>,
    token_secret: Option<String>,
}

impl ModalClient {
    /// Connect to the Modal control plane. If `server_url` is None, uses the same default as other SDKs: "https://api.modal.com:443".
    /// Create a client from the user's Modal configuration or environment.
    ///
    /// Lookup order:
    /// 1. ~/.modal.toml (or %USERPROFILE%\.modal.toml on Windows) - pick the profile with `active = true`,
    ///    or the first profile if none are active. Use its `token_id` and `token_secret`.
    /// 2. Environment variables: `MODAL_TOKEN_ID` and `MODAL_TOKEN_SECRET`.
    ///
    /// `MODAL_SERVER_URL` may be provided via env and will be forwarded into `connect()`; otherwise
    /// the default `https://api.modal.com:443` is used.
    pub async fn from_env() -> Result<Self> {
        // Allow server override from env
        let server_url = std::env::var("MODAL_SERVER_URL").ok();

        // Attempt to read ~/.modal.toml or %USERPROFILE%/.modal.toml
        let home = std::env::var("USERPROFILE")
            .ok()
            .or_else(|| std::env::var("HOME").ok());
        if let Some(home_dir) = home {
            let path = std::path::Path::new(&home_dir).join(".modal.toml");
            if path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(value) = toml::from_str::<toml::Value>(&contents) {
                        if let Some(table) = value.as_table() {
                            // Find active profile or fallback to first
                            let mut chosen: Option<&toml::value::Table> = None;
                            for (_k, v) in table.iter() {
                                if let Some(t) = v.as_table() {
                                    if chosen.is_none() {
                                        chosen = Some(t);
                                    }
                                    if let Some(active) = t.get("active").and_then(|a| a.as_bool())
                                    {
                                        if active {
                                            chosen = Some(t);
                                            break;
                                        }
                                    }
                                }
                            }

                            if let Some(profile) = chosen {
                                let token_id = profile
                                    .get("token_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let token_secret = profile
                                    .get("token_secret")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                return Self::connect(
                                    server_url.as_deref(),
                                    token_id.as_deref(),
                                    token_secret.as_deref(),
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }

        // Fallback to environment variables if no profile found
        let token_id = std::env::var("MODAL_TOKEN_ID").ok();
        let token_secret = std::env::var("MODAL_TOKEN_SECRET").ok();

        Self::connect(
            server_url.as_deref(),
            token_id.as_deref(),
            token_secret.as_deref(),
        )
        .await
    }

    /// Create a client with explicit configuration.
    ///
    /// # Arguments
    /// * `server_url` - The Modal API server URL. Defaults to https://api.modal.com:443
    /// * `token_id` - The Modal token ID for authentication
    /// * `token_secret` - The Modal token secret for authentication
    pub async fn connect(
        server_url: Option<&str>,
        token_id: Option<&str>,
        token_secret: Option<&str>,
    ) -> Result<Self> {
        let server = server_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("MODAL_SERVER_URL").ok())
            .unwrap_or_else(|| "https://api.modal.com:443".to_string());

        let endpoint = Endpoint::from_shared(server.clone())?;
        let channel = endpoint.connect().await?;

        let stub = ModalClientClient::new(channel.clone());

        Ok(Self {
            stub,
            http: HttpClient::new(),
            max_inline: 16 * 1024 * 1024,
            token_id: token_id
                .map(|s| s.to_string())
                .or_else(|| std::env::var("MODAL_TOKEN_ID").ok()),
            token_secret: token_secret
                .map(|s| s.to_string())
                .or_else(|| std::env::var("MODAL_TOKEN_SECRET").ok()),
        })
    }

    pub(crate) fn make_request<T>(&self, msg: T) -> Request<T> {
        let mut req = Request::new(msg);
        // Standard metadata used by other SDKs
        req.metadata_mut().insert(
            "x-modal-client-version",
            MetadataValue::from_static("1.0.0"),
        );
        req.metadata_mut()
            .insert("x-modal-client-type", MetadataValue::from_static("1"));

        if let Some(ref id) = self.token_id {
            if let Ok(mv) = MetadataValue::try_from(id.as_str()) {
                req.metadata_mut().insert("x-modal-token-id", mv);
            }
        }
        if let Some(ref secret) = self.token_secret {
            if let Ok(mv) = MetadataValue::try_from(secret.as_str()) {
                req.metadata_mut().insert("x-modal-token-secret", mv);
            }
        }

        req
    }

    /// Look up a deployed function by app name and object tag (function name)
    pub async fn function_get(&mut self, app_name: &str, object_tag: &str) -> Result<String> {
        let req_msg = FunctionGetRequest {
            app_name: app_name.to_string(),
            object_tag: object_tag.to_string(),
            environment_name: String::new(),
        };
        let req = self.make_request(req_msg);
        let resp = self.stub.function_get(req).await?.into_inner();
        if resp.function_id.is_empty() {
            Err(anyhow!("function not found"))
        } else {
            Ok(resp.function_id)
        }
    }

    /// Call a deployed function synchronously. `args_cbor` should be CBOR encoded bytes of the payload.
    /// This follows the control-plane flow: FunctionMap -> FunctionPutInputs (if needed) -> poll FunctionGetOutputs.
    pub async fn call_function_sync(
        &mut self,
        function_id: &str,
        args_cbor: Vec<u8>,
    ) -> Result<Vec<u8>> {
        // Build FunctionInput. For simplicity use DATA_FORMAT_CBOR and inline bytes if small enough.
        let data_format = DataFormat::Cbor as i32;
        let function_input = FunctionInput {
            args_oneof: Some(
                crate::proto::modal::client::function_input::ArgsOneof::Args(args_cbor.clone()),
            ),
            final_input: false,
            data_format,
            method_name: None,
        };

        let item = FunctionPutInputsItem {
            idx: 0,
            input: Some(function_input),
            r2_failed: false,
            r2_throughput_bytes_s: 0,
        };

        use crate::proto::modal::client::FunctionCallInvocationType as InvokeType;
        use crate::proto::modal::client::FunctionCallType as CallType;

        let map_msg = FunctionMapRequest {
            function_id: function_id.to_string(),
            parent_input_id: String::new(),
            return_exceptions: false,
            function_call_type: CallType::Unary as i32,
            pipelined_inputs: vec![item.clone()],
            function_call_invocation_type: InvokeType::Sync as i32,
            from_spawn_map: false,
        };
        let map_req = self.make_request(map_msg);
        let map_resp = self.stub.function_map(map_req).await?.into_inner();
        let function_call_id = map_resp.function_call_id;

        // If pipelined_inputs empty, we need to call FunctionPutInputs
        if map_resp.pipelined_inputs.is_empty() {
            let put_msg = FunctionPutInputsRequest {
                function_id: function_id.to_string(),
                function_call_id: function_call_id.clone(),
                inputs: vec![item],
            };
            let put_req = self.make_request(put_msg);
            let put_resp = self.stub.function_put_inputs(put_req).await?.into_inner();
            if put_resp.inputs.is_empty() {
                return Err(anyhow!(
                    "FunctionPutInputs returned no inputs - input queue full?"
                ));
            }
        }

        // Poll for outputs
        let mut attempts = 0u32;
        loop {
            let get_msg = FunctionGetOutputsRequest {
                function_call_id: function_call_id.clone(),
                max_values: 1,
                timeout: 5.0,
                last_entry_id: String::from("0-0"),
                clear_on_success: true,
                requested_at: 0.0,
                input_jwts: vec![],
                start_idx: Some(0),
                end_idx: Some(0),
            };
            let get_req = self.make_request(get_msg);
            let resp = self.stub.function_get_outputs(get_req).await?.into_inner();
            if !resp.outputs.is_empty() {
                let item = &resp.outputs[0];
                if let Some(ref result) = item.result.as_ref() {
                    match result.data_oneof {
                        Some(crate::proto::modal::client::function_result::DataOneof::Data(
                            ref data,
                        )) => {
                            return Ok(data.clone());
                        }
                        Some(
                            crate::proto::modal::client::function_result::DataOneof::DataBlobId(
                                ref blob_id,
                            ),
                        ) => {
                            // Fetch blob and return its bytes
                            let blob_req =
                                self.make_request(crate::proto::modal::client::BlobGetRequest {
                                    blob_id: blob_id.clone(),
                                });
                            let blob_resp = self.stub.blob_get(blob_req).await?.into_inner();
                            let download_url = blob_resp.download_url;
                            let resp = self.http.get(&download_url).send().await?;
                            let bytes = resp.bytes().await?.to_vec();
                            return Ok(bytes);
                        }
                        _ => {}
                    }
                    // Result received but no data - check for error
                    if !result.exception.is_empty() {
                        return Err(anyhow!("Remote exception: {}", result.exception));
                    } else if result.exitcode != 0 {
                        return Err(anyhow!("Remote exit code: {}", result.exitcode));
                    }
                }
            }
            attempts += 1;
            if attempts > 60 {
                return Err(anyhow!("timeout waiting for function output"));
            }
            sleep(Duration::from_millis(500)).await;
        }
    }
}
