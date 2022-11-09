use std::path::PathBuf;

use sea_orm::DatabaseConnection;

pub struct State {
    pub ipc_path: PathBuf,
    pub database_connection: DatabaseConnection,
}
