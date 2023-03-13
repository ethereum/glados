use std::collections::HashSet;

use ethportal_api::types::content_key::HistoryContentKey;
use rand::{thread_rng, Rng};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use tokio::{
    sync::mpsc,
    time::{interval, Duration},
};
use tracing::{debug, error, warn};

use entity::{
    content::{self, Model},
    content_audit::{self, SelectionStrategy},
};

use crate::AuditTask;

/// Interval between audit selections for a particular strategy.
const AUDIT_SELECTION_PERIOD_SECONDS: u64 = 120;

/// Number of content keys to audit per
/// [`AUDIT_SELECTION_PERIOD_SECONDS`].
const KEYS_PER_PERIOD: u64 = 10;

pub async fn start_audit_selection_task(
    strategy: SelectionStrategy,
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) {
    match strategy {
        SelectionStrategy::Latest => select_latest_content_for_audit(tx, conn).await,
        SelectionStrategy::Random => select_random_content_for_audit(tx, conn).await,
        SelectionStrategy::Failed => warn!("Need to implement SelectionStrategy::Failed"),
        SelectionStrategy::OldestMissing => {
            warn!("Need to implement SelectionStrategy::OldestMissing")
        }
    }
}

/// Finds and sends audit tasks for [Strategy::Latest].
///
/// Strategy achieved by:
/// 1. Left joining contentkey table to the contentaudit table to find audits per key.
/// 2. Filter for null audits (Exclude any item with an existing audit).
/// 3. Sort ascending to have most recently added content keys first.
async fn select_latest_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process for 'latest' strategy");

    let mut interval = interval(Duration::from_secs(AUDIT_SELECTION_PERIOD_SECONDS));
    loop {
        interval.tick().await;
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }
        let content_key_db_entries = match content::Entity::find()
            .left_join(entity::content_audit::Entity)
            .filter(content_audit::Column::CreatedAt.is_null())
            .order_by_desc(content::Column::FirstAvailableAt)
            .limit(KEYS_PER_PERIOD)
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

/// Adds Glados database History sub-protocol search results
/// to a channel for auditing against a Portal Node.
async fn add_to_queue(
    tx: mpsc::Sender<AuditTask>,
    strategy: SelectionStrategy,
    items: Vec<content::Model>,
) {
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
async fn select_random_content_for_audit(
    tx: mpsc::Sender<AuditTask>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process for 'random' strategy");

    let mut interval = interval(Duration::from_secs(AUDIT_SELECTION_PERIOD_SECONDS));
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
        let mut random_ids: HashSet<u32> = HashSet::new();
        {
            // Thread safe block for the rng, which is not `Send`.
            let mut rng = thread_rng();
            for _ in 0..KEYS_PER_PERIOD {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;
    use entity::{
        content::{self, SubProtocol},
        content_audit::{self, AuditResult},
    };
    use ethportal_api::{
        types::content_key::{BlockHeaderKey, OverlayContentKey},
        HistoryContentKey,
    };
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
    /// Populated in three sections (old, middle, new) of 15, given
    /// that audit strategies are performed on blocks of KEYS_PER_PERIOD (=10).
    ///
    /// Properties:
    /// - 45 total
    ///     - 1-15 "old" never audited
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
            // content table
            let content_key_active_model = content::ActiveModel {
                id: NotSet,
                content_id: Set(content_key.content_id().to_vec()),
                content_key: Set(content_key.to_bytes()),
                first_available_at: Set(Utc::now().into()),
                protocol_id: Set(SubProtocol::History),
            };
            let content_key_model = content_key_active_model.insert(&conn).await?;
            // audit table
            if (16..=30).contains(&num) {
                let result = match num % 2 == 0 {
                    true => AuditResult::Success,
                    false => AuditResult::Failure,
                };
                let content_audit_active_model = content_audit::ActiveModel {
                    id: NotSet,
                    content_key: Set(content_key_model.id),
                    created_at: Set(Utc::now().into()),
                    strategy_used: Set(Some(SelectionStrategy::Random)),
                    result: Set(result),
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
        let (tx, mut rx) = channel::<AuditTask>(100);
        // Start strategy
        tokio::spawn(select_latest_content_for_audit(tx.clone(), conn.clone()));
        let mut checked_ids: HashSet<i32> = HashSet::new();
        // There are 10 correct values: [36, 37, ... 45]
        let expected_key_ids: Vec<i32> = (36..=45).collect();
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
            if checked_ids.len() == KEYS_PER_PERIOD as usize {
                break;
            }
        }
        // Make sure no key was audited twice by pushing to a hashmap and checking it's length.
        assert_eq!(checked_ids.len(), KEYS_PER_PERIOD as usize);
    }

    /// Tests that the `SelectionStrategy::Random` selects the correct values
    /// from the test database.
    #[tokio::test]
    async fn test_random_strategy() {
        // Orchestration
        let conn = get_populated_test_audit_db().await.unwrap();
        let (tx, mut rx) = channel::<AuditTask>(100);
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
            if checked_ids.len() == KEYS_PER_PERIOD as usize {
                break;
            }
        }
        // Make sure no key was audited twice by pushing to a hashmap and checking it's length.
        assert_eq!(checked_ids.len(), KEYS_PER_PERIOD as usize);
    }
}
