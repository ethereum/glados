use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .create_table(
                Table::create()
                    .table(Content::Table)
                    .col(
                        ColumnDef::new(Content::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Content::ProtocolId).unsigned().not_null())
                    .col(ColumnDef::new(Content::ContentKey).binary().not_null())
                    .col(ColumnDef::new(Content::ContentId).binary_len(32).not_null())
                    .col(
                        ColumnDef::new(Content::FirstAvailableAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-id-protocol-and-key") // Triple column constraint
                            .col(Content::ProtocolId)
                            .col(Content::ContentKey)
                            .col(Content::ContentId),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-protocol-and-key")
                            .col(Content::ProtocolId)
                            .col(Content::ContentKey),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-protocol-and-id")
                            .col(Content::ProtocolId)
                            .col(Content::ContentId),
                    )
                    .to_owned(),
            )
            .await;
        manager
            .create_index(
                Index::create()
                    .name("idx_content-time-and-protocol")
                    .table(Content::Table)
                    .col(Content::FirstAvailableAt)
                    .col(Content::ProtocolId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content-id")
                    .table(Content::Table)
                    .col(Content::ContentId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Content::Table).to_owned())
            .await
    }
}

// Content that is known to exist that the Portal Network should be aware of.
#[derive(Iden)]
#[allow(clippy::enum_variant_names)]
enum Content {
    Table,
    Id,               // Database primary key
    ProtocolId,       // Custom enum: Sub-protocol
    ContentKey,       // 33 byte full content key
    ContentId,        // 32 byte content key
    FirstAvailableAt, // datetime
}
