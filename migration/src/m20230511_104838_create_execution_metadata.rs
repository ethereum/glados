use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .create_table(
                Table::create()
                    .table(ExecutionMetadata::Table)
                    .col(
                        ColumnDef::new(ExecutionMetadata::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ExecutionMetadata::Content)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionMetadata::BlockNumber)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_executionmetadata_content") // Metadata points to content
                            .from(ExecutionMetadata::Table, ExecutionMetadata::Content)
                            .to(Content::Table, Content::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-metadata") // Content only has 1 metadata record.
                            .col(ExecutionMetadata::Content),
                    )
                    .to_owned(),
            )
            .await;
        manager
            .create_index(
                Index::create()
                    .name("idx_executionmetadata-block_number")
                    .table(ExecutionMetadata::Table)
                    .col(ExecutionMetadata::BlockNumber)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExecutionMetadata::Table).to_owned())
            .await
    }
}

/// Some content is associated with a block. Record metadata for that block
/// for introspection (E.g., sort content by oldest block).
#[derive(Iden)]
enum ExecutionMetadata {
    Table,
    Id,          // Database primary key
    Content,     // Foreign key
    BlockNumber, // Block number
}

// Content that is known to exist that the Portal Network should be aware of.
#[derive(Iden)]
enum Content {
    Table,
    Id, // Database primary key
}
