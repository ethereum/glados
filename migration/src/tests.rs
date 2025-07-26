use pgtemp::PgTempDB;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

use crate::Migrator;

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
