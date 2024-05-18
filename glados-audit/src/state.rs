use std::time::Duration;

use alloy_primitives::B256;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use entity::{
    client_info,
    content::SubProtocol,
    content_audit::{self, SelectionStrategy, StateSelectionStrategy},
    node, state_roots,
};
use eth_trie::node::Node;
use ethportal_api::{
    jsonrpsee::http_client::HttpClient,
    types::{
        content_key::state::AccountTrieNodeKey, state::ContentInfo, state_trie::nibbles::Nibbles,
    },
    StateContentKey, StateContentValue, StateNetworkApiClient,
};
use glados_core::{
    db::store_content_key,
    jsonrpc::{PortalApi, PortalClient, Transport},
};
use rand::seq::IteratorRandom;
use sea_orm::DatabaseConnection;
use tokio::time::sleep;
use tracing::error;

use crate::AuditConfig;

async fn save_state_audit(
    conn: &DatabaseConnection,
    content_key: StateContentKey,
    audit_result: bool,
    block_number: i32,
    client: PortalClient,
    available_at: DateTime<Utc>,
) -> Result<(), anyhow::Error> {
    let content_model = store_content_key(
        &content_key,
        "state_intermediate_trie_node",
        block_number,
        available_at,
        conn,
        SubProtocol::State,
    )
    .await
    .ok_or(anyhow!("Failed to store content key."))?;

    let client_info_id = client_info::get_or_create(client.client_info, conn)
        .await
        .map_err(|err| anyhow!("Could not create/lookup client info in db {err}"))?
        .id;

    let node_id = node::get_or_create(client.enr.node_id(), conn)
        .await
        .map_err(|err| anyhow!("Failed to created node. {err}"))?
        .id;

    let _ = content_audit::create(
        content_model.id,
        client_info_id,
        node_id,
        audit_result,
        SelectionStrategy::State(StateSelectionStrategy::StateRoots),
        "".to_string(),
        conn,
    )
    .await
    .map_err(|err| anyhow!("Could not create audit entry in db. {err}"))?;

    Ok(())
}

async fn process_trie_node(
    conn: &DatabaseConnection,
    portal_client: PortalClient,
    block_number: i32,
    path: &[u8],
    content_key: StateContentKey,
    trie_node: Node,
    available_at: DateTime<Utc>,
) -> Result<Option<(StateContentKey, usize)>, (anyhow::Error, StateContentKey)> {
    match trie_node {
        Node::Leaf(_leaf_node) => {
            match save_state_audit(conn, content_key.clone(), true, block_number, portal_client.clone(), available_at).await.map_err(|err| (err, content_key)) {
                Ok(_) => Ok(None),
                Err(err) => Err(err),
            }
        }
        Node::Extension(extension_node) => {
            let extension_node =
                &extension_node.read().expect("Read should never fail");
            let prefix = extension_node.prefix.get_data();
            let node = &extension_node.node;
            match node {
                Node::Hash(hash_node) => {
                    Ok(Some((
                        StateContentKey::AccountTrieNode(AccountTrieNodeKey {
                            path: Nibbles::try_from_unpacked_nibbles([path, prefix]
                                .concat()
                                .as_slice()).expect("Bug building path in random_state_walk"),
                            node_hash: hash_node.hash,
                        }),
                        prefix.first().copied().unwrap_or(0) as usize,
                    )))
                }
                other_node => {
                    Err((anyhow!(
                        "Extension node has a non-hash node: {:?}",
                        other_node
                    ), content_key))
                }
            }
        },
        Node::Branch(branch_node) => {
            let children =
                &branch_node.read().expect("Read should never fail").children;
            let optional_random_node = children
                .iter()
                .enumerate()
                .filter_map(|(index, child)| match child {
                    Node::Hash(hash_node) => Some((
                        index,
                        StateContentKey::AccountTrieNode(AccountTrieNodeKey {
                            path: Nibbles::try_from_unpacked_nibbles(
                                [path, &[index as u8]]
                                    .concat()
                                    .as_slice(),
                            )
                            .expect("Bug building path in random_state_walk"),
                            node_hash: hash_node.hash,
                        }),
                    )),
                    _ => {
                        None
                    }
                })
                .choose(&mut rand::thread_rng());
            match optional_random_node {
                Some((index, random_node)) => {
                    Ok(Some((random_node, index)))
                }
                None => {
                    Err((anyhow!(
                        "Branch node contained 0 hash nodes, this shouldn't be possible: {content_key:?}"
                    ), content_key))
                }
            }
        }
        Node::Hash(hash_node) => {
            Ok(Some((
                StateContentKey::AccountTrieNode(AccountTrieNodeKey {
                    path: Nibbles::try_from_unpacked_nibbles([path, &[0u8]]
                        .concat()
                        .as_slice()).expect("Bug building path in random_state_walk"),
                    node_hash: hash_node.hash,
                }),
                0,
            )))
        }
        Node::Empty => {
            Err((anyhow!(
                "State random walk audit recevied empty node, when it shouldn't be possible: {content_key:?}"
            ), content_key))
        }
    }
}

