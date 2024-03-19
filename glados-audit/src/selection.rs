use std::collections::HashSet;

use chrono::{DateTime, TimeZone, Utc};
use ethportal_api::HistoryContentKey;
use glados_core::db::store_block_keys;
use rand::{thread_rng, Rng};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, JoinType, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait,
};
use tokio::{
    sync::mpsc,
    time::{interval, Duration},
};
use tracing::{debug, error, warn};

use entity::{
    content::{self, Model},
    content_audit::{self, SelectionStrategy},
    execution_metadata,
};
use web3::types::{BlockId, BlockNumber};

use crate::{AuditConfig, AuditTask};

pub const MERGE_BLOCK_HEIGHT: i32 = 15537393;

pub async fn start_audit_selection_task(
    strategy: SelectionStrategy,
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
    config: AuditConfig,
) {
    match strategy {
        SelectionStrategy::Latest => select_latest_content_for_audit(tx, conn).await,
        SelectionStrategy::Random => select_random_content_for_audit(tx, conn).await,
        SelectionStrategy::FourFours => {
            // Fourfours strategy downloads its own keys rather than waiting on glados-monitor to put them in the DB.
            let w3 = web3::Web3::new(web3::transports::Http::new(&config.provider_url).unwrap());
            select_fourfours_content_for_audit(tx, conn, w3).await
        }
        SelectionStrategy::Failed => warn!("Need to implement SelectionStrategy::Failed"),
        SelectionStrategy::SelectOldestUnaudited => {
            select_oldest_unaudited_content_for_audit(tx, conn).await
        }
        SelectionStrategy::SpecificContentKey => {
            error!("SpecificContentKey is not a valid audit strategy")
        }
    }
}

/// Finds and sends audit tasks for [Strategy::Latest].
///
/// Strategy achieved by:
/// 1. Left joining contentkey table to the contentaudit table to find audits per key.
/// 2. Filter for null audits (Exclude any item with an existing audit).
/// 3. Sort ascending to have most recently added content keys first.
/// 4. Filter for content that is older than n seconds to allow the network a chance to propagate the content.
///
/// At regular intervals the channel capacity is assessed and new tasks are added to reach capacity.
async fn select_latest_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process for 'latest' strategy");
    let mut interval = interval(Duration::from_secs(10));

    loop {
        interval.tick().await;
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }
        let keys_required = tx.capacity();
        if keys_required == 0 {
            continue;
        };
        let content_key_db_entries = match content::Entity::find()
            .left_join(entity::content_audit::Entity)
            .filter(content_audit::Column::CreatedAt.is_null())
            .filter(content::Column::FirstAvailableAt.lt(Utc::now()
                - chrono::TimeDelta::try_seconds(10).expect("Couldn't calculate 10 seconds")))
            .join(
                JoinType::InnerJoin,
                entity::content::Relation::ExecutionMetadata.def(),
            )
            .filter(execution_metadata::Column::BlockNumber.gt(MERGE_BLOCK_HEIGHT))
            .order_by_desc(content::Column::FirstAvailableAt)
            .limit(keys_required as u64)
            .all(&conn)
            .await
        {
            Ok(content_key_db_entries) => content_key_db_entries,
            Err(err) => {
                error!(audit.strategy="latest", err=?err, "Could not make audit query");
                continue;
            }
        };
        let item_count = content_key_db_entries.len();
        debug!(
            strategy = "latest",
            item_count, "Adding content keys to the audit queue."
        );
        add_to_queue(
            tx.clone(),
            SelectionStrategy::Latest,
            content_key_db_entries,
        )
        .await;
    }
}

/// Finds and sends audit tasks for [SelectionStrategy::FourFours].
///
/// 1. Get a random block number between 1 and MERGE_BLOCK_HEIGHT.
/// 2. Get the block hash for that block.
/// 3. Send content keys for header, body, receipts.
///
async fn select_fourfours_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
    w3: web3::Web3<web3::transports::Http>,
) -> ! {
    let mut interval = interval(Duration::from_secs(5));

    loop {
        interval.tick().await;
        let block_number = thread_rng().gen_range(1..MERGE_BLOCK_HEIGHT);
        debug!(
            strategy = "4444s",
            "Getting hash for block number {block_number}."
        );
        let block = match w3
            .eth()
            .block(BlockId::Number(BlockNumber::Number(block_number.into())))
            .await
        {
            Ok(Some(block)) => block,
            Ok(None) => {
                error!(strategy = "4444s", block.number=?block_number, "Block not found");
                continue;
            }
            Err(err) => {
                error!(strategy = "4444s", block.number=?block_number, err=?err, "Could not get block");
                continue;
            }
        };

        let block_hash = block.hash.unwrap();

        let timestamp = block.timestamp.as_u64() as i64;
        let block_timestamp = match Utc.timestamp_opt(timestamp, 0) {
            chrono::LocalResult::Single(time) => time,
            _ => {
                error!(block.number=?block_number, block.timestamp=?timestamp, "Could not convert timestamp");
                continue;
            }
        };

        let items_to_audit = store_block_keys(
            block_number,
            block_hash.as_fixed_bytes(),
            block_timestamp,
            &conn,
        )
        .await;
        debug!(
            strategy = "4444s",
            item_count = items_to_audit.len(),
            "Adding content keys to the audit queue."
        );
        add_to_queue(tx.clone(), SelectionStrategy::FourFours, items_to_audit).await;
    }
}

