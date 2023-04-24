use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KeyValue::Table).to_owned())
            .await?;

        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .drop_table(Table::drop().table(Record::Table).to_owned())
                .await?;
            manager
                .drop_table(Table::drop().table(Node::Table).to_owned())
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Node {
    Table,
}

#[derive(Iden)]
enum Record {
    Table,
}

#[derive(Iden)]
enum KeyValue {
    Table,
}
