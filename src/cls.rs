use anyhow::{anyhow, Result};
use prost::Message;
use serde::de::DeserializeOwned;
use serde::Serialize;

use std::collections::HashMap;

use crate::proto::modal::client;
use crate::serialization::to_cbor;

/// A referenced Modal class (service function) with metadata and helper methods.
#[derive(Clone)]
pub struct Cls {
    pub service_function_id: String,
    pub service_function_metadata: Option<client::FunctionHandleMetadata>,
    pub client: crate::client::ModalClient,
}

/// An instantiated class with bound parameters. Methods map to function IDs.
#[derive(Clone)]
pub struct ClsInstance {
    methods: std::collections::HashMap<String, String>,
    client: crate::client::ModalClient,
}

impl crate::client::ModalClient {
    /// Lookup a class by name in an app and return a `Cls` with metadata.
    pub async fn cls_from_name(&mut self, app_name: &str, name: &str) -> Result<Cls> {
        let service_function_name = format!("{}.*", name);
        let req_msg = client::FunctionGetRequest {
            app_name: app_name.to_string(),
            object_tag: service_function_name,
            environment_name: String::new(),
        };
        let req = self.make_request(req_msg);
        let resp = self.stub.function_get(req).await?.into_inner();

        if resp.function_id.is_empty() {
            return Err(anyhow!("class not found"));
        }

        Ok(Cls {
            service_function_id: resp.function_id,
            service_function_metadata: resp.handle_metadata,
            client: self.clone(),
        })
    }
}

impl Cls {
    /// Create an instance of the class, binding the given parameters.
    /// Parameters map should contain values matching the class parameter schema.
    pub async fn instance(
        &mut self,
        parameters: HashMap<String, serde_cbor::Value>,
    ) -> Result<ClsInstance> {
        // If there is no parameter schema, the bound function id is the service function id.
        let mut function_id = self.service_function_id.clone();

        if let Some(ref metadata) = self.service_function_metadata {
            if let Some(ref param_info) = metadata.class_parameter_info.as_ref() {
                // proto value 2 == PARAM_SERIALIZATION_FORMAT_PROTO
                if param_info.format == 2 {
                    let schema = &param_info.schema;
                    if !schema.is_empty() {
                        let serialized = encode_parameter_set(schema, &parameters)?;
                        // Build bind params request
                        let bind_req = client::FunctionBindParamsRequest {
                            function_id: self.service_function_id.clone(),
                            serialized_params: serialized,
                            function_options: None,
                            environment_name: String::new(),
                            auth_secret: String::new(),
                        };
                        let req = self.client.make_request(bind_req);
                        let resp = self
                            .client
                            .stub
                            .function_bind_params(req)
                            .await?
                            .into_inner();
                        if !resp.bound_function_id.is_empty() {
                            function_id = resp.bound_function_id;
                        }
                    }
                }
            }
            // Build method map from metadata.method_handle_metadata
            let mut methods = HashMap::new();
            for (name, _m) in metadata.method_handle_metadata.iter() {
                // use the bound function id for methods (methods share same bound function id)
                methods.insert(name.clone(), function_id.clone());
            }

            return Ok(ClsInstance {
                methods,
                client: self.client.clone(),
            });
        }

        // No metadata -> no methods
        Err(anyhow!("class metadata missing"))
    }
}

impl ClsInstance {
    /// Call a method on the instance with a serde-serializable argument and decode the result.
    pub async fn call_method<T: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        args: &T,
    ) -> Result<R> {
        let func_id = self
            .methods
            .get(method)
            .ok_or_else(|| anyhow!("method not found"))?
            .clone();
        let cbor = to_cbor(args)?;
        let out = self.client.call_function_sync(&func_id, cbor).await?;
        let decoded: R = serde_cbor::from_slice(&out)?;
        Ok(decoded)
    }
}

fn encode_parameter_set(
    schema: &Vec<client::ClassParameterSpec>,
    parameters: &HashMap<String, serde_cbor::Value>,
) -> Result<Vec<u8>> {
    let mut encoded: Vec<client::ClassParameterValue> = Vec::new();
    for spec in schema.iter() {
        let name = spec.name.clone();
        let ptype = spec.r#type;
        let mut value = client::ClassParameterValue {
            name: name.clone(),
            r#type: ptype,
            value_oneof: None,
        };

        if let Some(v) = parameters.get(&name) {
            match v {
                serde_cbor::Value::Text(s) => {
                    value.value_oneof = Some(
                        client::class_parameter_value::ValueOneof::StringValue(s.clone()),
                    );
                }
                serde_cbor::Value::Integer(i) => {
                    value.value_oneof = Some(client::class_parameter_value::ValueOneof::IntValue(
                        *i as i64,
                    ));
                }
                serde_cbor::Value::Bool(b) => {
                    value.value_oneof =
                        Some(client::class_parameter_value::ValueOneof::BoolValue(*b));
                }
                serde_cbor::Value::Bytes(bs) => {
                    value.value_oneof = Some(
                        client::class_parameter_value::ValueOneof::BytesValue(bs.clone()),
                    );
                }
                _ => {
                    return Err(anyhow!("unsupported parameter value type for '{}'", name));
                }
            }
        } else if spec.has_default {
            // handle defaults where present by inspecting default_oneof
            if let Some(ref d) = spec.default_oneof.as_ref() {
                match d {
                    client::class_parameter_spec::DefaultOneof::StringDefault(s) => {
                        value.value_oneof = Some(
                            client::class_parameter_value::ValueOneof::StringValue(s.clone()),
                        );
                    }
                    client::class_parameter_spec::DefaultOneof::IntDefault(i) => {
                        value.value_oneof =
                            Some(client::class_parameter_value::ValueOneof::IntValue(*i));
                    }
                    client::class_parameter_spec::DefaultOneof::BytesDefault(b) => {
                        value.value_oneof = Some(
                            client::class_parameter_value::ValueOneof::BytesValue(b.clone()),
                        );
                    }
                    client::class_parameter_spec::DefaultOneof::BoolDefault(b) => {
                        value.value_oneof =
                            Some(client::class_parameter_value::ValueOneof::BoolValue(*b));
                    }
                    _ => {}
                }
            }
        } else {
            return Err(anyhow!("missing parameter '{}'", name));
        }

        encoded.push(value);
    }

    // sort by name to ensure deterministic serialization
    encoded.sort_by(|a, b| a.name.cmp(&b.name));

    let set = client::ClassParameterSet {
        parameters: encoded,
    };
    let mut buf = Vec::new();
    set.encode(&mut buf)?;
    Ok(buf)
}
