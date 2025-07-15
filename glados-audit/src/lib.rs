use anyhow::Result;
use chrono::Utc;
use cli::Args;
use ethportal_api::types::query_trace::QueryTrace;
use ethportal_api::{utils::bytes::hex_encode, HistoryContentKey, OverlayContentKey};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread::available_parallelism,
    vec,
};

use tokio::{
    sync::mpsc::{self, Receiver},
    time::{sleep, Duration},
};
use tracing::{debug, error, info, warn};

use entity::{
    audit_internal_failure, client_info,
    content::{self, SubProtocol},
    content_audit::{self, HistorySelectionStrategy, SelectionStrategy},
    execution_metadata, node,
};
use glados_core::jsonrpc::PortalClient;

use crate::{selection::start_audit_selection_task, validation::content_is_valid};

pub mod cli;
pub(crate) mod selection;
pub mod stats;
pub(crate) mod validation;

/// Configuration created from CLI arguments.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// For Glados-related data.
    pub database_url: String,
    /// Specific audit strategies to run, and their weights.
    pub strategies: HashMap<SelectionStrategy, u8>,
    /// Number requests to a Portal node active at the same time.
    pub concurrency: u8,
    /// Portal Clients
    pub portal_clients: Vec<PortalClient>,
    /// Number of seconds between recording the current audit performance in audit_stats table.
    pub stats_recording_period: u64,
}

impl AuditConfig {
    pub async fn from_args(args: Args) -> Result<AuditConfig> {
        let parallelism = available_parallelism()?.get() as u8;
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

        let strategies = args
            .strategy
            .iter()
            .map(|strategy_with_weight| {
                (
                    strategy_with_weight.strategy.clone(),
                    strategy_with_weight.weight,
                )
            })
            .collect();

        let mut portal_clients: Vec<PortalClient> = vec![];
        for client_url in args.portal_client {
            let client = PortalClient::from(client_url).await?;
            info!("Found a portal client with type: {:?}", client.client_info);
            portal_clients.push(client);
        }
        Ok(AuditConfig {
            database_url: args.database_url,
            strategies,
            concurrency: args.concurrency,
            portal_clients,
            stats_recording_period: args.stats_recording_period,
        })
    }
}

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

pub async fn run_glados_command(conn: DatabaseConnection, command: cli::Command) -> Result<()> {
    let (content_key, portal_client) = match command {
        cli::Command::Audit {
            content_key,
            portal_client,
            ..
        } => (content_key, portal_client),
    };
    let content_key =
        HistoryContentKey::try_from_hex(&content_key).expect("needs valid hex-encoded history key");

    let task = AuditTask {
        strategy: SelectionStrategy::History(HistorySelectionStrategy::SpecificContentKey),
        content: content::get_or_create(SubProtocol::History, &content_key, Utc::now(), &conn)
            .await?,
    };
    let client = PortalClient::from(portal_client).await?;
    let active_threads = Arc::new(AtomicU8::new(0));
    perform_single_audit(active_threads, task, client.clone(), conn).await;
    Ok(())
}

pub async fn run_glados_audit(conn: DatabaseConnection, config: AuditConfig) {
    start_audit(conn.clone(), config).await;

    debug!("setting up CTRL+C listener");
    tokio::signal::ctrl_c()
        .await
        .expect("failed to pause until ctrl-c");

    info!("got CTRL+C. shutting down...");
    std::process::exit(0);
}

async fn start_audit(conn: DatabaseConnection, config: AuditConfig) {
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
        tokio::spawn(start_audit_selection_task(strategy, tx, conn.clone()));
    }
    // Collation of generated tasks, taken proportional to weights.
    let (collation_tx, collation_rx) = mpsc::channel::<AuditTask>(100);
    tokio::spawn(start_collation(collation_tx, task_channels));
    // Perform collated audit tasks.
    tokio::spawn(perform_content_audits(config, collation_rx, conn));
}

/// Listens to tasks coming on different strategy channels and selects
/// according to strategy weight. Collated audit tasks are sent in a single
/// channel for completion.
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
            // Limit check for new tasks to 5/sec
            sleep(Duration::from_millis(200)).await;
        }
    }
}

