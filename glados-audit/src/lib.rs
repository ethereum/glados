use std::{sync::Arc, vec};

use entity::{
    audit_internal_failure, content,
    content_audit::{self, SelectionStrategy},
};
use ethportal_api::{types::query_trace::QueryTrace, utils::bytes::hex_encode};
use glados_core::jsonrpc::{JsonRpcError, PortalClient};
use sea_orm::DatabaseConnection;
use serde_json::json;
use tokio::{
    sync::{
        mpsc::{self, Receiver},
        OwnedSemaphorePermit, Semaphore,
    },
    time::{interval, Duration, MissedTickBehavior},
};
use tracing::{debug, error, info, warn};

use crate::{config::AuditConfig, strategy::execute_audit_strategy, validation::content_is_valid};

pub mod cli;
pub mod config;
pub mod stats;
mod strategy;
pub(crate) mod validation;

#[derive(Clone, Debug)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content: content::Model,
}

// Associates strategies with their channels and weights.
#[derive(Debug)]
pub struct TaskChannel {
    strategy: SelectionStrategy,
    weight: u8,
    rx: Receiver<AuditTask>,
}

pub async fn run_glados_audit(config: AuditConfig) {
    let mut task_channels: Vec<TaskChannel> = vec![];
    for (strategy, weight) in config.strategies.clone() {
        // Each strategy sends tasks to a separate channel.
        let (tx, rx) = mpsc::channel::<AuditTask>(100);
        let task_channel = TaskChannel {
            strategy: strategy.clone(),
            weight,
            rx,
        };
        task_channels.push(task_channel);

        // Strategies generate tasks in their own thread for their own channel.
        tokio::spawn(execute_audit_strategy(
            strategy,
            tx,
            config.database_connection.clone(),
        ));
    }

    // Collation of generated tasks, taken proportional to weights.
    let (collation_tx, collation_rx) = mpsc::channel::<AuditTask>(100);
    tokio::spawn(start_collation(collation_tx, task_channels));

    // Perform collated audit tasks.
    tokio::spawn(perform_content_audits(config, collation_rx));
}

/// Listens to tasks coming on different strategy channels and selects according to strategy weight.
///
/// Collated audit tasks are sent in a single channel for completion.
async fn start_collation(
    collation_tx: mpsc::Sender<AuditTask>,
    mut task_channels: Vec<TaskChannel>,
) {
    loop {
        for tasks in task_channels.iter_mut() {
            debug!(strategy=?tasks.strategy, max=tasks.weight, "collating");
            for _ in 0..tasks.weight {
                match tasks.rx.try_recv() {
                    Ok(task) => collation_tx
                        .send(task)
                        .await
                        .expect("Unable to collate task"),
                    Err(_) => break,
                }
            }
        }
    }
}

/// Accepts tasks and runs them in parallel as separate tasks.
///
/// Controls the number of audit tasks and frequency that is running concurrently.
async fn perform_content_audits(config: AuditConfig, mut rx: mpsc::Receiver<AuditTask>) {
    // Make sure we don't exceed `max_audit_rate`
    let mut audit_interval = interval(Duration::from_secs(1) / config.max_audit_rate as u32);
    audit_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // Make sure we don't run too many tasks concurrently
    let audit_semaphore = Arc::new(Semaphore::new(config.concurrency));

    let mut clients = config.portal_clients.iter().cycle();

    loop {
        audit_interval.tick().await;

        let audit_permit = audit_semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore shouldn't be closed");

        match rx.recv().await {
            Some(task) => {
                let client = clients.next().expect("No clients");

                tokio::spawn(perform_single_audit(
                    audit_permit,
                    task,
                    client.clone(),
                    config.database_connection.clone(),
                ))
            }
            None => {
                continue;
            }
        };
    }
}

/// Performs a single audit task and saves the result.
///
/// The audit permit is released at the end.
async fn perform_single_audit(
    audit_permit: OwnedSemaphorePermit,
    task: AuditTask,
    client: PortalClient,
    conn: DatabaseConnection,
) {
    debug!(
        strategy = ?task.strategy,
        content.key = hex_encode(&task.content.content_key),
        client.url =? client.api.client,
        "Audit started",
    );

    let (audit_result, trace) = get_and_validate_content(&task, &client).await;

    save_audit_result(&task, audit_result, trace, &client, &conn).await;

    info!(
        strategy = ?task.strategy,
        content.key = hex_encode(&task.content.content_key),
        pass = audit_result,
        "Audit finished",
    );

    drop(audit_permit);
}

async fn get_and_validate_content(
    task: &AuditTask,
    client: &PortalClient,
) -> (bool, Option<QueryTrace>) {
    match client.get_content(&task.content).await {
        Ok((content_bytes, trace)) => (content_is_valid(&task.content, &content_bytes), trace),
        Err(JsonRpcError::ContentNotFound { trace }) => {
            warn!(
                content.key = hex_encode(&task.content.content_key),
                "Content not found."
            );
            (false, trace)
        }
        Err(err) => {
            error!(
                content.key = hex_encode(&task.content.content_key),
                %err,
                "Problem requesting content from Portal node."
            );
            (false, None)
        }
    }
}

async fn save_audit_result(
    task: &AuditTask,
    audit_result: bool,
    trace: Option<QueryTrace>,
    client: &PortalClient,
    conn: &DatabaseConnection,
) {
    let audit = content_audit::create(
        task.content.id,
        client.client_info.id,
        client.node_info.id,
        audit_result,
        task.strategy.clone(),
        json!(trace).to_string(),
        conn,
    )
    .await;

    let audit: content_audit::Model = match audit {
        Ok(audit) => audit,
        Err(err) => {
            error!(
                content.key = hex_encode(&task.content.content_key),
                %err,
                "Could not save audit in db."
            );
            return;
        }
    };

    if let Some(trace) = trace {
        save_transfer_failures(audit, trace, client, conn).await
    }
}

async fn save_transfer_failures(
    audit: content_audit::Model,
    trace: QueryTrace,
    client: &PortalClient,
    conn: &DatabaseConnection,
) {
    // Create a list of the failures from the parsed trace json
    for (sender_node_id, failure_entry) in trace.failures {
        let fail_type = failure_entry.failure;
        info!(
            audit.id = audit.id,
            failure.type = ?fail_type,
            sender.node_id = %sender_node_id,
            receiver = client.client_info.version_info,
            "Found new transfer failure",
        );

        // Get the ENR for the sender node
        let sender_enr = match trace.metadata.get(&sender_node_id) {
            Some(node_info) => &node_info.enr,
            None => {
                error!(
                    audit.id = audit.id,
                    sender.node_id = %sender_node_id,
                    "Sender ENR not found in trace metadata",
                );
                continue;
            }
        };

        if let Err(err) =
            audit_internal_failure::create(audit.id, sender_enr, fail_type.into(), conn).await
        {
            error!(%err, "Failed to insert audit transfer failure into database");
        }
    }
}
