use std::{path::PathBuf, time::Duration};

use anyhow::{anyhow, Ok};
use entity::content;
use ethportal_api::types::enr::Enr;
use ethportal_api::types::{
    beacon::{ContentInfo as BeaconContentInfo, TraceContentInfo as BeaconTraceContentInfo},
    history::{ContentInfo as HistoryContentInfo, TraceContentInfo as HistoryTraceContentInfo},
    state::{ContentInfo as StateContentInfo, TraceContentInfo as StateTraceContentInfo},
};
use ethportal_api::{
    BeaconNetworkApiClient, ContentValue, Discv5ApiClient, HistoryNetworkApiClient, NodeInfo,
    RoutingTableInfo, StateNetworkApiClient, Web3ApiClient,
};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use serde_json::json;
use url::Url;

/// Configuration details for connection to a Portal network node.
#[derive(Clone, Debug)]
pub enum TransportConfig {
    HTTP(Url),
    IPC(PathBuf),
}

#[derive(Clone, Debug)]
pub struct PortalApi {
    pub client: HttpClient,
}

#[derive(Clone, Debug)]
pub struct PortalClient {
    pub api: PortalApi,
    pub client_info: String,
    pub enr: Enr,
}

pub struct Content {
    pub raw: Vec<u8>,
}

impl PortalClient {
    pub async fn from(portal_client_url: String) -> Result<Self, anyhow::Error> {
        let api = PortalApi::new(portal_client_url).await?;

        let client_info = api.get_client_version().await?;

        let node_info = api.get_node_info().await?;

        Ok(PortalClient {
            api,
            client_info,
            enr: node_info.enr,
        })
    }

    pub fn supports_trace(&self) -> bool {
        self.client_info.contains("trin") || self.client_info.contains("fluffy")
    }
}

impl PortalApi {
    pub async fn new(client_url: String) -> Result<Self, anyhow::Error> {
        let http_prefix = "http://";
        let client = if client_url.strip_prefix(http_prefix).is_some() {
            Ok(HttpClientBuilder::default()
                .request_timeout(Duration::from_secs(120))
                .build(client_url)?)
        } else {
            panic!("None supported RPC interface {client_url}, use http.");
        };

        Ok(PortalApi { client: client? })
    }

    pub async fn get_client_version(&self) -> Result<String, anyhow::Error> {
        Ok(Web3ApiClient::client_version(&self.client).await?)
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo, anyhow::Error> {
        Ok(Discv5ApiClient::node_info(&self.client).await?)
    }

    pub async fn get_routing_table_info(self) -> Result<RoutingTableInfo, anyhow::Error> {
        Ok(Discv5ApiClient::routing_table_info(&self.client).await?)
    }

    pub async fn get_content(
        self,
        content: &content::Model,
    ) -> Result<Option<Content>, anyhow::Error> {
        match content.protocol_id {
            content::SubProtocol::History => {
                let result = HistoryNetworkApiClient::recursive_find_content(
                    &self.client,
                    content.content_key.clone().try_into()?,
                )
                .await?;
                let HistoryContentInfo::Content { content, .. } = result else {
                    return Err(anyhow!("No content found History"));
                };
                Ok(Some(Content {
                    raw: content.encode(),
                }))
            }
            content::SubProtocol::State => {
                let result = StateNetworkApiClient::recursive_find_content(
                    &self.client,
                    content.content_key.clone().try_into()?,
                )
                .await?;
                let StateContentInfo::Content { content, .. } = result else {
                    return Err(anyhow!("No content found State"));
                };
                Ok(Some(Content {
                    raw: content.encode(),
                }))
            }
            content::SubProtocol::Beacon => {
                let result = BeaconNetworkApiClient::recursive_find_content(
                    &self.client,
                    content.content_key.clone().try_into()?,
                )
                .await?;
                let BeaconContentInfo::Content { content, .. } = result else {
                    return Err(anyhow!("No content found Beacon"));
                };
                Ok(Some(Content {
                    raw: content.encode(),
                }))
            }
        }
    }

    pub async fn get_content_with_trace(
        self,
        content: &content::Model,
    ) -> Result<(Option<Content>, String), anyhow::Error> {
        match content.protocol_id {
            content::SubProtocol::History => {
                let HistoryTraceContentInfo { content, trace, .. } =
                    HistoryNetworkApiClient::trace_recursive_find_content(
                        &self.client,
                        content.content_key.clone().try_into()?,
                    )
                    .await?;
                Ok((
                    Some(Content {
                        raw: content.encode(),
                    }),
                    json!(trace).to_string(),
                ))
            }
            content::SubProtocol::State => {
                let StateTraceContentInfo { content, trace, .. } =
                    StateNetworkApiClient::trace_recursive_find_content(
                        &self.client,
                        content.content_key.clone().try_into()?,
                    )
                    .await?;
                Ok((
                    Some(Content {
                        raw: content.encode(),
                    }),
                    json!(trace).to_string(),
                ))
            }
            content::SubProtocol::Beacon => {
                let BeaconTraceContentInfo { content, trace, .. } =
                    BeaconNetworkApiClient::trace_recursive_find_content(
                        &self.client,
                        content.content_key.clone().try_into()?,
                    )
                    .await?;
                Ok((
                    Some(Content {
                        raw: content.encode(),
                    }),
                    json!(trace).to_string(),
                ))
            }
        }
    }
}
