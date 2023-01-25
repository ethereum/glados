use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Execution Header table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionHeader::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExecutionHeader::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ExecutionHeader::ContentidId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ExecutionHeader::BlockNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionHeader::BlockHash)
                            .binary_len(32)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_execution_header_content_id")
                            .from(ExecutionHeader::Table, ExecutionHeader::ContentidId)
                            .to(ContentId::Table, ContentId::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_header-content_id")
                            .col(ExecutionHeader::ContentidId),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_header-block_number")
                            .col(ExecutionHeader::BlockNumber),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_header-block_hash")
                            .col(ExecutionHeader::BlockHash),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExecutionHeader::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum ContentId {
    Table,
    Id,
}

#[derive(Iden)]
enum ExecutionHeader {
    Table,
    Id,
    ContentidId,
    BlockNumber,
    BlockHash,
}
