pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20221114_143914_create_content_id_key_and_audit;
mod m20230310_134806_add_audit_strategy_column;
mod m20230404_163459_drop_record_tables;
mod m20230404_163706_create_new_record_tables;
mod m20230421_120832_add_audit_trace;
mod m20230501_153121_add_node_id_high_bits_column_to_node_id;
mod m20230503_131114_add_client_info;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20221114_143914_create_content_id_key_and_audit::Migration),
            Box::new(m20230310_134806_add_audit_strategy_column::Migration),
            Box::new(m20230404_163459_drop_record_tables::Migration),
            Box::new(m20230404_163706_create_new_record_tables::Migration),
            Box::new(m20230421_120832_add_audit_trace::Migration),
            Box::new(m20230501_153121_add_node_id_high_bits_column_to_node_id::Migration),
            Box::new(m20230503_131114_add_client_info::Migration),
        ]
    }
}