async fn perform_content_audits(
    config: AuditConfig,
    mut rx: mpsc::Receiver<AuditTask>,
    conn: DatabaseConnection,
) {
    let concurrency = config.concurrency;
    let active_threads = Arc::new(AtomicU8::new(0));

    let mut cycle_of_clients = config.portal_clients.iter().cycle();

    loop {
        let active_count = active_threads.load(Ordering::Relaxed);
        if active_count >= concurrency {
            // Each audit is performed in new thread if enough concurrency is available.
            debug!(
                active.threads = active_count,
                max.threads = concurrency,
                "Waiting for responses on all audit threads... Sleeping..."
            );
            sleep(Duration::from_millis(5000)).await;
            continue;
        }

        debug!(
            active.threads = active_count,
            max.threads = concurrency,
            "Checking Rx channel for audits"
        );

        match rx.recv().await {
            Some(task) => {
                active_threads.fetch_add(1, Ordering::Relaxed);
                let client = match cycle_of_clients.next() {
                    Some(client) => client,
                    None => {
                        error!("Empty list of clients for audit.");
                        return;
                    }
                };
                tokio::spawn(perform_single_audit(
                    active_threads.clone(),
                    task,
                    client.clone(),
                    conn.clone(),
                ))
            }
            None => {
                continue;
            }
        };
    }
}

