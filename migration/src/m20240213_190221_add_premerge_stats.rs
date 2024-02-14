use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRatePremerge)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRatePremergeHeaders)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRatePremergeBodies)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRatePremergeReceipts)
                            .float()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .drop_column(AuditStats::SuccessRatePremerge)
                    .drop_column(AuditStats::SuccessRatePremergeHeaders)
                    .drop_column(AuditStats::SuccessRatePremergeBodies)
                    .drop_column(AuditStats::SuccessRatePremergeReceipts)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum AuditStats {
    Table,
    SuccessRatePremerge,
    SuccessRatePremergeHeaders,
    SuccessRatePremergeBodies,
    SuccessRatePremergeReceipts,
}
