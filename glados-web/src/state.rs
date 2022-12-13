use sea_orm::DatabaseConnection;

pub struct State {
    pub database_connection: DatabaseConnection,
}