/// Adds Glados database History sub-protocol search results
/// to a channel for auditing against a Portal Node.
async fn add_to_queue(
    tx: mpsc::Sender<AuditTask>,
    strategy: SelectionStrategy,
    items: Vec<content::Model>,
) {
    let capacity = tx.capacity();
    let max_capacity = tx.max_capacity();
    debug!(
        channel.availability = capacity,
        channel.size = max_capacity,
        "Adding items to audit task channel."
    );
    for content_key_model in items {
        // Create key from database bytes.
        let content_key = match HistoryContentKey::try_from(content_key_model.content_key) {
            Ok(key) => key,
            Err(err) => {
                error!(database.id=?content_key_model.id, err=?err, "Could not decode content key from database record");
                continue;
            }
        };
        let task = AuditTask {
            strategy: strategy.clone(),
            content_key,
        };
        if let Err(e) = tx.send(task).await {
            debug!(audit.strategy=?strategy, err=?e, "Could not send key for audit, channel might be full or closed.")
        }
    }
}

/// Finds and sends audit tasks for [Strategy::Random].
///
/// Strategy achieved by:
/// 1. Checking number of keys in DB.
/// 2. Generating random ids.
/// 3. Looking up each one separately, then sending them all in the channel.
///
/// At regular intervals the channel capacity is assessed and new tasks are added to reach capacity.
async fn select_random_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process for 'random' strategy");

    let mut interval = interval(Duration::from_secs(10));
    loop {
        interval.tick().await;

        let num_keys = match content::Entity::find().count(&conn).await {
            // Skip if no keys yet.
            Ok(0) => continue,
            Ok(count) => count as u32,
            Err(err) => {
                error!(audit.strategy="random", err=?err, "Could not make audit query");
                continue;
            }
        };
        let keys_required = tx.capacity();
        if keys_required == 0 {
            continue;
        };
        let mut random_ids: HashSet<u32> = HashSet::new();
        {
            // Thread safe block for the rng, which is not `Send`.
            let mut rng = thread_rng();
            for _ in 0..keys_required {
                random_ids.insert(rng.gen_range(0..num_keys));
            }
        }
        let mut content_key_db_entries: Vec<Model> = vec![];
        for random_id in random_ids {
            match content::Entity::find()
                .filter(content::Column::Id.eq(random_id))
                .all(&conn)
                .await
            {
                Ok(found) => content_key_db_entries.extend(found),
                Err(err) => {
                    error!(audit.strategy="random", err=?err, "Could not make audit query");
                    continue;
                }
            };
        }
        let item_count = content_key_db_entries.len();
        debug!(
            strategy = "random",
            item_count, "Adding content keys to the audit queue."
        );
        add_to_queue(
            tx.clone(),
            SelectionStrategy::Random,
            content_key_db_entries,
        )
        .await;
    }
}

