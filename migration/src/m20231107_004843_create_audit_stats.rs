use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .create_table(
                Table::create()
                    .table(AuditStats::Table)
                    .col(
                        ColumnDef::new(AuditStats::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryAll)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistorySync)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryRandom)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryAllBodies)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryAllReceipts)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistorySyncBodies)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistorySyncReceipts)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryRandomBodies)
                            .float()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditStats::SuccessRateHistoryRandomReceipts)
                            .float()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await;
        manager
            .create_index(
                Index::create()
                    .name("idx_auditstats-time")
                    .table(AuditStats::Table)
                    .col(AuditStats::Timestamp)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditStats::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum AuditStats {
    Table,
    Id,
    Timestamp,
    SuccessRateHistoryAll,
    SuccessRateHistorySync,
    SuccessRateHistoryRandom,
    SuccessRateHistoryAllBodies,
    SuccessRateHistoryAllReceipts,
    SuccessRateHistorySyncBodies,
    SuccessRateHistorySyncReceipts,
    SuccessRateHistoryRandomBodies,
    SuccessRateHistoryRandomReceipts,
}
