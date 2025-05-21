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
                    .drop_column(AuditStats::SuccessRateStateLatest)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRateStateLatest)
                            .float()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum AuditStats {
    Table,
    SuccessRateStateLatest,
}