/// Finds and sends audit tasks for [SelectionStrategy::SelectOldestUnaudited].
///
/// Strategy achieved by:
/// 1. Find oldest content
/// 2. Filter for content with no audits
/// 3. As audits are sent, gradually select more recent content
///
/// At regular intervals the channel capacity is assessed and new tasks are added to reach capacity.
async fn select_oldest_unaudited_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) {
    debug!("initializing audit process for 'select oldest unaudited' strategy");
    let mut interval = interval(Duration::from_secs(10));

    // Memory of which audits have been sent using their timestamp.
    let mut timestamp_too_old_threshold: DateTime<Utc> = match Utc.timestamp_millis_opt(0i64) {
        chrono::LocalResult::Single(time) => time,
        _ => {
            error!("Could not convert starting for timestamp");
            return;
        }
    };

    loop {
        interval.tick().await;
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }
        let keys_required = tx.capacity();
        if keys_required == 0 {
            continue;
        };
        let search_result: Vec<(content::Model, Vec<content_audit::Model>)> =
            match content::Entity::find()
                .filter(content::Column::FirstAvailableAt.gt(timestamp_too_old_threshold))
                .filter(content::Column::FirstAvailableAt.lt(Utc::now()
                    - chrono::TimeDelta::try_days(1).expect("Failed to calculate time delta")))
                .order_by_asc(content::Column::FirstAvailableAt)
                .find_with_related(entity::content_audit::Entity)
                .filter(content_audit::Column::CreatedAt.is_null())
                .limit(keys_required as u64)
                .all(&conn)
                .await
            {
                Ok(content_key_db_entries) => content_key_db_entries,
                Err(err) => {
                    error!(audit.strategy="latest", err=?err, "Could not make audit query");
                    continue;
                }
            };
        let content_key_db_entries: Vec<content::Model> = search_result
            .into_iter()
            .map(|(content, _audits)| {
                if content.first_available_at > timestamp_too_old_threshold {
                    timestamp_too_old_threshold = content.first_available_at
                }
                content
            })
            .collect();
        let item_count = content_key_db_entries.len();
        debug!(
            strategy = "select oldest unaudited",
            item_count, "Adding content keys to the audit queue."
        );
        add_to_queue(
            tx.clone(),
            SelectionStrategy::SelectOldestUnaudited,
            content_key_db_entries,
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;
    use enr::NodeId;
    use entity::{
        client_info,
        content::{self, SubProtocol},
        content_audit::{self, AuditResult},
        node,
    };
    use ethportal_api::{BlockHeaderKey, HistoryContentKey, OverlayContentKey};
    use migration::{DbErr, Migrator, MigratorTrait};
    use sea_orm::{
        ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Database, DbConn, EntityTrait,
        QueryFilter, Set,
    };
    use tokio::sync::mpsc::channel;

    use super::*;

    /// Creates a new in-memory SQLite database for a unit test.
    #[allow(dead_code)]
    async fn setup_database() -> Result<DbConn, DbErr> {
        let conn: DbConn = Database::connect("sqlite::memory:").await?;
        Migrator::up(&conn, None).await.unwrap();
        Ok(conn)
    }

    /// Creates a database and fills it with entries for testing with
    /// different strategies.
    ///
    /// Populated in three sections (old, middle, new) of ~15, given
    /// that audit strategies are performed on blocks of KEYS_PER_PERIOD (=10).
    ///
    /// Properties:
    /// - 45 total
    ///     - 1 audited, result fail
    ///     - 2-15 "old" never audited
    ///     - 16-30 "middle" audited
    ///         - Odd = audit result is fail
    ///         - Even = audit result is pass
    ///     - 31-45 "new" never audited
    ///
    /// Content are all block header (selector = 0), with every byte in the block hash set to the
    /// of the test audit entry. Hence keys are:
    /// - [0, 1, 1, ..., 1]
    /// - [0, 2, 2, ..., 2]
    /// - ...
    /// - [0, 45, 45, ..., 45]
    async fn get_populated_test_audit_db() -> Result<DbConn, DbErr> {
        let conn = setup_database().await?;
        for num in 1..=45 {
            let block_hash = [num; 32];
            let content_key =
                HistoryContentKey::BlockHeaderWithProof(BlockHeaderKey { block_hash });
            // Content table test data initialization
            // Check whether row should be new or old data
            let available_at = match (2..=15).contains(&num) {
                true => Utc::now() - chrono::TimeDelta::try_days(1).unwrap(),
                false => Utc::now() - chrono::TimeDelta::try_minutes(10).unwrap(),
            };
            let block_number = match (2..=15).contains(&num) {
                true => MERGE_BLOCK_HEIGHT - num as i32,
                false => MERGE_BLOCK_HEIGHT + num as i32,
            };
            let content_key_active_model = content::ActiveModel {
                id: NotSet,
                content_id: Set(content_key.content_id().to_vec()),
                content_key: Set(content_key.to_bytes()),
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
            // audit table.
            if (16..=30).contains(&num) || num == 1 {
                let result = match num % 2 == 0 {
                    true => AuditResult::Success,
                    false => AuditResult::Failure,
                };
                let content_audit_active_model = content_audit::ActiveModel {
                    id: NotSet,
                    content_key: Set(content_key_model.id),
                    created_at: Set(Utc::now()),
                    strategy_used: Set(Some(SelectionStrategy::Random)),
                    result: Set(result),
                    trace: Set("".to_owned()),
                    client_info: Set(Some(client_info_model.id)),
                    node: Set(Some(node.id)),
                };
                content_audit_active_model.insert(&conn).await?;
            }
        }
        let test_keys = content::Entity::find().all(&conn).await?;
        assert_eq!(test_keys.len(), 45);

        let item_index_18_audit = content_audit::Entity::find()
            .filter(content_audit::Column::ContentKey.eq(18))
            .one(&conn)
            .await?
            .unwrap();
        assert_eq!(item_index_18_audit.result, AuditResult::Success);
        Ok(conn)
    }

    /// Tests that the `SelectionStrategy::Latest` selects the correct values
    /// from the test database.
    #[tokio::test]
    async fn test_latest_strategy() {
        // Orchestration
        let conn = get_populated_test_audit_db().await.unwrap();
        const CHANNEL_SIZE: usize = 20;
        let (tx, mut rx) = channel::<AuditTask>(CHANNEL_SIZE);
        // Start strategy
        tokio::spawn(select_latest_content_for_audit(tx.clone(), conn.clone()));
        let mut checked_ids: HashSet<i32> = HashSet::new();
        // There are 15 correct values: [31, 32, ... 45], after which the queue should be empty
        let expected_key_ids: Vec<i32> = (31..=45).collect();
        // Await strategy results
        while let Some(task) = rx.recv().await {
            let key_model = content::Entity::find()
                .filter(content::Column::ContentKey.eq(task.content_key.to_bytes()))
                .one(&conn)
                .await
                .unwrap()
                .unwrap();
            // Check that strategy only yields expected keys.
            assert!(expected_key_ids.contains(&key_model.id));
            checked_ids.insert(key_model.id);
            if checked_ids.len() == expected_key_ids.len() {
                break;
            }
        }

        // Make sure that there are no further keys (latest should filter out the premerge keys).
        assert!(matches!(
            rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));

        // Make sure no key was audited twice by pushing to a hashmap and checking it's length.
        assert_eq!(checked_ids.len(), expected_key_ids.len());
    }

    /// Tests that the `SelectionStrategy::SelectOldestUnaudited` selects the correct values
    /// from the test database.
    #[tokio::test]
    async fn test_select_oldest_unaudited_strategy() {
        // Orchestration
        let conn = get_populated_test_audit_db().await.unwrap();
        const CHANNEL_SIZE: usize = 10;
        let (tx, mut rx) = channel::<AuditTask>(CHANNEL_SIZE);
        // Start strategy
        tokio::spawn(select_oldest_unaudited_content_for_audit(
            tx.clone(),
            conn.clone(),
        ));
        let mut checked_ids: HashSet<i32> = HashSet::new();
        // There are 10 correct values: [2, ..., 11]
        let expected_key_ids: Vec<i32> = (2..=11).collect();
        // Await strategy results
        while let Some(task) = rx.recv().await {
            let key_model = content::Entity::find()
                .filter(content::Column::ContentKey.eq(task.content_key.to_bytes()))
                .one(&conn)
                .await
                .unwrap()
                .unwrap();
            // Check that strategy only yields expected keys.
            assert!(expected_key_ids.contains(&key_model.id));
            checked_ids.insert(key_model.id);
            if checked_ids.len() == CHANNEL_SIZE {
                break;
            }
        }
        // Make sure no key was audited twice by pushing to a hashmap and checking it's length.
        assert_eq!(checked_ids.len(), CHANNEL_SIZE);
    }

    /// Tests that the `SelectionStrategy::Random` selects the correct values
    /// from the test database.
    #[tokio::test]
    async fn test_random_strategy() {
        // Orchestration
        let conn = get_populated_test_audit_db().await.unwrap();
        const CHANNEL_SIZE: usize = 10;
        let (tx, mut rx) = channel::<AuditTask>(CHANNEL_SIZE);
        // Start strategy
        tokio::spawn(select_latest_content_for_audit(tx.clone(), conn.clone()));
        let mut checked_ids: HashSet<i32> = HashSet::new();
        // There are 45 possible correct values: [1, 2, ... 45]
        let expected_key_ids: Vec<i32> = (1..=45).collect();
        // Await strategy results
        while let Some(task) = rx.recv().await {
            let key_model = content::Entity::find()
                .filter(content::Column::ContentKey.eq(task.content_key.to_bytes()))
                .one(&conn)
                .await
                .unwrap()
                .unwrap();
            // Check that strategy only yields expected keys.
            assert!(expected_key_ids.contains(&key_model.id));
            checked_ids.insert(key_model.id);
            if checked_ids.len() == CHANNEL_SIZE {
                break;
            }
        }
        // Make sure no key was audited twice by pushing to a hashmap and checking it's length.
        assert_eq!(checked_ids.len(), CHANNEL_SIZE);
    }
}
