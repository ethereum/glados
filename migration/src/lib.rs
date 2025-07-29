use sea_orm::entity::prelude::*;

pub use sea_orm_migration::{MigrationTrait, MigratorTrait};

mod m20250722_193400_create_node;
mod m20250722_202331_create_node_enr;
mod m20250722_205252_create_client;
mod m20250722_211026_create_census;
mod m20250722_212338_create_census_node;
mod m20250722_223408_create_content;
mod m20250723_083350_create_audit;
mod m20250723_085824_create_audit_latest;
mod m20250723_092658_create_audit_transfer_failure;
mod m20250723_093821_create_audit_stats;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250722_193400_create_node::Migration),
            Box::new(m20250722_202331_create_node_enr::Migration),
            Box::new(m20250722_205252_create_client::Migration),
            Box::new(m20250722_211026_create_census::Migration),
            Box::new(m20250722_212338_create_census_node::Migration),
            Box::new(m20250722_223408_create_content::Migration),
            Box::new(m20250723_083350_create_audit::Migration),
            Box::new(m20250723_085824_create_audit_latest::Migration),
            Box::new(m20250723_092658_create_audit_transfer_failure::Migration),
            Box::new(m20250723_093821_create_audit_stats::Migration),
        ]
    }
}

#[cfg(test)]
mod tests;
