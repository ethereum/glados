use std::io::Write;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::str::FromStr;

use discv5::enr::CombinedKey;
use ethereum_types::{H256, U256};
use ethportal_api::types::content_key::OverlayContentKey;
use jsonrpc::Request;
use jsonrpsee::{
    core::{client::ClientT, params::ArrayParams},
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use serde::{Deserialize, Serialize};
use serde_json::{
    json,
    value::{to_raw_value, RawValue},
    Value,
};
use thiserror::Error;
use tracing::error;
use trin_utils::bytes::hex_decode;
#[cfg(windows)]
use uds_windows::UnixStream;
use url::Url;

type Enr = discv5::enr::Enr<CombinedKey>;

/// Configuration details for connection to a Portal network node.
#[derive(Clone, Debug)]
pub enum TransportConfig {
    HTTP(Url),
    IPC(PathBuf),
}

/// Details for a Connection to a Portal network node over different transports.
pub enum PortalClient {
    HTTP(HttpClientManager),
    IPC(IpcClientManager),
}

/// HTTP-based transport for connecting to a Portal network node.
pub struct HttpClientManager {
    client: HttpClient,
}

/// IPC-based transport for connecting to a Portal network node.
pub struct IpcClientManager {
    stream: UnixStream,
    request_id: u64,
}

#[derive(Error, Debug)]
pub enum JsonRpcError {
    #[error("received formatted response with no error, but contains a None result")]
    ContainsNone,

    #[error("received empty response (EOF only)")]
    Empty,

    #[error("HTTP client error")]
    HttpClient(#[from] jsonrpsee_core::Error),

    /// Portal network defines "0x" as the response for absent content.
    #[error("expected special 0x 'content absent' message for content request, received HTTP response with None result")]
    SpecialMessageExpected,

    /// Portal network defines "0x" as the response for absent content.
    #[error("received special 0x 'content absent' message for non-content request, expected HTTP response with None result")]
    SpecialMessageUnexpected,

    #[error("unable to convert `{enr_string}` into ENR due to {error}")]
    InvalidEnr {
        error: String, // This source doesn't implement Error
        enr_string: String,
    },

    #[error("unable to convert {input} to hash")]
    InvalidHash {
        source: rustc_hex::FromHexError,
        input: String,
    },

    #[error("invalid integer conversion")]
    InvalidIntegerConversion(#[from] std::num::TryFromIntError),

    #[error("unable to convert string `{input}`")]
    InvalidJson {
        source: serde_json::Error,
        input: String,
    },

    #[error("non-specific I/O error")]
    IO(#[from] std::io::Error),

    #[error("received malformed response: {0}")]
    Malformed(serde_json::Error),

    // todo remove once trin utils stops using nyhow.
    #[error("Opaque error encountered")]
    AnyhowError(#[from] anyhow::Error),

    #[error("unable to serialize/deserialize")]
    Serialization(#[from] serde_json::Error),

    #[error("could not open file {path:?}")]
    OpenFileFailed {
        source: std::io::Error,
        path: PathBuf,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRPCResult {
    id: u32,
    jsonrpc: String,
    result: Value,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub enr: String,
    pub nodeId: String,
    pub ip: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct RoutingTableInfoRaw {
    localKey: String,
    buckets: Vec<(String, String, String)>,
}

pub struct RoutingTableEntry {
    pub node_id: H256,
    pub enr: Enr,
    pub status: String,
    pub distance: U256,
    pub log_distance: u16,
}

#[allow(non_snake_case)]
pub struct RoutingTableInfo {
    pub localKey: H256,
    pub buckets: Vec<RoutingTableEntry>,
}

pub struct Content {
    pub raw: Vec<u8>,
}

/// Differentiates content absent responses from other responses.
/// Portal network specs define content absent by an "0x" response, which otherwise
/// is not readily convertible to the Response type.
#[derive(Clone, Debug)]
pub enum PortalResponse {
    ContentAbsent,
    Regular(Value),
}

impl PortalResponse {
    /// Creates a Portal response from an IPC/HTTP response result.
    fn from_value(val: Value) -> Self {
        match val.eq(&json!("0x")) {
            true => PortalResponse::ContentAbsent,
            false => PortalResponse::Regular(val),
        }
    }

    /// Converts a content response JSON value to a string.
    ///
    /// A valid content response may be None, unlike non-content responses.
    /// This occurs through the special "0x" response defined in the Portal specs.
    fn content_response_to_string(&self) -> Result<Option<String>, JsonRpcError> {
        match self {
            PortalResponse::ContentAbsent => Ok(None),
            PortalResponse::Regular(response) => match response.as_str() {
                None | Some("") => Err(JsonRpcError::ContainsNone),
                Some("0x") => Err(JsonRpcError::SpecialMessageExpected),
                Some(s) => Ok(Some(s.to_owned())),
            },
        }
    }
    /// Converts a non-content (e.g., node info) response JSON value to a string.
    ///
    /// A valid non-content response may be None, unlike content responses,
    /// which must use the special "0x" response defined in the Portal specs.
    fn non_content_response_to_string(&self) -> Result<String, JsonRpcError> {
        match self {
            PortalResponse::ContentAbsent => Err(JsonRpcError::SpecialMessageUnexpected),
            PortalResponse::Regular(r) => Ok(r.to_string()),
        }
    }
}

impl PortalClient {
    /// Returns a client to connect to a Portal network node.
    pub fn from_config(config: &TransportConfig) -> Result<Self, JsonRpcError> {
        Ok(match config {
            TransportConfig::HTTP(http_url) => PortalClient::HTTP(HttpClientManager {
                client: HttpClientBuilder::default().build(http_url.as_ref())?,
            }),
            TransportConfig::IPC(path) => PortalClient::IPC(IpcClientManager {
                stream: UnixStream::connect(path).map_err(|e| JsonRpcError::OpenFileFailed {
                    source: e,
                    path: path.to_owned(),
                })?,
                request_id: 0,
            }),
        })
    }

    pub async fn make_request(
        self,
        method: &str,
        params: Option<Vec<Box<RawValue>>>,
    ) -> Result<PortalResponse, JsonRpcError> {
        match self {
            PortalClient::HTTP(http) => {
                // jsonrpsee requires the conversion of `Option<Vec<Box<RawValue>>>` to `ArrayParams`
                let array_params: ArrayParams = match params {
                    Some(json_params) => {
                        let mut param_aggregator = rpc_params!();
                        for json_param in json_params {
                            param_aggregator.insert(json_param).unwrap()
                        }
                        param_aggregator
                    }
                    None => rpc_params!(),
                };
                let val: Value = http.client.request(method, array_params).await?;
                Ok(PortalResponse::from_value(val))
            }
            PortalClient::IPC(mut ipc) => {
                let request = match &params {
                    Some(raw_params) => Request {
                        method,
                        params: raw_params,
                        id: serde_json::json!(ipc.request_id),
                        jsonrpc: Some("2.0"),
                    },
                    None => Request {
                        method,
                        params: &[],
                        id: serde_json::json!(ipc.request_id),
                        jsonrpc: Some("2.0"),
                    },
                };
                // Manually increment the request id after using it in the request.
                ipc.request_id += 1;

                let data = serde_json::to_vec(&request)?;
                ipc.stream.write_all(&data)?;
                ipc.stream.flush()?;

                let response: JsonRPCResult =
                    serde_json::Deserializer::from_reader(ipc.stream.try_clone()?)
                        .into_iter()
                        .next()
                        // Empty response should only happen when they immediately send EOF
                        .ok_or(JsonRpcError::Empty)??;
                Ok(PortalResponse::from_value(response.result))
            }
        }
    }

    pub async fn get_client_version(self) -> Result<String, JsonRpcError> {
        let method = "web3_clientVersion";
        let params = None;
        self.make_request(method, params)
            .await?
            .non_content_response_to_string()
    }

    pub async fn get_node_info(self) -> Result<NodeInfo, JsonRpcError> {
        let method = "discv5_nodeInfo";
        let params = None;
        let response = self
            .make_request(method, params)
            .await?
            .non_content_response_to_string()?;
        serde_json::from_str(&response).map_err(|e| JsonRpcError::InvalidJson {
            source: e,
            input: response.to_string(),
        })
    }

    pub async fn get_routing_table_info(self) -> Result<RoutingTableInfo, JsonRpcError> {
        let method = "discv5_routingTableInfo";
        let params = None;
        let response = self
            .make_request(method, params)
            .await?
            .non_content_response_to_string()?;
        let result_raw: RoutingTableInfoRaw =
            serde_json::from_str(&response).map_err(|e| JsonRpcError::InvalidJson {
                source: e,
                input: response.to_string(),
            })?;
        let local_node_id =
            H256::from_str(&result_raw.localKey).map_err(|e| JsonRpcError::InvalidHash {
                source: e,
                input: result_raw.localKey.to_string(),
            })?;
        let buckets: Result<Vec<RoutingTableEntry>, JsonRpcError> = result_raw
            .buckets
            .iter()
            .map(|entry| parse_routing_table_entry(&local_node_id, &entry.0, &entry.1, &entry.2))
            .collect();
        Ok(RoutingTableInfo {
            localKey: local_node_id,
            buckets: buckets?,
        })
    }

    pub async fn get_content<T: OverlayContentKey>(
        self,
        content_key: &T,
    ) -> Result<Option<Content>, JsonRpcError> {
        let method = "portal_historyRecursiveFindContent";
        let key = &content_key.to_hex();
        let param = to_raw_value(&key).map_err(|e| JsonRpcError::InvalidJson {
            source: e,
            input: key.to_string(),
        })?;
        match self
            .make_request(method, Some(vec![param]))
            .await?
            .content_response_to_string()?
        {
            Some(response) => {
                let content_raw = hex_decode(&response)?;
                Ok(Some(Content { raw: content_raw }))
            }
            None => Ok(None),
        }
    }
}

fn parse_routing_table_entry(
    local_node_id: &H256,
    raw_node_id: &str,
    encoded_enr: &str,
    status: &String,
) -> Result<RoutingTableEntry, JsonRpcError> {
    let node_id = H256::from_str(raw_node_id).map_err(|e| JsonRpcError::InvalidHash {
        source: e,
        input: raw_node_id.to_string(),
    })?;
    let enr = Enr::from_str(encoded_enr).map_err(|e| JsonRpcError::InvalidEnr {
        error: e,
        enr_string: encoded_enr.to_string(),
    })?;

    let distance = distance_xor(node_id.as_fixed_bytes(), local_node_id.as_fixed_bytes());
    let log_distance = distance_log2(distance)?;
    Ok(RoutingTableEntry {
        node_id,
        enr,
        status: status.to_string(),
        distance,
        log_distance,
    })
}

fn distance_xor(x: &[u8; 32], y: &[u8; 32]) -> U256 {
    let mut z: [u8; 32] = [0; 32];
    for i in 0..32 {
        z[i] = x[i] ^ y[i];
    }
    U256::from_big_endian(z.as_slice())
}

fn distance_log2(distance: U256) -> Result<u16, JsonRpcError> {
    if distance.is_zero() {
        Ok(0)
    } else {
        Ok((256 - distance.leading_zeros()).try_into()?)
    }
}
