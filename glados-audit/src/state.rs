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
        content_key::state::AccountTrieNodeKey, portal::ContentInfo, state_trie::nibbles::Nibbles,
        state_trie::EncodedTrieNode,
    },
    StateContentKey, StateNetworkApiClient,
};
use glados_core::{db::store_content_key, jsonrpc::PortalClient};
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
    content_key: AccountTrieNodeKey,
    trie_node: Node,
) -> Result<(AccountTrieNodeKey, bool), (anyhow::Error, AccountTrieNodeKey)> {
    match trie_node {
        Node::Leaf(_leaf_node) => {
            Ok((content_key, true))
        }
        Node::Extension(extension_node) => {
            let extension_node =
                &extension_node.read().expect("Read should never fail");
            let prefix = extension_node.prefix.get_data();
            let node = &extension_node.node;
            match node {
                Node::Hash(hash_node) => {
                    Ok((
                        AccountTrieNodeKey {
                            path: Nibbles::try_from_unpacked_nibbles([content_key.path.nibbles(), prefix]
                                .concat()
                                .as_slice()).expect("Bug building path in random_state_walk"),
                            node_hash: hash_node.hash,
                        },
                        false,
                    ))
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
                    Node::Hash(hash_node) => Some(
                        AccountTrieNodeKey {
                            path: Nibbles::try_from_unpacked_nibbles(
                                [content_key.path.nibbles(), &[index as u8]]
                                    .concat()
                                    .as_slice(),
                            )
                            .expect("Bug building path in random_state_walk"),
                            node_hash: hash_node.hash,
                        },
                    ),
                    _ => {
                        None
                    }
                })
                .choose(&mut rand::thread_rng());
            match optional_random_node {
                Some(random_node) => {
                    Ok((random_node, false))
                }
                None => {
                    Err((anyhow!(
                        "Branch node contained 0 hash nodes, this shouldn't be possible: {content_key:?}"
                    ), content_key))
                }
            }
        }
        Node::Hash(hash_node) => {
            Err((anyhow!(
                "Hash shouldn't be returned from the network: {content_key:?} {hash_node:?}"
            ), content_key))
        }
        Node::Empty => {
            Err((anyhow!(
                "State random walk audit recevied empty node, when it shouldn't be possible: {content_key:?}"
            ), content_key))
        }
    }
}

async fn random_state_walk(
    state_root: B256,
    client: HttpClient,
) -> Result<AccountTrieNodeKey, (anyhow::Error, AccountTrieNodeKey)> {
    let root_content_key = AccountTrieNodeKey {
        path: Nibbles::try_from_unpacked_nibbles(&[])
            .expect("Bug building path in random_state_walk"),
        node_hash: state_root,
    };
    let mut current_content_key = root_content_key.clone();
    loop {
        let response = match StateNetworkApiClient::recursive_find_content(
            &client,
            StateContentKey::AccountTrieNode(current_content_key.clone()),
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                return Err((
                    anyhow!("Error recursive_find_content failed with: {err:?}"),
                    current_content_key,
                ));
            }
        };

        let content_value = match response {
            ContentInfo::Content {
                content: content_value,
                ..
            } => content_value,
            other_content_info => {
                return Err((
                    anyhow!(
                        "Error unexpected recursive_find_content response: {other_content_info:?}"
                    ),
                    current_content_key,
                ));
            }
        };

        let encoded_trie_node: EncodedTrieNode = content_value.to_vec().into();

        let trie_node = encoded_trie_node.as_trie_node().map_err(|err| {
            (
                anyhow!("Error decoding node while walking trie: {err:?}"),
                current_content_key.clone(),
            )
        })?;
        match process_trie_node(current_content_key, trie_node).await? {
            (next_content_key, false) => {
                current_content_key = next_content_key;
            }
            (next_content_key, true) => return Ok(next_content_key),
        }

        // Limit check for new tasks to 10/sec
        sleep(Duration::from_millis(100)).await;
    }
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

            let client = portal_client.api.client.clone();

            let (state_audit_result, content_key) =
                match random_state_walk(state_root, client).await {
                    Ok(content_key) => (true, content_key),
                    Err((err, content_key)) => {
                        error!(err=?err, content_key=?content_key, "Error during state audit.");
                        (false, content_key)
                    }
                };

            if let Err(err) = save_state_audit(
                &conn,
                StateContentKey::AccountTrieNode(content_key),
                state_audit_result,
                block_number,
                portal_client.clone(),
                available_at,
            )
            .await
            {
                error!(err=?err, "Error saving state audit.");
            }
        }
    });
}
