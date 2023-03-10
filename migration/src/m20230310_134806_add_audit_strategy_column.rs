use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add a column to the contentaudit table for the strategy used in the audit.
        // Values pre-migration will be null.
        manager
            .alter_table(
                Table::alter()
                    .table(ContentAudit::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ContentAudit::StrategyUsed).integer(), // i32
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentAudit::Table).to_owned())
            .await
    }
}

/// Old table, but with new column to add.
#[derive(Iden)]
enum ContentAudit {
    Table,
    StrategyUsed, // Custom enum: SelectionStrategy. Can be null (for entries predating this column).
}
