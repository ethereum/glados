pub use sea_orm_migration::prelude::*;

mod m20230508_111707_create_census_tables;
mod m20230511_104804_create_node;
mod m20230511_104811_create_record;
mod m20230511_104814_create_content;
mod m20230511_104823_create_client_info;
mod m20230511_104830_create_content_audit;
mod m20230511_104838_create_execution_metadata;
mod m20230511_104937_create_key_value;

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
            Box::new(m20230508_111707_create_census_tables::Migration),
        ]
    }
}
