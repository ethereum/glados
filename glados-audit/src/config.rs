use std::{collections::HashMap, thread::available_parallelism, time::Duration};

use entity::content_audit::SelectionStrategy;
use glados_core::jsonrpc::PortalClient;
use sea_orm::{Database, DatabaseConnection};
use tracing::{debug, info, warn};

use crate::cli::{Args, StrategyWithWeight};

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// For connection to the database.
    pub database_connection: DatabaseConnection,
    /// Specific audit strategies to run, and their weights.
    pub strategies: HashMap<SelectionStrategy, u8>,
    /// Number requests to a Portal node active at the same time.
    pub concurrency: usize,
    /// The maximum number of audits per second.
    pub max_audit_rate: usize,
    /// Portal Clients
    pub portal_clients: Vec<PortalClient>,
    /// The frequency of recording the current audit performance in audit_stats table.
    pub stats_recording_period: Duration,
}

impl AuditConfig {
    pub async fn from_args(args: Args) -> anyhow::Result<AuditConfig> {
        debug!(database_url = &args.database_url, "Connecting to database");
        let database_connection = Database::connect(&args.database_url).await?;
        info!(database_url = &args.database_url, "Connected to database");

        let strategies = args
            .strategy
            .iter()
            .map(StrategyWithWeight::as_tuple)
            .collect();

        let parallelism = available_parallelism()?.get();
        if args.concurrency > parallelism {
            warn!(
                selected.concurrency = args.concurrency,
                system.concurrency = parallelism,
                "Selected concurrency greater than system concurrency."
            )
        } else {
            info!(
                selected.concurrency = args.concurrency,
                system.concurrency = parallelism,
                "Selected concurrency set."
            )
        }

        let mut portal_clients: Vec<PortalClient> = vec![];
        for client_url in args.portal_client {
            let client = PortalClient::new(client_url, &database_connection).await?;
            info!(
                "Found a portal client: {:?}",
                client.client_info.version_info
            );
            portal_clients.push(client);
        }

        Ok(AuditConfig {
            database_connection,
            strategies,
            concurrency: args.concurrency,
            max_audit_rate: args.max_audit_rate,
            portal_clients,
            stats_recording_period: Duration::from_secs(args.stats_recording_period),
        })
    }
}
