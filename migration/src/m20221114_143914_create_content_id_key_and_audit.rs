use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Content table
        manager
            .create_table(
                Table::create()
                    .table(Content::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Content::Id)
                            .integer() // i32
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Content::ProtocolId)
                            .integer() // i32
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Content::ContentKey)
                            .binary_len(33) // 33 bytes
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Content::ContentId)
                            .binary_len(32) // 32 bytes
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Content::FirstAvailableAt)
                            .timestamp_with_time_zone() // chrono::DateTime<FixedOffset>
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-id-protocol-and-key") // Triple column constraint
                            .col(Content::ProtocolId) // 1
                            .col(Content::ContentKey) // 2
                            .col(Content::ContentId), // 3
                    )
                    .to_owned(),
            )
            .await?;

        // Execution block metadata table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionMetadata::Table)
                    .if_not_exists()
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
            .await?;

        // ContentAudit
        manager
            .create_table(
                Table::create()
                    .table(ContentAudit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentAudit::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentAudit::ContentKey)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_contentaudit_content_key")
                            .from(ContentAudit::Table, ContentAudit::ContentKey)
                            .to(Content::Table, Content::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(ContentAudit::CreatedAt)
                            .timestamp_with_time_zone() // chrono::DateTime<FixedOffset>
                            .not_null(),
                    )
                    .col(ColumnDef::new(ContentAudit::Trace).string())
                    //.index(Index::create().name("idx-contentaudit-created_at").col(ContentAudit::CreatedAt))
                    .col(ColumnDef::new(ContentAudit::Result).integer().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Content::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ExecutionMetadata::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ContentAudit::Table).to_owned())
            .await?;

        Ok(())
    }
}

// Content that is known to exist that the Portal Network should be aware of.
#[derive(Iden)]
enum Content {
    Table,
    Id,               // Database primary key
    ProtocolId,       // Custom enum: Sub-protocol
    ContentKey,       // 33 byte full content key
    ContentId,        // 32 byte content key
    FirstAvailableAt, // datetime
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

#[derive(Iden)]
enum ContentAudit {
    Table,
    Id,
    ContentKey, // Foreign key
    CreatedAt,  // datetime
    Result,     // Custom enum: Succeed/Fail
    Trace,      // Trace object from query
}
