use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn to_cbor<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let bytes = serde_cbor::to_vec(v)?;
    Ok(bytes)
}

pub fn from_cbor<T: DeserializeOwned>(b: &[u8]) -> Result<T> {
    let v = serde_cbor::from_slice(b)?;
    Ok(v)
}