/// Performs an audit against a Portal node.
///
/// After auditing finishes the thread counter is deprecated. This
/// applies even if the audit process encounters an error.
async fn perform_single_audit(
    active_threads: Arc<AtomicU8>,
    task: AuditTask,
    client: PortalClient,
    conn: DatabaseConnection,
) {
    let client_info = client.client_info.clone();

    debug!(
        content.key = hex_encode(&task.content.content_key),
        client.url =? client.api.client,
        "auditing content",
    );
    let (content_response, trace) = if client.supports_trace() {
        match client.api.get_content_with_trace(&task.content).await {
            Ok((content_response, query_trace)) => (content_response, Some(query_trace)),
            Err(e) => {
                error!(
                    content.key=hex_encode(&task.content.content_key),
                    err=?e,
                    "Problem requesting content with trace from Portal node."
                );
                active_threads.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        }
    } else {
        match client.api.get_content(&task.content).await {
            Ok(c) => (c, None),
            Err(e) => {
                error!(
                    content.key=hex_encode(task.content.content_key),
                    err=?e,
                    "Problem requesting content from Portal node."
                );
                active_threads.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        }
    };

    // If content was absent audit result is 'fail'.
    let audit_result = match content_response {
        Some(content_bytes) => content_is_valid(&task.content, &content_bytes.raw),
        None => false,
    };

    let client_info_id = match client_info::get_or_create(client_info, &conn).await {
        Ok(client_info) => client_info.id,
        Err(error) => {
            error!(content.key=?task.content,
                err=?error,
                "Could not create/lookup client info in db."
            );
            return;
        }
    };

    let node_id = match node::get_or_create(client.enr.node_id(), &conn).await {
        Ok(enr) => enr.id,
        Err(err) => {
            error!(
                err=?err,
                "Failed to created node."
            );
            return;
        }
    };

    let created_audit = content_audit::create(
        task.content.id,
        client_info_id,
        node_id,
        audit_result,
        task.strategy,
        json!(trace).to_string(),
        &conn,
    )
    .await;

    let created_audit: content_audit::Model = match created_audit {
        Ok(inserted_audit) => inserted_audit,
        Err(e) => {
            error!(
                content.key=?task.content,
                err=?e,
                "Could not create audit entry in db."
            );
            active_threads.fetch_sub(1, Ordering::Relaxed);
            return;
        }
    };

    if let Some(trace) = trace {
        create_entry_for_failures(created_audit, trace, &conn).await;
    }

    // Display audit result.
    match task.content.protocol_id {
        SubProtocol::History => {
            display_history_audit_result(task.content, audit_result, &conn).await;
        }
    }

    active_threads.fetch_sub(1, Ordering::Relaxed);
}

// For each transfer failure in the trace, create a new entry in the database.
async fn create_entry_for_failures(
    audit: content_audit::Model,
    trace: QueryTrace,
    conn: &DatabaseConnection,
) {
    // Create a list of the failures from the parsed trace json
    for (sender_node_id, failure_entry) in trace.failures.into_iter() {
        let fail_type = failure_entry.failure;
        info!("Found a new transfer failure: Sender: {sender_node_id}, FailureType: {fail_type:?}, Audit ID: {}, recipient_client_info_id: {:?}", audit.id, audit.client_info);

        // Get the ENR for the sender node
        let sender_enr = match trace.metadata.get(&sender_node_id) {
            Some(node_info) => &node_info.enr,
            None => {
                error!(
                    "In audit {}, sender ENR for node {sender_node_id} was not found in metadata",
                    audit.id,
                );
                continue;
            }
        };

        if let Err(e) =
            audit_internal_failure::create(audit.id, sender_enr, fail_type.into(), conn).await
        {
            error!(
                err=?e,
                "Failed to insert audit transfer failure into database"
            );
        }
    }
}

async fn display_history_audit_result(
    content: content::Model,
    audit_result: bool,
    conn: &DatabaseConnection,
) {
    match execution_metadata::get(content.id, conn).await {
        Ok(Some(b)) => {
            info!(
                content.key=hex_encode(content.content_key),
                audit.pass=?audit_result,
                block = b.block_number,
                "History content audit"
            );
        }
        Ok(None) => {
            info!(
                content.key=hex_encode(content.content_key),
                audit.pass=?audit_result,
                "Block metadata absent for history key."
            );
        }
        Err(e) => error!(
                    content.key=hex_encode(content.content_key),
                    err=?e,
                    "Problem getting block metadata for history key."),
    };
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use enr::NodeId;
    use entity::content_audit::HistorySelectionStrategy;
    use entity::execution_metadata;
    use entity::{
        audit_internal_failure::{self, TransferFailureType},
        client_info,
        content::{self, SubProtocol},
        content_audit::{self, AuditResult},
        node,
        prelude::*,
        record,
    };
    use ethportal_api::utils::bytes::hex_decode;
    use migration::{DbErr, Migrator, MigratorTrait};
    use pgtemp::PgTempDB;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Database, DbBackend, DbConn,
        EntityTrait, FromQueryResult, QueryFilter, Set, Statement,
    };

    use super::*;

    // TODO move to utility, mereg with other uses of this functionality
    /// Creates a temporary Postgres database that will be deleted once the PgTempDB goes out of scope.
    async fn setup_database() -> Result<(DbConn, PgTempDB), DbErr> {
        let pgtemp = PgTempDB::async_new().await;
        let conn: DbConn = Database::connect(&pgtemp.connection_uri()).await?;
        Migrator::up(&conn, None).await.unwrap();
        Ok((conn, pgtemp))
    }

    #[tokio::test]
    async fn test_generate_internal_failures() {
        let (conn, _temp_db, audit) = get_populated_test_audit_db().await.unwrap();

        // Deserialize the trace into a QueryTrace object
        let trace: QueryTrace = serde_json::from_str(&audit.trace).unwrap();

        let audit_id = audit.id;
        create_entry_for_failures(audit, trace, &conn).await;

        // Find the 3 created failures, that must match the audit id
        let failures = AuditInternalFailure::find()
            .filter(audit_internal_failure::Column::Audit.eq(audit_id))
            .all(&conn)
            .await
            .unwrap();
        assert_eq!(failures.len(), 3);

        // The three failures to expect:
        //  "0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c":
        //      {"durationMs":16147,"failure":"utpTransferFailed"},
        //  "0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c":
        //      {"durationMs":36149,"failure":"invalidContent"},
        //  "0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7":
        //      {"durationMs":26148,"failure":"utpConnectionFailed"}
        for failure in failures.into_iter() {
            let sender_record = Record::find_by_id(failure.sender_record_id)
                .one(&conn)
                .await
                .unwrap()
                .unwrap();
            let sender_node = Node::find_by_id(sender_record.node_id)
                .one(&conn)
                .await
                .unwrap()
                .unwrap();
            match hex_encode(sender_node.node_id).as_str() {
                "0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c" => {
                    assert_eq!(failure.failure_type, TransferFailureType::UtpTransferFailed);
                }
                "0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c" => {
                    assert_eq!(failure.failure_type, TransferFailureType::InvalidContent);
                }
                "0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7" => {
                    assert_eq!(
                        failure.failure_type,
                        TransferFailureType::UtpConnectionFailed
                    );
                }
                _ => panic!("Unexpected failure"),
            }
        }
    }

    #[tokio::test]
    async fn test_internal_failures_with_records() {
        let (conn, _temp_db, audit) = get_populated_test_audit_db().await.unwrap();

        // Deserialize the trace into a QueryTrace object
        let trace: QueryTrace = serde_json::from_str(&audit.trace).unwrap();

        // Create records for one of the sender nodes
        // The purpose of the test is to confirm that the failure entries identify and store the
        // link to the matching associated with each sender node.

        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(
            &hex_decode("0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c")
                .unwrap(),
        );
        let node = node::get_or_create(NodeId::new(&node_id), &conn)
            .await
            .unwrap();

        // Create some false records for the node, including a sequence number higher than the one
        // specified in the query trace. These are red herrings, designed to catch if the logic is
        // picking up the wrong record to tie to the transfer failure. We should end up with
        // sequence_number 4, not 5 or 3.
        Record::insert_many(vec![
            record::ActiveModel {
                id: NotSet,
                node_id: Set(node.id),
                sequence_number: Set(5),
                raw: Set("5".to_string()),
            },
            record::ActiveModel {
                id: NotSet,
                node_id: Set(node.id),
                sequence_number: Set(3),
                raw: Set("3".to_string()),
            },
        ])
        .exec(&conn)
        .await
        .unwrap();

        // Generate the transfer failure rows
        create_entry_for_failures(audit, trace, &conn).await;

        // Find the record that matches a created failure and our node of interest
        let sender_record = record::Model::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT r.*
                FROM record AS r
                LEFT JOIN node AS n ON n.id = r.node_id
                LEFT JOIN audit_internal_failure AS aif ON aif.sender_record_id = r.id
                WHERE n.id = $1 AND aif.id IS NOT NULL",
            vec![node.id.into()],
        ))
        .one(&conn)
        .await
        .unwrap()
        .unwrap();

        // If you decode the ENR for this node ID you'll find a sequence number of 4.
        // This can be verified by finding the ENR in the metadata for this test's fixture:
        // enr:-I24QNw9C_xJvljho0dO27ug7-wZg7KCN1Mmqefdvqwxxqw3X-SLzBO3-KvzCbGFFJJMDn1be6Hd-Bf_TR3afjrwZ7UEY4d1IDAuMC4xgmlkgnY0gmlwhKRc9-KJc2VjcDI1NmsxoQJMpHmGj1xSP1O-Mffk_jYIHVcg6tY5_CjmWVg1gJEsPIN1ZHCCE4o
        assert_eq!(sender_record.sequence_number, 4);
    }

    async fn get_populated_test_audit_db() -> Result<(DbConn, PgTempDB, content_audit::Model), DbErr>
    {
        let (conn, temp_db) = setup_database().await?;

        let block_hash = [0xFF; 32];
        let content_key = HistoryContentKey::new_block_header_by_hash(block_hash);
        let available_at = Utc::now();
        let block_number = 1234;
        let content_key_active_model = content::ActiveModel {
            id: NotSet,
            content_id: Set(content_key.content_id().to_vec()),
            content_key: Set(content_key.to_bytes().to_vec()),
            first_available_at: Set(available_at),
            protocol_id: Set(SubProtocol::History),
        };
        let content_key_model = content_key_active_model.insert(&conn).await?;

        let execution_metadata = execution_metadata::ActiveModel {
            id: NotSet,
            content: Set(content_key_model.id),
            block_number: Set(block_number),
        };
        let _ = execution_metadata.insert(&conn).await?;

        let client_info_active_model = client_info::ActiveModel {
            id: NotSet,
            version_info: Set("trin v0.1.0".to_owned()),
        };

        let node_id = NodeId::random();
        let node = node::get_or_create(node_id, &conn).await.unwrap();

        let client_info_model = client_info_active_model.insert(&conn).await?;

        // successful audit result
        let audit_result = AuditResult::Success;
        // Create a trace with all three internal failure types
        let trace_string = r#"{"receivedFrom":null,"origin":"0x9759131b53dfdb369190514c8f6926896b8a96c633dd724cf725d099e181c666","responses":{"0x93c07e5f3adbf648ec7b11023107dacddb9340af673cb56a5a912ebb316d9bd8":{"durationMs":36239,"respondedWith":[]},"0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c":{"durationMs":194,"respondedWith":[]},"0x9223dde3ed71f62f938c7fc6841f4ca34048a8d08b79efea689a5546178fa126":{"durationMs":36159,"respondedWith":[]},"0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c":{"durationMs":253,"respondedWith":[]},"0x9e7dc510a091a46a07a9d337f4b451eee499503b194b80228a7b9715c579e865":{"durationMs":36280,"respondedWith":[]},"0x9ea4a21e76c31532001768d2881ac816a7bcfc96fdb3a618d2243b16ed47a270":{"durationMs":36391,"respondedWith":[]},"0x9e019b690d7a097894200bb8f2b7dd69d5438cb9c26a3eadb698ba51623d9749":{"durationMs":36369,"respondedWith":[]},"0x996c0ca458d4b21b422adbaa5bae7ef3c714b95b1050ecf4c35de26b61f6dd21":{"durationMs":36231,"respondedWith":[]},"0x984948788690c4f8f13fdb5b7fbb6998f803f7b0deff2d128d9d1cfba49874cb":{"durationMs":36243,"respondedWith":[]},"0x95b0bd504d53fe69b8764e3cf70f45f19cfcebb45631ab18ee112bf9a8bbc619":{"durationMs":6,"respondedWith":["0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c"]},"0x9c57077b168944507feade6be080bb62cafea6bd875ca87054fd71b72945e7ed":{"durationMs":36259,"respondedWith":[]},"0x97756e00cd260f23f9d102e77f3b1527aa40379e08e73e529819bdc358bced66":{"durationMs":36154,"respondedWith":[]},"0x95efb8a26d982460782cdb7570ec947a92a4fe92169d9ae599768efa2c042508":{"durationMs":103,"respondedWith":[]},"0x9759131b53dfdb369190514c8f6926896b8a96c633dd724cf725d099e181c666":{"durationMs":0,"respondedWith":["0x95efb8a26d982460782cdb7570ec947a92a4fe92169d9ae599768efa2c042508","0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c","0x95b0bd504d53fe69b8764e3cf70f45f19cfcebb45631ab18ee112bf9a8bbc619","0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7","0x97756e00cd260f23f9d102e77f3b1527aa40379e08e73e529819bdc358bced66","0x967c9eef512c10e1ad94b47d7b8f1443328e837fc21025a79bd75e8c0316237e","0x9136c1554dd223041c844b5ccb27fcb0d536426009d43a3e29e427472764b7cb","0x93c07e5f3adbf648ec7b11023107dacddb9340af673cb56a5a912ebb316d9bd8","0x9223dde3ed71f62f938c7fc6841f4ca34048a8d08b79efea689a5546178fa126","0x9c57077b168944507feade6be080bb62cafea6bd875ca87054fd71b72945e7ed","0x9ea4a21e76c31532001768d2881ac816a7bcfc96fdb3a618d2243b16ed47a270","0x9e7dc510a091a46a07a9d337f4b451eee499503b194b80228a7b9715c579e865","0x9e019b690d7a097894200bb8f2b7dd69d5438cb9c26a3eadb698ba51623d9749","0x996c0ca458d4b21b422adbaa5bae7ef3c714b95b1050ecf4c35de26b61f6dd21","0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5","0x984948788690c4f8f13fdb5b7fbb6998f803f7b0deff2d128d9d1cfba49874cb"]},"0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7":{"durationMs":227,"respondedWith":[]},"0x9136c1554dd223041c844b5ccb27fcb0d536426009d43a3e29e427472764b7cb":{"durationMs":36263,"respondedWith":[]},"0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5":{"durationMs":36278,"respondedWith":[]},"0x967c9eef512c10e1ad94b47d7b8f1443328e837fc21025a79bd75e8c0316237e":{"durationMs":36373,"respondedWith":[]}},"failures":{"0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c":{"durationMs":16147,"failure":"utpTransferFailed"},"0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c":{"durationMs":36149,"failure":"invalidContent"},"0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7":{"durationMs":26148,"failure":"utpConnectionFailed"}},"metadata":{"0x97756e00cd260f23f9d102e77f3b1527aa40379e08e73e529819bdc358bced66":{"enr":"enr:-JO4QKEwcAEGtIrz_NOSQKEY0AemFVa_GBuWCHlcRFjbES0-dX_GQ3H8-DuEQMGtKBQZB6PZx2LF6JE-EUiuU9vPzzmEZ7LIOGOJdCA0YzlhNzJigmlkgnY0gmlwhIbRTKyJc2VjcDI1NmsxoQP_Nt24JpQP_6mkru8Otqh8zFPTyAGB3RQaJbA_6Y1z0oN1ZHCCIzE","distance":"0x02b32797c4c960d811163068cc30c51433c8bba1f4485e462379619520886867","radius":"0x0000000000000000000000000000000000000000000000000000000000000000"},"0x967c9eef512c10e1ad94b47d7b8f1443328e837fc21025a79bd75e8c0316237e":{"enr":"enr:-JO4QA5SNXNSuON6WmtuNw9g97EsEC7Tw1nwfCxb3uRXnyXEf9yvNpqHpjl6IixNMtGGHko6dT7GfE_Sn_qrnB_IzfGEZ7BbQ2OJdCAxOTU1NmMxgmlkgnY0gmlwhDmAPUmJc2VjcDI1NmsxoQJI6UfMkG4OTdUdXW1Xt3crFLTP8H13nBK-f-fciWOS2oN1ZHCCIzE","distance":"0x03bad77858c37f1a455386f2c884c470ab060f403ebf45b320b782da7b22a67f","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x9223dde3ed71f62f938c7fc6841f4ca34048a8d08b79efea689a5546178fa126":{"enr":"enr:-JO4QJY_2HYZepk-uzxcsEW2RO-Ilb0MHdG0CnDJG-bMjVewEaMD5eck2ThryRWUwwpROZi2rVfEqX8Kc6ZMoiN1ON-EZ69tI2OJdCAxOTU1NmMxgmlkgnY0gmlwhGj4OA6Jc2VjcDI1NmsxoQJkTdcMqcmqrznObWcTDZ-ExrC2AZpbtsmki3BUnogOEYN1ZHCCIzE","distance":"0x07e59474e49e99d47b4b4d4937149c90d9c024ef77d68ffed3fa89106fbb2427","radius":"0x04f7374ba0fcbcc8404744f56482dfa9dfddf214cce5aa7e53a93e46bb7d9998"},"0x9136c1554dd223041c844b5ccb27fcb0d536426009d43a3e29e427472764b7cb":{"enr":"enr:-JO4QMBcRMjjOWeaXKhgS_T8mArZdypXutG6QruhbBRKBblEJ81bypvUu0C6JZpyB24T2Sf-8SNWWQTO8rYlvrFwWm-EZ6KCzGOJdCA3OTM2OWEwgmlkgnY0gmlwhJ_f7oaJc2VjcDI1NmsxoQLVQrKv95_T5BylvcppNb3Ur52qKS-CyMp84NrV0CtuWIN1ZHCCIzE","distance":"0x04f088c2443d4cfff44379d3782c2c834cbece5ff57b5a2a9284fb115f5032ca","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x95f8b17e1700b74f4ebad4bb5de0564673aeb10b199bd19e65426f7642e9fd3c":{"enr":"enr:-JO4QDQIszdWhSX4Un09K3so4A1DC9IQmK9-Sx_LVWWz6ZUuWB2ytpuhA6eg9UO4cOuDw-4wRjYa2Asl01uBGs3J8_aEZ69tNWOJdCAxOTU1NmMxgmlkgnY0gmlwhLKAN72Jc2VjcDI1NmsxoQKm3DEukdqVUWj71I43Nry4YedICKpncxICgM-IzxHtb4N1ZHCCIzE","distance":"0x003ef8e91eefd8b4a67de634eeeb8675ea263d34e534b18ade22b3203add783d","radius":"0x04f2d2eb1574e06ebc79a92e81ab9d52d6efbc59a48c0548f26075891ec0a812"},"0x95c8df0a57c901c5d4561403a77067996b03b64e7beb1bdee662fae236d2188c":{"enr":"enr:-I24QNw9C_xJvljho0dO27ug7-wZg7KCN1Mmqefdvqwxxqw3X-SLzBO3-KvzCbGFFJJMDn1be6Hd-Bf_TR3afjrwZ7UEY4d1IDAuMC4xgmlkgnY0gmlwhKRc9-KJc2VjcDI1NmsxoQJMpHmGj1xSP1O-Mffk_jYIHVcg6tY5_CjmWVg1gJEsPIN1ZHCCE4o","distance":"0x000e969d5e266e3e3c91268c147bb7aaf28b3a7187447bca5d0226b44ee69d8d","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x9c57077b168944507feade6be080bb62cafea6bd875ca87054fd71b72945e7ed":{"enr":"enr:-JO4QNGWjRCi7a0Oc1ZKTnfcgoIQwXD3WsmWgdisp8OIGzBAcqYAhaKHgFqAxOmFIwyJbUuVs1V6phgc3heXSYIU-QCEZ6U_amOJdCA5ODcwZTk5gmlkgnY0gmlwhKRc2YqJc2VjcDI1NmsxoQIU3hcK8PgfagpRyswRQGtGsTFietMSL3UfFqTh3Xkq6oN1ZHCCIzE","distance":"0x09914eec1f662bab972dece4538b6b5153762a827bf3c864ef9dade1517162ec","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x9e019b690d7a097894200bb8f2b7dd69d5438cb9c26a3eadb698ba51623d9749":{"enr":"enr:-JK4QKgXqKXulRTzc6oCHs-nP3j1VC57EPz5VXg3XtlR8lFXZ0NBz8-4CHQTT6c-Q9ATe-x6nLlsQI8RYiU4EKGwIuCGAZT0FQ08Y4ZzaGlzdWmCaWSCdjSCaXCEQW1FYolzZWNwMjU2azGhAit9RUv-oakSH8EY2tkjswHjktJ_9IMusw4eKlI3Zh0ag3VkcIJxVQ","distance":"0x0bc7d2fe049566837ce7393741bc0d5a4ccb00863ec55eb90df866071a091248","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x93c07e5f3adbf648ec7b11023107dacddb9340af673cb56a5a912ebb316d9bd8":{"enr":"enr:-JO4QDRuRa8NlOY78sQwSVXIv9T3NS2Z4grJ7DKYfzh0ds0SRXOFVAFl3O9EBM9D4Wmp39JXfslPp-ZE8CavKU0NwzWEZ69tJWOJdCAxOTU1NmMxgmlkgnY0gmlwhLI-5QWJc2VjcDI1NmsxoQKoT5qcsVvx-sqwdfftK-aiglhfodxttWsWTMTw54_ag4N1ZHCCIzE","distance":"0x060637c8333499b304bc238d820c0afe421bcc909b93d57ee1f1f2ed49591ed9","radius":"0x0576962045628af83df39df90f4fca76070e34f0a6e2deabccfd91caa3fe8bee"},"0x944ca5359881d5bd8ab044ea89517aaefc006b2ec2da406b14bb4eea780cb5e7":{"enr":"enr:-Ii4QEXbeGRDdBwlNte_6hCW5BkzoPIueIcQAFY5Z9x87ickUDOERUSCWuJbatmvMP8kH9OKEJp9hmHvJtsa_YeyiKCCF_VjZoJpZIJ2NIJpcITCISs0iXNlY3AyNTZrMaED3PAPsE8he5cg4keKfVZQGd3nry1jU5HA4dVBIWko3JWDdWRwgiOM","distance":"0x018aeca2916eba46627776653a5aaa9d6588e7113e75207fafdb92bc003830e6","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x95efb8a26d982460782cdb7570ec947a92a4fe92169d9ae599768efa2c042508":{"enr":"enr:-JO4QBNoKEb7vrFPoSdpisZ-xnJ6GSdUVl2Y2wd8aPGmVyYVchqB6vtALGn5B0B5SjDehZ57XOMZMnM1K7cPLnt-eueEZ6KCzGOJdCA3OTM2OWEwgmlkgnY0gmlwhIZ6Nu2Jc2VjcDI1NmsxoQJ65XCkLhQKNfzddg9Ugm5rV6moggutaInxY1WxFCK4nIN1ZHCCIzE","distance":"0x0029f13564774b9b90ebe9fac3e744490b2c72adea32faf1221652ac5430a009","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x9ea4a21e76c31532001768d2881ac816a7bcfc96fdb3a618d2243b16ed47a270":{"enr":"enr:-JO4QH958vF7uYyUkyBiW_o0YUOh1_Gvrz3wS9xkCPnt6VY5SRG8EUqDoCA11eFuTVDOuNrOPic3snu2anneu1yokfKEZ69tNWOJdCAxOTU1NmMxgmlkgnY0gmlwhJ_fIqSJc2VjcDI1NmsxoQK6JpkVhP_qZNnxIbvZoUD3c7cWa7nsWFd4hUqy_0NOEIN1ZHCCIzE","distance":"0x0b62eb897f2c7ac9e8d05a5d3b1118253e3470a9011cc60c6944e74095732771","radius":"0x04e78d8d7b7940ad52266a20f268d94ada9eae4624f2ddd6597a9d3706757b3c"},"0x9759131b53dfdb369190514c8f6926896b8a96c633dd724cf725d099e181c666":{"enr":"enr:-JO4QA8TH8rP6Jk8RWR9pr8AJ-QCeJHwHCeDtxc8QK02x2ATDtea6ZQLMzIGONE-4udmfUeiwFrWrJDCOJ7_k6gnBOKEZ69urGOJdCA3Y2IyMDYzgmlkgnY0gmlwhKes4RSJc2VjcDI1NmsxoQOl0EddqO8i9qB-m3ooZuFq_g3gat9Ux57SOn0iisk49oN1ZHCCIzE","distance":"0x029f5a8c5a30b4cd795763c33c62f6baf2021af9cf7212584c450ccf99b54367","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x95b0bd504d53fe69b8764e3cf70f45f19cfcebb45631ab18ee112bf9a8bbc619":{"enr":"enr:-JO4QJKv-NK2NImnbhPd_hXXy1c216QVSKgvAr2g2MhpDDuVQQEABxWScLU35kYsA8RCsB_YFlynpbgwliK3tOVAfiiEZ6U_pGOJdCA5ODcwZTk5gmlkgnY0gmlwhKLzp8uJc2VjcDI1NmsxoQJiYtkgbieoOh1iUcO8ol-L6MY_Ik46LYLyg7c8OUhw1YN1ZHCCIzE","distance":"0x0076f4c744bc919250b17cb3440495c20574678baa9ecb0c5571f7afd08f4318","radius":"0x00f1a1bb621221c63720552b994237b19c19b35908bdd8eeb0a68ea51bba80c9"},"0x9891fe3fdfcec2707f877306fc6e5596b1405fedf70ad2911496c2ac40dfeda5":{"enr":"enr:-Ii4QAEb-wSaHXuh_7rqKP0YnuITG0C359ZO_k7sipTDdtxBUwGIHZB_zqLS248owpDzcLhRuAhENWZztUSkxwk-c9uCHZpjZoJpZIJ2NIJpcITCIStAiXNlY3AyNTZrMaEDjt4BbmggMkCrHGojSwvR3y21cxXSHqxEhZThnBLilh-DdWRwgiOM","distance":"0x0d57b7a8d621ad8b974041894f6585a528c8d3d20ba5b285aff61efa38eb68a4","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x996c0ca458d4b21b422adbaa5bae7ef3c714b95b1050ecf4c35de26b61f6dd21":{"enr":"enr:-JO4QKvgdlyCkEro17JfxXu4KDyaOKj_QtKLaMF3ZbH8GUu9SG8wYDE4VG3Tz27NnSLu29xAQvJm5rEyYd30r4h6qBOEZ69tJWOJdCAxOTU1NmMxgmlkgnY0gmlwhLymaYGJc2VjcDI1NmsxoQI8Re2Z3O6cGwzX_kzzl9gYzsakPJo9cbHcSHkR74jzo4N1ZHCCIzE","distance":"0x0caa4533513bdde0aaede925e8a5aec05e9c3564ecff8ce0783d3e3d19c25820","radius":"0x050ff80b430a52fec3eedbc3ac3a195c4157821420cdc3c1e96a4306f12a20d4"},"0x9e7dc510a091a46a07a9d337f4b451eee499503b194b80228a7b9715c579e865":{"enr":"enr:-Ii4QN7q9m-cSvRhAfumViJOWQ7Hzjbg1EXVAnY47SEil-X4M0PTanPSU9El1rtPwfFOs2TT6VQXrLh9PUILd7bxKSaCG2djZoJpZIJ2NIJpcITCISsgiXNlY3AyNTZrMaECedPKSKkarI7L5lEH2Br2lBU8X7BCz7KP-thSg6pcSNuDdWRwgiOM","distance":"0x0bbb8c87a97ecb91ef6ee1b847bf81dd7d11dc04e5e4e036311b4b43bd4d6d64","radius":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"0x984948788690c4f8f13fdb5b7fbb6998f803f7b0deff2d128d9d1cfba49874cb":{"enr":"enr:-JO4QBw48MgDAPjjufUgu8jP3EQKIe8SvBbdnZATmp_7cJu6QB0NpJW8RcrCvO0TLZQJAn5VHZ_6V9vJ23frJVcrmviEZ69tJWOJdCAxOTU1NmMxgmlkgnY0gmlwhKRc0OOJc2VjcDI1NmsxoQIp1pXlJLKqWipuaHE1EROa1YY-CKXPhFaBRSGeyYAdXoN1ZHCCIzE","distance":"0x0d8f01ef8f7fab0319f8e9d4ccb0b9ab618b7b8f22504d0636fdc0addcacf1ca","radius":"0x04dc1661bc6f34d56e2249fb7eea6e21fbd3803929c740238c09246e459d96ca"}},"startedAtMs":1739786826151,"targetId":"0x95c6499709ef6ffbe8c7328fb30bd03399888c3ffcaf6014bb60dc5678348501","cancelled":[]}"#;
        let content_audit_active_model = content_audit::ActiveModel {
            id: NotSet,
            content_key: Set(content_key_model.id),
            created_at: Set(Utc::now()),
            strategy_used: Set(Some(SelectionStrategy::History(
                HistorySelectionStrategy::Random,
            ))),
            result: Set(audit_result),
            trace: Set(trace_string.to_owned()),
            client_info: Set(Some(client_info_model.id)),
            node: Set(Some(node.id)),
        };
        content_audit_active_model.insert(&conn).await?;

        let loaded_audit = content_audit::Entity::find()
            .filter(content_audit::Column::ContentKey.eq(1))
            .one(&conn)
            .await?
            .unwrap();
        // Verify the premise of the test: a successful audit with a trace string (including failures)
        assert_eq!(loaded_audit.result, AuditResult::Success);
        assert_ne!(loaded_audit.trace, "");
        Ok((conn, temp_db, loaded_audit))
    }
}
