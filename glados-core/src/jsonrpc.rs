use std::str::FromStr;
use std::{path::PathBuf, time::Duration};

use entity::content;
use ethereum_types::{H256, U256};
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

use ethportal_api::utils::bytes::{hex_decode, hex_encode, ByteUtilsError};
use thiserror::Error;
use tracing::{error, info};
use url::Url;

use ethportal_api::types::enr::Enr;

/// Configuration details for connection to a Portal network node.
#[derive(Clone, Debug)]
pub enum TransportConfig {
    HTTP(Url),
    IPC(PathBuf),
}

/// Details for a Connection to a Portal network node over different transports.
pub enum Transport {
    HTTP(HttpClientManager),
}

#[derive(Clone, Debug)]
pub struct PortalApi {
    pub client_url: String,
}

#[derive(Clone, Debug)]
pub struct PortalClient {
    pub api: PortalApi,
    pub client_info: String,
    pub enr: Enr,
}

/// HTTP-based transport for connecting to a Portal network node.
pub struct HttpClientManager {
    client: HttpClient,
}

const CONTENT_NOT_FOUND_ERROR_CODE: i32 = -39001;
#[derive(Error, Debug)]
pub enum JsonRpcError {
    #[error("received formatted response with no error, but contains a None result")]
    ContainsNone,

    #[error("received empty response (EOF only)")]
    Empty,

    #[error("HTTP client error: {0}")]
    HttpClient(String),

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

    #[error("malformed portal client URL")]
    ClientURL { url: String },

