use anyhow::anyhow;
use chrono::Utc;
use entity::content::{self, SubProtocol};

use ethportal_api::{
    types::content_key::beacon::{LightClientFinalityUpdateKey, LightClientOptimisticUpdateKey},
    utils::bytes::{hex_decode, hex_encode},
    BeaconContentKey, LightClientBootstrapKey, LightClientUpdatesByRangeKey, OverlayContentKey,
};
use reqwest::{header, Client as HttpClient};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::sleep;
use tracing::{debug, error, info};

/// Nimbus node to retrieve beacon data from.
pub const PANDA_OPS_BEACON: &str = "";
/// How often the provider will be queried for a new block hash.
const POLL_PERIOD_SECONDS: u64 = 1;
// Beacon chain mainnet genesis time: Tue Dec 01 2020 12:00:23 GMT+0000
pub const BEACON_GENESIS_TIME: u64 = 1606824023;

/// Checks for and stores new Beacon Light Client Bootstrap content keys.
pub async fn follow_beacon_head(
    conn: DatabaseConnection,
    client: HttpClient,
    beacon_base_url: String,
) {
    debug!("Getting initial block root");
    let mut latest_finalized_block_root = get_current_beacon_block_root(&client, &beacon_base_url)
        .await
        .expect("Failed to get initial finalized beacon block");

    info!(
        "Retrieved initial block root: {}",
        latest_finalized_block_root
    );
    store_bootstrap_content_key(&latest_finalized_block_root, conn.clone())
        .await
        .expect("Failed to store initial block root");

    loop {
        debug!("Sleeping for {} seconds", POLL_PERIOD_SECONDS);
        sleep(Duration::from_secs(POLL_PERIOD_SECONDS)).await;

        debug!("Checking for new finalized block root");
        let current_finalized_block_root =
            match get_current_beacon_block_root(&client, &beacon_base_url).await {
                Ok(block_root) => block_root,
                Err(e) => {
                    error!(err=?e, "Failed to get current beacon block root");
                    continue;
                }
            };

        if current_finalized_block_root != latest_finalized_block_root {
            latest_finalized_block_root = current_finalized_block_root;
            info!("New finalized block root: {}", latest_finalized_block_root);

            if let Err(err) =
                store_bootstrap_content_key(&latest_finalized_block_root, conn.clone()).await
            {
                error!("Failed to store bootstrap: {err:?}");
            }
        }
    }
}

/// Stores a LightClientBootstrap content key for the given block hash if it doesn't already exist.
async fn store_bootstrap_content_key(hash: &str, conn: DatabaseConnection) -> anyhow::Result<()> {
    let content_key = BeaconContentKey::LightClientBootstrap(LightClientBootstrapKey {
        block_hash: <[u8; 32]>::try_from(hex_decode(hash)?).map_err(|err| {
            anyhow::anyhow!("Failed to convert finalized block root to bytes: {err:?}")
        })?,
    });

    match content::get_or_create(SubProtocol::Beacon, &content_key, Utc::now(), &conn).await {
        Ok(_) => debug!(
            content.key = hex_encode(content_key.to_bytes()),
            "Imported new beacon Bootstrap content key",
        ),
        Err(err) => return Err(anyhow!("Failed to store bootstrap content key: {}", err)),
    }

    Ok(())
}

/// Stores a LightClientOptimisticUpdate content key.
pub async fn store_lc_optimistic_update(
    conn: DatabaseConnection,
    client: &HttpClient,
    beacon_base_url: &String,
) -> anyhow::Result<()> {
    let content_key = get_lc_optimistic_update_key(client, beacon_base_url).await?;
    let content_key = BeaconContentKey::LightClientOptimisticUpdate(content_key);

    match content::get_or_create(SubProtocol::Beacon, &content_key, Utc::now(), &conn).await {
        Ok(_) => {
            info!(
                content.key = hex_encode(content_key.to_bytes()),
                "Imported new beacon LightClientOptimisticUpdate content key",
            );
            Ok(())
        }
        Err(err) => Err(anyhow!(
            "Failed to store LC optimistic update content key: {}",
            err
        )),
    }
}

