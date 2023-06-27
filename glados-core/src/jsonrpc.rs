use ethportal_api::{Discv5ApiClient, Web3ApiClient};
#[cfg(unix)]
use reth_ipc::client::{IpcClientBuilder, IpcError};
use std::path::PathBuf;
use std::str::FromStr;

use ethereum_types::{H256, U256};
use ethportal_api::types::content_key::OverlayContentKey;
use ethportal_api::HistoryNetworkApiClient;
use jsonrpsee::{
    async_client::{Client, ClientBuilder},
    core::client::ClientT,
    core::params::ArrayParams,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use serde::{Deserialize, Serialize};
use serde_json::{
    json,
    value::{to_raw_value, RawValue},
    Value,
};

use ethportal_api::types::enr::Enr;
use ethportal_api::utils::bytes::ByteUtilsError;
use thiserror::Error;
use tracing::{error, info};
use url::Url;

/// Configuration details for connection to a Portal network node.
#[derive(Clone, Debug)]
pub enum TransportConfig {
    HTTP(Url),
    IPC(PathBuf),
}

#[derive(Debug)]
pub struct PortalApi {
    pub client_url: String,
    pub client: Client,
}

#[derive(Debug)]
pub struct PortalClient {
    pub api: PortalApi,
    pub client_info: String,
    pub enr: Enr,
}

#[derive(Error, Debug)]
pub enum JsonRpcError {
    #[error("received formatted response with no error, but contains a None result")]
    ContainsNone,

    #[error("received empty response (EOF only)")]
    Empty,

    #[error("Client error")]
    HttpClient(#[from] jsonrpsee::core::error::Error),

    #[cfg(unix)]
    #[error("IPC client error")]
    IpcClient(#[from] IpcError),

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

impl PortalClient {
    pub async fn from(portal_client_url: String) -> Result<Self, JsonRpcError> {
        let api = PortalApi {
            client_url: portal_client_url.clone(),
            client: PortalApi::parse_client_url(portal_client_url.clone()).await?,
        };

        let client_info = api
            .client
            .client_version()
            .await
            .expect("Client version request");
        info!("Received client info string: {}", client_info);
        let stripped_client_info = strip_quotes(client_info);

        let node_info = api.client.node_info().await.expect("Node Info request");

        let enr = node_info.enr.clone();

        Ok(PortalClient {
            api,
            client_info: stripped_client_info.to_string(),
            enr,
        })
    }

    pub fn is_trin(&self) -> bool {
        self.client_info.contains("trin")
    }
}

impl PortalApi {
    pub async fn parse_client_url(client_url: String) -> Result<Client, JsonRpcError> {
        let http_prefix = "http://";
        let ipc_prefix = "ipc:///";
        // if client_url.strip_prefix(http_prefix).is_some() {
        //     return Ok(HttpClientBuilder::default()
        //         .build(client_url)
        //         .expect("Creating HTTP client"));
        // } else
        if let Some(ipc_path) = client_url.strip_prefix(ipc_prefix) {
            #[cfg(unix)]
            return Ok(IpcClientBuilder::default()
                .build(ipc_path)
                .await
                .expect("Creating IPC Client"));
            #[cfg(windows)]
            panic!("Reth doesn't support Unix Domain Sockets IPC for windows, use http")
        } else {
            Err(JsonRpcError::ClientURL { url: client_url })
        }
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
