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
                    .drop_column(AuditStats::SuccessRateBeaconAll)
                    .drop_column(AuditStats::SuccessRateBeaconLatest)
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
                        ColumnDef::new(AuditStats::SuccessRateBeaconAll)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRateBeaconLatest)
                            .float()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum AuditStats {
    Table,
    SuccessRateBeaconAll,
    SuccessRateBeaconLatest,
}