async fn random_state_walk(
    conn: &DatabaseConnection,
    state_root: B256,
    portal_client: PortalClient,
    client: HttpClient,
    block_number: i32,
    available_at: DateTime<Utc>,
) -> Result<(), (anyhow::Error, StateContentKey)> {
    let mut stack = vec![StateContentKey::AccountTrieNode(AccountTrieNodeKey {
        path: Nibbles::try_from_unpacked_nibbles(&[])
            .expect("Bug building path in random_state_walk"),
        node_hash: state_root,
    })];
    let mut path: Vec<u8> = vec![];
    while let Some(content_key) = stack.pop() {
        match StateNetworkApiClient::recursive_find_content(&client, content_key.clone()).await {
            Ok(response) => match response {
                ContentInfo::Content {
                    content: content_value,
                    ..
                } => match content_value {
                    StateContentValue::TrieNode(encoded_trie_node) => {
                        let trie_node = encoded_trie_node.node.as_trie_node().expect("Trie node received from the portal network should be decoded as a trie node");
                        match process_trie_node(
                            conn,
                            portal_client.clone(),
                            block_number,
                            &path,
                            content_key,
                            trie_node,
                            available_at,
                        )
                        .await?
                        {
                            Some((next_content_key, next_path_index)) => {
                                path.push(next_path_index as u8);
                                stack.push(next_content_key);
                            }
                            None => return Ok(()),
                        }
                    }
                    other_state_content_value => {
                        return Err((anyhow!(
                                "State random walk audit recevied unexpected content type: {other_state_content_value:?}"
                            ), content_key));
                    }
                },
                other_content_info => {
                    return Err((
                        anyhow!(
                        "Error unexpected recursive_find_content response: {other_content_info:?}"
                    ),
                        content_key,
                    ));
                }
            },
            Err(err) => {
                return Err((
                    anyhow!("Error recursive_find_content failed with: {err:?}"),
                    content_key,
                ));
            }
        }
        // Limit check for new tasks to 10/sec
        sleep(Duration::from_millis(100)).await;
    }
    Err((
        anyhow!("Walk exhausted without finding a leaf node or failing, maybe there is a bug? this should not happen."),
        StateContentKey::AccountTrieNode(AccountTrieNodeKey {
            path: Nibbles::try_from_unpacked_nibbles(&[])
                .expect("Bug building path in random_state_walk"),
            node_hash: state_root,
        }),
    ))
}

pub async fn spawn_state_audit(conn: DatabaseConnection, config: AuditConfig) {
    tokio::spawn(async move {
        let mut cycle_of_clients = config.portal_clients.clone().into_iter().cycle();

        loop {
            let state_roots_model = match state_roots::get_random_state_root(&conn).await {
                Ok(state_roots_model) => match state_roots_model {
                    Some(state_roots_model) => state_roots_model,
                    None => {
                        error!("No state roots found in the database.");
                        continue;
                    }
                },
                Err(err) => {
                    error!(err=?err, "Error getting random state root.");
                    continue;
                }
            };
            let block_number = state_roots_model.block_number();
            let state_root = state_roots_model.state_root();
            let available_at = state_roots_model.first_available_at;

            let portal_client = match cycle_of_clients.next() {
                Some(client) => client,
                None => {
                    error!("Empty list of clients for audit.");
                    return;
                }
            };

            let transport = PortalApi::parse_client_url(portal_client.api.client_url.clone())
                .await
                .expect("Failed to parse client url.");
            let client = match transport {
                Transport::HTTP(http) => http.client,
            };

            if let Err((err, content_key)) = random_state_walk(
                &conn,
                state_root,
                portal_client.clone(),
                client,
                block_number,
                available_at,
            )
            .await
            {
                if let Err(err) = save_state_audit(
                    &conn,
                    content_key.clone(),
                    false,
                    block_number,
                    portal_client.clone(),
                    available_at,
                )
                .await
                {
                    error!(err=?err, "Error saving state audit.");
                }
                error!(err=?err, content_key=?content_key, "Error during state audit.");
            }
        }
    });
}
