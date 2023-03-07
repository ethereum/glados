use ethportal_api::types::content_key::HistoryContentKey;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use tokio::{
    sync::mpsc,
    time::{interval, Duration},
};
use tracing::{debug, error, warn};

use entity::{
    contentaudit,
    contentkey::{self, Model},
};

const AUDIT_SELECTION_PERIOD_SECONDS: u64 = 120;

/// Number of content keys to audit per
/// [`AUDIT_SELECTION_PERIOD_SECONDS`].
const KEYS_PER_PERIOD: u64 = 10;

/// A strategy is responsible for generating audit tasks.
///
/// An audit task is a content key from the the glados database that
/// is expected to be in a portal node.
pub enum SelectionStrategy {
    /// Content that is:
    /// 1. Not yet audited
    /// 2. Sorted by date entered into glados database (newest first).
    Latest,
    /// Randomly selected content.
    Random,
    /// Content that looks for failed audits and checks whether the data is still missing.
    Failed,
}

impl SelectionStrategy {
    pub async fn start_audit_selection_task(
        self,
        tx: mpsc::Sender<HistoryContentKey>,
        conn: DatabaseConnection,
    ) {
        match self {
            SelectionStrategy::Latest => select_latest_content_for_audit(tx, conn).await,
            SelectionStrategy::Random => {
                warn!("Audit strategy 'Strategy::Random' not yet implemented.")
            }
            SelectionStrategy::Failed => {
                warn!("Audit strategy 'Strategy::Failed' not yet implemented.")
            }
        }
    }
}

/// Finds and sends audit tasks for the 'latest' strategy.
///
/// Strategy achieved by:
/// 1. Left joining contentkey table to the contentaudit table to find audits per key.
/// 2. Filter for null audits (Exclude any item with an existing audit).
/// 3. Sort ascending to have most recently added content keys first.
async fn select_latest_content_for_audit(
    tx: mpsc::Sender<HistoryContentKey>,
    conn: DatabaseConnection,
) -> ! {
    debug!("initializing audit process");

    let mut interval = interval(Duration::from_secs(AUDIT_SELECTION_PERIOD_SECONDS));
    loop {
        interval.tick().await;
        if tx.is_closed() {
            error!("Channel is closed.");
            panic!();
        }
        let content_key_db_entries = match contentkey::Entity::find()
            .left_join(entity::contentaudit::Entity)
            .filter(contentaudit::Column::CreatedAt.is_null())
            .order_by_desc(contentkey::Column::CreatedAt)
            .limit(KEYS_PER_PERIOD)
            .all(&conn)
            .await
        {
            Ok(content_key_db_entries) => content_key_db_entries,
            Err(err) => {
                error!("DB Error looking up content key: {err}");
                continue;
            }
        };
        let item_count = content_key_db_entries.len();
        debug!(
            strategy = "latest",
            item_count, "Adding content keys to the audit queue."
        );
        add_to_queue(tx.clone(), content_key_db_entries).await;
    }
}

/// Adds Glados database History sub-protocol search results
/// to a channel for auditing against a Portal Node.
async fn add_to_queue(tx: mpsc::Sender<HistoryContentKey>, items: Vec<Model>) {
    for content_key_model in items {
        let key = HistoryContentKey::try_from(content_key_model.content_key).unwrap();
        if let Err(e) = tx.send(key).await {
            debug!(err=?e, "Could not send key for audit, channel might be full or closed.")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;
    use entity::{contentaudit, contentid, contentkey, AuditResult};
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

    use super::{select_latest_content_for_audit, KEYS_PER_PERIOD};

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
            // content_id table
            let content_id_active_model = contentid::ActiveModel {
                id: NotSet,
                content_id: Set(content_key.content_id().to_vec()),
            };
            let content_id_model = content_id_active_model.insert(&conn).await?;
            // content_key table
            let content_key_active_model = contentkey::ActiveModel {
                id: NotSet,
                content_id: Set(content_id_model.id),
                content_key: Set(content_key.to_bytes()),
                created_at: Set(Utc::now()),
            };
            let content_key_model = content_key_active_model.insert(&conn).await?;
            // audit table
            if (16..=30).contains(&num) {
                let result = match num % 2 == 0 {
                    true => AuditResult::Success,
                    false => AuditResult::Failure,
                };
                let content_audit_active_model = contentaudit::ActiveModel {
                    id: NotSet,
                    content_key: Set(content_key_model.id),
                    created_at: Set(Utc::now()),
                    result: Set(result),
                };
                content_audit_active_model.insert(&conn).await?;
            }
        }
        let test_keys = contentkey::Entity::find().all(&conn).await?;
        assert_eq!(test_keys.len(), 45);

        let item_index_18_audit = contentaudit::Entity::find()
            .filter(contentaudit::Column::ContentKey.eq(18))
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
        let (tx, mut rx) = channel::<HistoryContentKey>(100);
        // Start strategy
        tokio::spawn(select_latest_content_for_audit(tx.clone(), conn.clone()));
        let mut checked_ids: HashSet<i32> = HashSet::new();
        // There are 10 correct values: [36, 37, ... 45]
        let expected_key_ids: Vec<i32> = (36..=45).collect();
        // Await strategy results
        while let Some(key) = rx.recv().await {
            let key_model = contentkey::Entity::find()
                .filter(contentkey::Column::ContentKey.eq(key.to_bytes()))
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
