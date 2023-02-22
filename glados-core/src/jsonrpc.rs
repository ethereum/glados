#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

use std::path::Path;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use discv5::enr::CombinedKey;
use ethereum_types::{H256, U256};
use ethportal_api::types::content_key::OverlayContentKey;
use serde::{Deserialize, Serialize};
use serde_json::value::{to_raw_value, RawValue};
use thiserror::Error;

type Enr = discv5::enr::Enr<CombinedKey>;

//
// JSON RPC Client
//
fn build_request<'a>(
    method: &'a str,
    raw_params: &'a Option<Vec<Box<RawValue>>>,
    request_id: u64,
) -> jsonrpc::Request<'a> {
    match raw_params {
        Some(val) => jsonrpc::Request {
            method,
            params: val,
            id: serde_json::json!(request_id),
            jsonrpc: Some("2.0"),
        },
        None => jsonrpc::Request {
            method,
            params: &[],
            id: serde_json::json!(request_id),
            jsonrpc: Some("2.0"),
        },
    }
}

pub trait TryClone {
    fn try_clone(&self) -> Result<Self>
    where
        Self: Sized;
}

impl TryClone for UnixStream {
    fn try_clone(&self) -> Result<Self> {
        Ok(UnixStream::try_clone(self)?)
    }
}

pub struct PortalClient<S>
where
    S: std::io::Read + std::io::Write + TryClone,
{
    stream: S,
    request_id: u64,
}

impl PortalClient<UnixStream> {
    pub fn from_ipc(path: &Path) -> Result<Self> {
        Ok(Self {
            stream: UnixStream::connect(path)
                .with_context(|| format!("Could not open ipc file {}", path.display()))?,
            request_id: 0,
        })
    }
}

#[derive(Error, Debug)]
pub enum JsonRpcError {
    #[error("Received malformed response: {0}")]
    Malformed(serde_json::Error),

    #[error("Received empty response")]
    Empty,
}

#[derive(Serialize, Deserialize)]
struct JsonRPCResult {
    id: u32,
    jsonrpc: String,
    result: serde_json::Value,
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

// TryClone is used because JSON-RPC responses are not followed by EOF. We must read bytes
// from the stream until a complete object is detected, and the simplest way of doing that
// with available APIs is to give ownership of a Read to a serde_json::Deserializer. If we
// gave it exclusive ownership that would require us to open a new connection for every
// command we wanted to send! By making a clone (or, by trying to) we can have our cake
// and eat it too.
//
// TryClone is not necessary if PortalClient stays in this file forever; this script only
// needs to make a single request before it exits. However, in a future where PortalClient
// becomes the mechanism other parts of the codebase (such as peertester) use to act as
// JSON-RPC clients then this becomes necessary. So, this is slightly over-engineered but
// with an eye to future growth.
impl<'a, S> PortalClient<S>
where
    S: std::io::Read + std::io::Write + TryClone,
{
    fn build_request(
        &mut self,
        method: &'a str,
        params: &'a Option<Vec<Box<RawValue>>>,
    ) -> jsonrpc::Request<'a> {
        let result = build_request(method, params, self.request_id);
        self.request_id += 1;

        result
    }

    fn make_request(&mut self, req: jsonrpc::Request) -> Result<JsonRPCResult> {
        let data = serde_json::to_vec(&req)?;

        self.stream.write_all(&data)?;
        self.stream.flush()?;

        let clone = self.stream.try_clone()?;
        let deser = serde_json::Deserializer::from_reader(clone);

        let Some(obj) = deser.into_iter::<JsonRPCResult>().next() else {
            // this should only happen when they immediately send EOF
            return Err(JsonRpcError::Empty)?
        };
        Ok(obj?)
    }

    pub fn get_client_version(&mut self) -> String {
        let req = self.build_request("web3_clientVersion", &None);
        let resp = self.make_request(req);

        match resp {
            Err(err) => format!("error: {err}"),
            Ok(value) => value.result.to_string(),
        }
    }

    pub fn get_node_info(&mut self) -> Result<NodeInfo> {
        let req = self.build_request("discv5_nodeInfo", &None);
        let resp = self.make_request(req)?;
        Ok(serde_json::from_value(resp.result)?)
    }

    pub fn get_routing_table_info(&mut self) -> Result<RoutingTableInfo> {
        let req = self.build_request("discv5_routingTableInfo", &None);
        let resp = self.make_request(req)?;

        let result_raw: RoutingTableInfoRaw = serde_json::from_value(resp.result)?;
        let local_node_id = H256::from_str(&result_raw.localKey)?;
        let buckets: Result<Vec<RoutingTableEntry>> = result_raw
            .buckets
            .iter()
            .map(|entry| parse_routing_table_entry(&local_node_id, &entry.0, &entry.1, &entry.2))
            .collect();
        Ok(RoutingTableInfo {
            localKey: H256::from_str(&result_raw.localKey)?,
            buckets: buckets?,
        })
    }

    pub fn get_content<T: OverlayContentKey>(&mut self, content_key: &T) -> Result<Content> {
        let encoded = hex::encode(content_key.to_bytes());
        let content_key_string = format!("0x{encoded}");
        let params = Some(vec![to_raw_value(&content_key_string).unwrap()]);
        let req = self.build_request("portal_historyRecursiveFindContent", &params);
        let resp = self.make_request(req)?;

        let content_as_hex: String = serde_json::from_value(resp.result)?;
        let content_raw = hex::decode(&content_as_hex[2..])?;

        Ok(Content { raw: content_raw })
    }

    //fn get_node_enr(&mut self) -> Enr {
    //    let node_info = self.get_node_info();
    //    Enr::from_str(node_info.result.enr)?
    //}
}

fn parse_routing_table_entry(
    local_node_id: &H256,
    raw_node_id: &str,
    encoded_enr: &str,
    status: &String,
) -> Result<RoutingTableEntry> {
    let node_id = H256::from_str(raw_node_id)?;
    let Ok(enr) = Enr::from_str(encoded_enr) else {
        bail!("Could not make ENR from string: {}", encoded_enr)
    };
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

fn distance_log2(distance: U256) -> Result<u16> {
    if distance.is_zero() {
        Ok(0)
    } else {
        Ok((256 - distance.leading_zeros()).try_into()?)
    }
}
