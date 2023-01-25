pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20221114_143914_create_content_id_key_and_audit;
mod m20230125_205211_create_execution_header_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20221114_143914_create_content_id_key_and_audit::Migration),
            Box::new(m20230125_205211_create_execution_header_table::Migration),
        ]
    }
}
