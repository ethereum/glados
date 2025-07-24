use sea_orm_migration::{prelude::*, schema::*};

const IDX_TIMESTAMP: &str = "IDX-audit_stats-timestamp";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditStats::Table)
                    .if_not_exists()
                    .col(pk_auto(AuditStats::Id))
                    .col(timestamp_with_time_zone(AuditStats::Timestamp))
                    .col(float(AuditStats::SuccessRateHistoryAll))
                    .col(float(AuditStats::SuccessRateHistorySync))
                    .col(float(AuditStats::SuccessRateHistoryRandom))
                    .col(float(AuditStats::SuccessRateHistoryAllBodies))
                    .col(float(AuditStats::SuccessRateHistorySyncBodies))
                    .col(float(AuditStats::SuccessRateHistoryRandomBodies))
                    .col(float(AuditStats::SuccessRateHistoryAllReceipts))
                    .col(float(AuditStats::SuccessRateHistorySyncReceipts))
                    .col(float(AuditStats::SuccessRateHistoryRandomReceipts))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(AuditStats::Table)
                    .name(IDX_TIMESTAMP)
                    .col(AuditStats::Timestamp)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditStats::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditStats {
    Table,
    Id,
    Timestamp,
    SuccessRateHistoryAll,
    SuccessRateHistorySync,
    SuccessRateHistoryRandom,
    SuccessRateHistoryAllBodies,
    SuccessRateHistorySyncBodies,
    SuccessRateHistoryRandomBodies,
    SuccessRateHistoryAllReceipts,
    SuccessRateHistorySyncReceipts,
    SuccessRateHistoryRandomReceipts,
}
