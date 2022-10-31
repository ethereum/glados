#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

use std::sync::Arc;
use std::path::{
    Path,
    PathBuf
};
use std::str::FromStr;

use serde_json::value::RawValue;
use serde::{Deserialize, Serialize};

use thiserror::Error;

use askama::Template;

use axum::{
    extract::Extension,
    http::StatusCode,
    Router,
    response::{Html, IntoResponse, Response},
    routing::get
};
use clap::Parser;

use discv5::enr::CombinedKey;
type Enr = discv5::enr::Enr<CombinedKey>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   #[arg(short, long)]
   ipc_path: PathBuf,
}

struct State {
    ipc_path: PathBuf
}



#[tokio::main]
async fn main() {
    // parse command line arguments
    let args = Args::parse();

    let shared_state = Arc::new(State {ipc_path: args.ipc_path});

    // setup router
    let app = Router::new()
        .route("/", get(root))
        .layer(Extension(shared_state));
        

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3001".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}


//
// Routes
//
async fn root(
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let ipc_path = state.ipc_path.as_os_str().to_os_string().into_string().unwrap();
    let mut client = PortalClient::from_ipc(&state.ipc_path).unwrap();

    let client_version = client.get_client_version();
    let node_info = client.get_node_info();

    let template = IndexTemplate { ipc_path, client_version, node_info };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    ipc_path: String,
    client_version: String,
    node_info: String,
}

struct HtmlTemplate<T>(T);


impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}

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
    fn try_clone(&self) -> std::io::Result<Self>
    where
        Self: Sized;
}

impl TryClone for UnixStream {
    fn try_clone(&self) -> std::io::Result<Self> {
        UnixStream::try_clone(self)
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
    fn from_ipc(path: &Path) -> std::io::Result<Self> {
        // TODO: a nice error if this file does not exist
        Ok(Self {
            stream: UnixStream::connect(path)?,
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

#[derive(Serialize, Deserialize)]
struct NodeInfo {
    enr: String,
    nodeId: String,
    ip: String,
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

    fn make_request(&mut self, req: jsonrpc::Request) -> Result<JsonRPCResult, JsonRpcError> {
        let data = serde_json::to_vec(&req).unwrap();

        self.stream.write_all(&data).unwrap();
        self.stream.flush().unwrap();

        let clone = self.stream.try_clone().unwrap();
        let deser = serde_json::Deserializer::from_reader(clone);

        if let Some(obj) = deser.into_iter::<serde_json::Value>().next() {
            return obj.map_err(JsonRpcError::Malformed);
        }

        // this should only happen when they immediately send EOF
        Err(JsonRpcError::Empty)
    }

    fn get_client_version(&mut self) -> String {
        let req = self.build_request("web3_clientVersion", &None);
        let resp = self.make_request(req);

        match resp {
            Err(err) => format!("error: {}", err),
            Ok(value) => value.result.to_string(),
        }
    }

    fn get_node_info(&mut self) -> NodeInfo {
        let req = self.build_request("discv5_nodeInfo", &None);
        let resp = self.make_request(req);

        let node_info: NodeInfo = match resp {
            Err(err) => format!("error: {}", err),
            Ok(value) => value.result,
        };

        node_info
    }

    //fn get_node_enr(&mut self) -> Enr {
    //    let node_info = self.get_node_info();
    //    Enr::from_str(node_info.result.enr).unwrap()
    //}
}
