use anyhow::{bail, Result};
use futures::TryStreamExt;
use sea_orm::DatabaseConnection;
pub use sea_orm_migration::prelude::*;
use tracing::{error, info, warn};

mod m20230511_104804_create_node;
mod m20230511_104811_create_record;
mod m20230511_104814_create_content;
mod m20230511_104823_create_client_info;
mod m20230511_104830_create_content_audit;
mod m20230511_104838_create_execution_metadata;
mod m20230511_104937_create_key_value;
mod m20230599_999999_create_census_tables;
mod m20231107_004843_create_audit_stats;
mod m20240213_190221_add_fourfours_stats;
mod m20240322_205213_add_content_audit_index;
mod m20240515_064320_state_roots;
mod m20240720_111606_create_census_index;
mod m20240814_121507_census_subnetwork;
mod m20240919_121611_census_subnetwork_index;
mod m20241010_151313_audit_stats_performance;
mod m20241206_154045_add_beacon_and_state_audit_stats;
mod m20250130_042751_create_audit_internal_failures;
mod m20250311_115816_create_blocks;
mod m20250311_121724_create_audit_result_latest;
mod m20250314_144135_add_history_headers_by_number_audit_stats;
mod m20250317_183352_refactor_history_audit_stats;
mod m20250404_202958_internal_failures_replace_node_with_record;
mod m20250404_220628_add_client_info_to_census_node;
mod m20250713_130007_remove_state;
mod m20250713_191500_remove_beacon_audit_stats;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230511_104804_create_node::Migration),
            Box::new(m20230511_104811_create_record::Migration),
            Box::new(m20230511_104814_create_content::Migration),
            Box::new(m20230511_104823_create_client_info::Migration),
            Box::new(m20230511_104830_create_content_audit::Migration),
            Box::new(m20230511_104838_create_execution_metadata::Migration),
            Box::new(m20230511_104937_create_key_value::Migration),
            Box::new(m20230599_999999_create_census_tables::Migration),
            Box::new(m20231107_004843_create_audit_stats::Migration),
            Box::new(m20240213_190221_add_fourfours_stats::Migration),
            Box::new(m20240322_205213_add_content_audit_index::Migration),
            Box::new(m20240515_064320_state_roots::Migration),
            Box::new(m20240720_111606_create_census_index::Migration),
            Box::new(m20240814_121507_census_subnetwork::Migration),
            Box::new(m20240919_121611_census_subnetwork_index::Migration),
            Box::new(m20241010_151313_audit_stats_performance::Migration),
            Box::new(m20241206_154045_add_beacon_and_state_audit_stats::Migration),
            Box::new(m20250130_042751_create_audit_internal_failures::Migration),
            Box::new(m20250311_115816_create_blocks::Migration),
            Box::new(m20250311_121724_create_audit_result_latest::Migration),
            Box::new(m20250314_144135_add_history_headers_by_number_audit_stats::Migration),
            Box::new(m20250317_183352_refactor_history_audit_stats::Migration),
            Box::new(m20250404_202958_internal_failures_replace_node_with_record::Migration),
            Box::new(m20250404_220628_add_client_info_to_census_node::Migration),
            Box::new(m20250713_130007_remove_state::Migration),
            Box::new(m20250713_191500_remove_beacon_audit_stats::Migration),
        ]
    }
}

pub struct SeedConfig {
    migration_name: String,
    url: String,
    table: String,
}
pub trait SeedTrait {
    fn seeds() -> Vec<SeedConfig>;

    fn seed(
        conn: &DatabaseConnection,
        seed_config: SeedConfig,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn seed_new_migrations(
        conn: &DatabaseConnection,
        old_migrations: Vec<String>,
        skip_seeding: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn seed_by_table(
        conn: DatabaseConnection,
        table: String,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

impl SeedTrait for Migrator {
    fn seeds() -> Vec<SeedConfig> {
        vec![SeedConfig {
            migration_name: "m20250311_115816_create_blocks".into(),
            // TODO(ktlxv): Update when file is uploaded
            url: "https://github.com/ethereum/glados/raw/refs/heads/master/datasets/mainnet/blocks/premerge.csv".into(),
            table: "block".into(),
        }]
    }

    async fn seed(conn: &DatabaseConnection, seed_config: SeedConfig) -> Result<()> {
        info!(
            "Seeding '{}' table (this might take several minutes)",
            seed_config.table
        );

        // Get low level Postgress connection
        let db = conn.get_postgres_connection_pool();

        // Use streams to avoid saving a large CSV
        let res = match reqwest::get(&seed_config.url).await {
            Ok(res) => res,
            Err(err) => {
                error!("Error while fetching seed data for table '{}' from {}. Please seed table manually with seed subcommand", seed_config.table, seed_config.url);
                bail!(err)
            }
        };

        let stream = res.bytes_stream().map_err(std::io::Error::other);
        let tokio_stream = tokio_util::io::StreamReader::new(stream);

        let mut writer = db
            .copy_in_raw(&(format!("COPY {} FROM stdin csv header", seed_config.table)))
            .await
            .unwrap();
        writer.read_from(tokio_stream).await?;

        if let Err(err) = writer.finish().await {
            error!("Error seeding {}, please seed manually", seed_config.table);
            bail!(err)
        }

        info!("Table {} seeded successfuly", seed_config.table);

        Ok(())
    }

    async fn seed_new_migrations(
        conn: &DatabaseConnection,
        old_migrations: Vec<String>,
        skip_seeding: bool,
    ) -> Result<()> {
        for s in Migrator::seeds() {
            if !old_migrations.contains(&s.migration_name) {
                match skip_seeding {
                    true => warn!("Skipped seeding for migration: {}", &s.migration_name),
                    false => Migrator::seed(conn, s).await?,
                }
            }
        }
        Ok(())
    }
    async fn seed_by_table(conn: DatabaseConnection, table: String) -> Result<()> {
        if let Some(seed_config) = Migrator::seeds().into_iter().find(|s| s.table == *table) {
            Migrator::seed(&conn, seed_config).await?;
            Ok(())
        } else {
            bail!("Seed for table {} not found.", table);
        }
    }
}

#[cfg(test)]
mod tests {

    use pgtemp::PgTempDB;
    use sea_orm::Database;

    use super::*;

    async fn setup_test_database() -> Result<(PgTempDB, DatabaseConnection), DbErr> {
        let temp_db = PgTempDB::async_new().await;
        let db = Database::connect(temp_db.connection_uri()).await?;
        Ok((temp_db, db))
    }

    #[async_std::test]
    async fn fresh() -> anyhow::Result<()> {
        let (_temp_db, db) = setup_test_database().await?;
        Migrator::fresh(&db).await?;
        Ok(())
    }

    #[async_std::test]
    async fn up() -> anyhow::Result<()> {
        let (_temp_db, db) = setup_test_database().await?;
        Migrator::up(&db, None).await?;
        Ok(())
    }

    #[async_std::test]
    async fn reset() -> anyhow::Result<()> {
        let (_temp_db, db) = setup_test_database().await?;
        Migrator::fresh(&db).await?;
        Migrator::reset(&db).await?;
        Ok(())
    }

    #[async_std::test]
    async fn refresh() -> anyhow::Result<()> {
        let (_temp_db, db) = setup_test_database().await?;
        Migrator::fresh(&db).await?;
        Migrator::refresh(&db).await?;
        Ok(())
    }
}
