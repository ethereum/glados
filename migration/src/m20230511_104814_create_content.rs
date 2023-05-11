use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
enum Content {
    Table,
    Id,               // Database primary key
    ProtocolId,       // Custom enum: Sub-protocol
    ContentKey,       // 33 byte full content key
    ContentId,        // 32 byte content key
    FirstAvailableAt, // datetime
}
