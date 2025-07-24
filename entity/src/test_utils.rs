use migration::{Migrator, MigratorTrait};
use pgtemp::PgTempDB;
use sea_orm::{Database, DatabaseConnection, DbErr};

// Temporary Postgres db will be deleted once PgTempDB goes out of scope, so keep it in scope.
pub async fn setup_database() -> Result<(DatabaseConnection, PgTempDB), DbErr> {
    let db = PgTempDB::async_new().await;
    let conn = Database::connect(db.connection_uri()).await?;
    Migrator::up(&conn, None).await.unwrap();

    Ok((conn, db))
}