/// Stores a LightClientUpdatesByRange content key for the current period if one doesnt already exist.
pub async fn store_lc_update_by_range(conn: DatabaseConnection) -> anyhow::Result<()> {
    let expected_period = expected_current_period();

    let content_key = BeaconContentKey::LightClientUpdatesByRange(LightClientUpdatesByRangeKey {
        start_period: expected_period,
        count: 1,
    });

    match content::get_or_create(SubProtocol::Beacon, &content_key, Utc::now(), &conn).await {
        Ok(_) => {
            debug!(
                content.key = hex_encode(content_key.to_bytes()),
                "Imported new beacon LightClientUpdatesByRange content key",
            );
            Ok(())
        }
        Err(err) => Err(anyhow!(
            "Failed to store LC update by range content key: {}",
            err
        )),
    }
}

/// Retrieve the latest finalized block root from the beacon node.
async fn get_current_beacon_block_root(
    client: &HttpClient,
    beacon_base_url: &String,
) -> anyhow::Result<String> {
    let url = format!("{}/eth/v1/beacon/blocks/finalized/root", beacon_base_url);
    let response = client.get(url).send().await?.text().await?;
    let response: Value = serde_json::from_str(&response)?;
    let latest_finalized_block_root: String =
        serde_json::from_value(response["data"]["root"].clone())?;
    Ok(latest_finalized_block_root)
}

/// Requests the latest `LightClientOptimisticUpdateKey` known by the server.
pub async fn get_lc_optimistic_update_key(
    client: &HttpClient,
    beacon_base_url: &String,
) -> anyhow::Result<LightClientOptimisticUpdateKey> {
    let url = format!(
        "{}/eth/v1/beacon/light_client/optimistic_update",
        beacon_base_url
    );
    let response = client.get(url).send().await?.text().await?;
    let update: Value = serde_json::from_str(&response)?;

    let signature_slot = update["data"]["signature_slot"]
        .as_str()
        .ok_or(anyhow!("signature_slot is not a string"))?;
    let signature_slot: u64 = signature_slot
        .parse()
        .map_err(|_| anyhow!("Failed to parse signature_slot as u64"))?;

    let update: LightClientOptimisticUpdateKey =
        LightClientOptimisticUpdateKey::new(signature_slot);

    Ok(update)
}

/// Gets the latest `LightClientFinalityUpdateKey` known by the server.
pub async fn get_lc_finality_update_key(
    client: &HttpClient,
    beacon_base_url: &String,
) -> anyhow::Result<LightClientFinalityUpdateKey> {
    let url = format!(
        "{}/eth/v1/beacon/light_client/finality_update",
        beacon_base_url
    );
    let response = client.get(url).send().await?.text().await?;
    let update: Value = serde_json::from_str(&response)?;

    let signature_slot = update["data"]["signature_slot"]
        .as_str()
        .ok_or(anyhow!("signature_slot is not a string"))?;
    let signature_slot: u64 = signature_slot
        .parse()
        .map_err(|_| anyhow!("Failed to parse signature_slot as u64"))?;
    let update: LightClientFinalityUpdateKey = LightClientFinalityUpdateKey::new(signature_slot);
    Ok(update)
}

/// Creates a reqwest::Client configured for PandaOps auth.
pub fn panda_ops_http() -> anyhow::Result<HttpClient> {
    let mut headers = header::HeaderMap::new();
    let client_id = env::var("PANDAOPS_CLIENT_ID")
        .map_err(|_| anyhow!("PANDAOPS_CLIENT_ID env var not set."))?;
    let client_id = header::HeaderValue::from_str(&client_id);
    let client_secret = env::var("PANDAOPS_CLIENT_SECRET")
        .map_err(|_| anyhow!("PANDAOPS_CLIENT_SECRET env var not set."))?;
    let client_secret = header::HeaderValue::from_str(&client_secret);
    headers.insert("CF-Access-Client-Id", client_id?);
    headers.insert("CF-Access-Client-Secret", client_secret?);

    match reqwest::Client::builder().default_headers(headers).build() {
        Ok(client) => Ok(client),
        Err(e) => Err(anyhow!("Failed to build http client: {}", e)),
    }
}

/// Calculates the expected current beacon period based on the current time.
fn expected_current_period() -> u64 {
    let now = SystemTime::now();
    let now = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let since_genesis = now - std::time::Duration::from_secs(BEACON_GENESIS_TIME);

    since_genesis.as_secs() / 12 / (32 * 256)
}
