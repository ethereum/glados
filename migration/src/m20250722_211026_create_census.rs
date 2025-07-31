use sea_orm_migration::{prelude::*, schema::*};

const IDX_SUBPROTOCOL_STARTED_AT: &str = "IDX-census-subprotocol-started_at";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Census::Table)
                    .if_not_exists()
                    .col(pk_auto(Census::Id))
                    .col(unsigned(Census::Subprotocol))
                    .col(timestamp_with_time_zone_uniq(Census::StartedAt))
                    .col(big_unsigned(Census::Duration))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Census::Table)
                    .name(IDX_SUBPROTOCOL_STARTED_AT)
                    .col(Census::Subprotocol)
                    .col(Census::StartedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Census::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Census {
    Table,
    Id,
    StartedAt,
    Duration,
    Subprotocol,
}