    #[error("unable to use byte utils {0}")]
    ByteUtils(#[from] ByteUtilsError),

    #[error("unable to serialize/deserialize")]
    Serialization(#[from] serde_json::Error),

    #[error("could not open file {path:?}")]
    OpenFileFailed {
        source: std::io::Error,
        path: PathBuf,
    },

    #[error("Query completed without finding content")]
    ContentNotFound { trace: Option<String> },
}

impl From<jsonrpsee::core::error::Error> for JsonRpcError {
    fn from(e: jsonrpsee::core::error::Error) -> Self {
        if let jsonrpsee::core::error::Error::Call(ref error) = e {
            if error.code() == CONTENT_NOT_FOUND_ERROR_CODE {
                return JsonRpcError::ContentNotFound {
                    trace: error.data().map(|data| data.to_string()),
                };
            }
        }

        // Fallback to the generic HttpClient error variant if no match
        JsonRpcError::HttpClient(e.to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct PortalRpcError {
    pub code: Value,
    pub message: Value,
    pub data: Option<Value>,
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

#[derive(Deserialize)]
pub struct TracedQueryResult {
    pub content: String,
    pub trace: Value,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct QueryResult {
    pub content: String,
    pub utpTransfer: bool,
}

pub struct Content {
    pub raw: Vec<u8>,
}

impl PortalClient {
    pub async fn from(portal_client_url: String) -> Result<Self, JsonRpcError> {
        let api = PortalApi {
            client_url: portal_client_url.clone(),
        };

        let client_info = api.get_client_version().await?;
        let stripped_client_info = strip_quotes(client_info);

        let node_info = &api.get_node_info().await?;

        let enr_string = node_info.enr.clone();
        let enr: Enr = enr_string.parse().map_err(|err| JsonRpcError::InvalidEnr {
            error: err,
            enr_string,
        })?;

        Ok(PortalClient {
            api,
            client_info: stripped_client_info.to_string(),
            enr,
        })
    }

    pub fn supports_trace(self) -> bool {
        self.client_info.contains("trin") || self.client_info.contains("fluffy")
    }
}

impl PortalApi {
    pub async fn make_request(
        &self,
        method: &str,
        params: Option<Vec<Box<RawValue>>>,
    ) -> Result<String, JsonRpcError> {
        let transport = PortalApi::parse_client_url(self.client_url.clone()).await?;
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
        match transport {
            Transport::HTTP(http) => {
                let val: Value = http.client.request(method, array_params).await?;
                Ok(val.to_string())
            }
        }
    }

    pub async fn get_client_version(&self) -> Result<String, JsonRpcError> {
        let method = "web3_clientVersion";
        let params = None;
        self.make_request(method, params).await
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo, JsonRpcError> {
        let method = "discv5_nodeInfo";
        let params = None;
        let response = self.make_request(method, params).await?;
        serde_json::from_str(&response).map_err(|e| JsonRpcError::InvalidJson {
            source: e,
            input: response.to_string(),
        })
    }

    pub async fn get_routing_table_info(self) -> Result<RoutingTableInfo, JsonRpcError> {
        let method = "discv5_routingTableInfo";
        let params = None;
        let response = self.make_request(method, params).await?;
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

    pub async fn get_content(
        self,
        content: &content::Model,
    ) -> Result<Option<Content>, JsonRpcError> {
        let method = match content.protocol_id {
            content::SubProtocol::History => "portal_historyRecursiveFindContent",
            content::SubProtocol::State => "portal_stateRecursiveFindContent",
            content::SubProtocol::Beacon => "portal_beaconRecursiveFindContent",
        };
        let key = hex_encode(content.content_key.clone());
        let param = to_raw_value(&key).map_err(|e| JsonRpcError::InvalidJson {
            source: e,
            input: key.to_string(),
        })?;
        match self.make_request(method, Some(vec![param])).await {
            Ok(response) => {
                let query_result = serde_json::from_value::<QueryResult>(json!(response))
                    .map_err(JsonRpcError::Malformed)?;

                let content_raw = hex_decode(&query_result.content)?;
                Ok(Some(Content { raw: content_raw }))
            }
            Err(err) => match err {
                JsonRpcError::ContentNotFound { trace: _ } => Ok(None),
                _ => Err(err),
            },
        }
    }

    pub async fn get_content_with_trace(
        self,
        content: &content::Model,
    ) -> Result<(Option<Content>, String), JsonRpcError> {
        let params = Some(vec![to_raw_value(&hex_encode(
            content.content_key.clone(),
        ))?]);

        let method = match content.protocol_id {
            content::SubProtocol::History => "portal_historyTraceRecursiveFindContent",
            content::SubProtocol::State => "portal_stateTraceRecursiveFindContent",
            content::SubProtocol::Beacon => "portal_beaconTraceRecursiveFindContent",
        };
        info!("Making request to method: {}", method);
        info!("Protocol: {:?}", content.protocol_id);
        match self.make_request(method, params).await {
            Ok(result) => {
                let query_result: TracedQueryResult = serde_json::from_str(&result)?;
                let trace = query_result.trace.to_string();
                Ok((
                    Some(Content {
                        raw: hex_decode(&query_result.content)?,
                    }),
                    trace,
                ))
            }
            Err(err) => match err {
                JsonRpcError::ContentNotFound { trace } => Ok((None, trace.unwrap_or_default())),
                _ => Err(err),
            },
        }
    }

    pub async fn parse_client_url(client_url: String) -> Result<Transport, JsonRpcError> {
        let http_prefix = "http://";
        let ipc_prefix = "ipc:///";
        if client_url.strip_prefix(http_prefix).is_some() {
            Ok(Transport::HTTP(HttpClientManager {
                client: HttpClientBuilder::default()
                    .request_timeout(Duration::from_secs(120))
                    .build(client_url)?,
            }))
        } else if client_url.strip_prefix(ipc_prefix).is_some() {
            panic!("IPC not implemented, use http.");
        } else {
            Err(JsonRpcError::ClientURL { url: client_url })
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

fn strip_quotes(client_info: String) -> String {
    if client_info.starts_with('"') && client_info.ends_with('"') {
        let mut chars = client_info.chars();
        chars.next();
        chars.next_back();
        chars.as_str().to_owned()
    } else {
        client_info
    }
}

#[cfg(test)]
mod tests {

    use super::strip_quotes;
    use rstest::rstest;

    #[rstest]
    #[case("\"test\"", "test")]
    #[case("test", "test")]
    fn test_strip_quotes(#[case] original: String, #[case] expected: String) {
        assert_eq!(strip_quotes(original), expected);
    }
}
