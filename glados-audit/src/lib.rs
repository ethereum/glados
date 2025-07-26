use std::{sync::Arc, vec};

use entity::SelectionStrategy;
use tokio::{
    sync::{
        mpsc::{self, Receiver},
        Semaphore,
    },
    time::{interval, Duration, MissedTickBehavior},
};
use tracing::debug;

use crate::{config::AuditConfig, strategy::execute_audit_strategy, task::AuditTask};

pub mod cli;
pub mod config;
pub mod stats;
mod strategy;
mod task;
pub(crate) mod validation;

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
        tokio::spawn(execute_audit_strategy(strategy, tx, config.clone()));
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
                let client = clients.next().expect("No clients").clone();
                let conn = config.database_connection.clone();
                tokio::spawn(async move { task.perform_audit(audit_permit, client, conn).await })
            }
            None => {
                continue;
            }
        };
    }
}
