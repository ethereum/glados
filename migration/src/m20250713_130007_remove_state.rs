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
                    .drop_column(AuditStats::SuccessRateStateAll)
                    .drop_column(AuditStats::SuccessRateStateLatest)
                    .drop_column(AuditStats::SuccessRateStateStateRoots)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(StateRoots::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StateRoots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StateRoots::BlockNumber)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(StateRoots::StateRoot).binary().not_null())
                    .col(
                        ColumnDef::new(StateRoots::FirstAvailableAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRateStateAll)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRateStateLatest)
                            .float()
                            .default(0.0),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditStats::SuccessRateStateStateRoots)
                            .float()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum StateRoots {
    Table,
    BlockNumber,
    StateRoot,
    FirstAvailableAt,
}

#[derive(Iden)]
enum AuditStats {
    Table,
    SuccessRateStateAll,
    SuccessRateStateLatest,
    SuccessRateStateStateRoots,
}
