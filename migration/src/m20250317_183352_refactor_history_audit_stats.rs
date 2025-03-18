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
                    .rename_column(
                        Alias::new("success_rate_all"),
                        Alias::new("success_rate_history_all"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_latest"),
                        Alias::new("success_rate_history_latest"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_random"),
                        Alias::new("success_rate_history_random"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_oldest"),
                        Alias::new("success_rate_history_oldest"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_all_headers"),
                        Alias::new("success_rate_history_all_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_all_bodies"),
                        Alias::new("success_rate_history_all_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_all_receipts"),
                        Alias::new("success_rate_history_all_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_latest_headers"),
                        Alias::new("success_rate_history_latest_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_latest_bodies"),
                        Alias::new("success_rate_history_latest_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_latest_receipts"),
                        Alias::new("success_rate_history_latest_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_random_headers"),
                        Alias::new("success_rate_history_random_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_random_bodies"),
                        Alias::new("success_rate_history_random_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_random_receipts"),
                        Alias::new("success_rate_history_random_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_four_fours"),
                        Alias::new("success_rate_history_four_fours"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_four_fours_headers"),
                        Alias::new("success_rate_history_four_fours_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_four_fours_bodies"),
                        Alias::new("success_rate_history_four_fours_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_four_fours_receipts"),
                        Alias::new("success_rate_history_four_fours_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(Table::alter().table(AuditStats::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_all"),
                        Alias::new("success_rate_all"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_latest"),
                        Alias::new("success_rate_latest"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_random"),
                        Alias::new("success_rate_random"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_oldest"),
                        Alias::new("success_rate_oldest"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_all_headers"),
                        Alias::new("success_rate_all_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_all_bodies"),
                        Alias::new("success_rate_all_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_all_receipts"),
                        Alias::new("success_rate_all_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_latest_headers"),
                        Alias::new("success_rate_latest_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_latest_bodies"),
                        Alias::new("success_rate_latest_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_latest_receipts"),
                        Alias::new("success_rate_latest_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_random_headers"),
                        Alias::new("success_rate_random_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_random_bodies"),
                        Alias::new("success_rate_random_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_random_receipts"),
                        Alias::new("success_rate_random_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_four_fours"),
                        Alias::new("success_rate_four_fours"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_four_fours_headers"),
                        Alias::new("success_rate_four_fours_headers"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_four_fours_bodies"),
                        Alias::new("success_rate_four_fours_bodies"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuditStats::Table)
                    .rename_column(
                        Alias::new("success_rate_history_four_fours_receipts"),
                        Alias::new("success_rate_four_fours_receipts"),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum AuditStats {
    Table,
}
