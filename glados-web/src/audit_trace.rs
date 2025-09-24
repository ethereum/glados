use std::collections::{HashMap, HashSet};

use alloy_primitives::{hex, B256};
use chrono::{TimeZone, Utc};
use enr::NodeId;
use entity::client_info::Client;
use ethportal_api::types::query_trace::QueryTrace;
use itertools::Itertools;
use sea_orm::{prelude::DateTimeUtc, DatabaseConnection, DbBackend, FromQueryResult, Statement};
use tracing::{error, warn};

/// Modifies query trace to include node details at the time of the query (e.g. node's radius).
///
/// It also returns other node's informations.
pub async fn audit_trace_set_node_details(
    trace: &mut QueryTrace,
    conn: &DatabaseConnection,
) -> HashMap<NodeId, Client> {
    let mut node_details = HashMap::new();

    // Get the timestamp of the query
    let timestamp: DateTimeUtc = Utc
        .timestamp_millis_opt(trace.started_at_ms as i64)
        .single()
        .expect("Failed to convert timestamp to DateTime");

    // Do a query to get, for each node, the radius recorded closest to the time at which the trace took place.
    let node_ids: Vec<Vec<u8>> = trace
        .metadata
        .keys()
        .cloned()
        .map(|x| x.raw().to_vec())
        .collect();
    let node_ids_str = format!(
        "{{{}}}",
        node_ids
            .iter()
            .map(|id| format!("\\\\x{}", hex::encode(id)))
            .collect::<Vec<String>>()
            .join(",")
    );

    #[derive(FromQueryResult, Debug)]
    pub struct NodeCensusInfo {
        pub node_id: Vec<u8>,
        pub data_radius: Vec<u8>,
        pub client: Client,
    }
    let node_infos: HashMap<NodeId, NodeCensusInfo> =
            match NodeCensusInfo::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "
                SELECT DISTINCT ON (node.node_id)
                    node.node_id,
                    closest_census_node.data_radius,
                    closest_census_node.client
                FROM
                    node
                    JOIN node_enr ON node_enr.node_id = node.id
                    CROSS JOIN LATERAL (
                        SELECT census_node.data_radius, census_node.client_name as client, census_node.surveyed_at
                        FROM census_node
                        WHERE census_node.node_enr_id = node_enr.id AND census_node.surveyed_at <= $2::timestamp + INTERVAL '15 minutes'
                        ORDER BY census_node.surveyed_at DESC
                        LIMIT 1
                    ) closest_census_node
                WHERE
                    node.node_id = ANY($1::bytea[])
                ORDER BY
                    node.node_id,
                    closest_census_node.surveyed_at DESC
                ",
                vec![node_ids_str.into(), timestamp.into()],
            ))
            .all(conn)
            .await
            {
                Ok(data) => data
                    .into_iter()
                    // Transform SQL result into a hashmap.
                    .map(|node_census_info| {
                        let mut node_id = [0u8; 32];
                        node_id.copy_from_slice(&node_census_info.node_id);
                        let node_id = NodeId::new(&node_id);
                        (node_id, node_census_info)
                    })
                    .collect(),
                Err(err) => {
                    error!(err=?err, "Failed to lookup radius for traced nodes");
                    HashMap::new()
                }
            };

    // Add radius info to node metadata.
    trace.metadata.iter_mut().for_each(|(node_id, node_info)| {
        if let Some(node_census_info) = node_infos.get(node_id) {
            node_info.radius = Some(B256::from_slice(&node_census_info.data_radius));
            node_details.insert(*node_id, node_census_info.client.clone());
        }
    });

    node_details
}

/// Modifies query traces by removing already discovered peers from the `responses` field.
///
/// These peers are usually just ignored while doing recursive lookup, and are you useful when
/// looking at the chart.
pub fn audit_trace_only_discovered_nodes(trace: &mut QueryTrace) {
    let mut discovered_peers = HashSet::new();

    // Query responses order by time of arrival
    let ordered_responses = trace
        .responses
        .iter_mut()
        .sorted_by_key(|(_node_id, query_response)| query_response.duration_ms)
        .collect::<Vec<_>>();

    for (node_id, query_response) in ordered_responses {
        if !discovered_peers.contains(node_id) {
            if discovered_peers.is_empty() {
                // This should be local node
                discovered_peers.insert(*node_id);
            } else {
                warn!(
                    %node_id,
                    "Received QueryResponse from peer that is not discovered. This indicates a bug in how trace is created.",
                )
            }
        }

        // Keep only newly discovered peers.
        query_response
            .responded_with
            .retain(|peer| discovered_peers.insert(*peer));
    }
}
