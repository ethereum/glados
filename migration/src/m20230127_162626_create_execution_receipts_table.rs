use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Execution receipts table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExecutionReceipts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ExecutionReceipts::ContentKey).integer())
                    .col(
                        ColumnDef::new(ExecutionReceipts::BlockNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionReceipts::BlockHash)
                            .binary_len(32)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_execution_receipts_content_key")
                            .from(ExecutionReceipts::Table, ExecutionReceipts::ContentKey)
                            .to(ContentKey::Table, ContentKey::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_receipts-content_key")
                            .col(ExecutionReceipts::ContentKey),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_receipts-block_number")
                            .col(ExecutionReceipts::BlockNumber),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_receipts-block_hash")
                            .col(ExecutionReceipts::BlockHash),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExecutionReceipts::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum ContentKey {
    Table,
    Id,
}

#[derive(Iden)]
enum ExecutionReceipts {
    Table,
    Id,
    ContentKey,
    BlockNumber,
    BlockHash,
}
