use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ContentAudit::Table)
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
                            .timestamp_with_time_zone() // chrono::DateTime<Utc>
                            .not_null(),
                    )
                    .col(ColumnDef::new(ContentAudit::Result).integer().not_null())
                    .col(
                        ColumnDef::new(ContentAudit::ClientInfo)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_contentaudit_client_info")
                            .from(ContentAudit::Table, ContentAudit::ClientInfo)
                            .to(ClientInfo::Table, ClientInfo::Id),
                    )
                    .col(ColumnDef::new(ContentAudit::Node).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_contentaudit_node")
                            .from(ContentAudit::Table, ContentAudit::Node)
                            .to(Node::Table, Node::Id),
                    )
                    .col(ColumnDef::new(ContentAudit::StrategyUsed).integer())
                    .col(ColumnDef::new(ContentAudit::Trace).string().default(""))
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

#[derive(Iden)]
enum ContentAudit {
    Table,
    Id,
    ContentKey, // Foreign key
    ClientInfo, // Foreign key
    Node,       // Foreign key
    CreatedAt,  // datetime
    Result,     // Custom enum: Succeed/Fail
    StrategyUsed,
    Trace,
}

#[derive(Iden)]
enum Node {
    Table,
    Id,
}

#[derive(Iden)]
enum ClientInfo {
    Table,
    Id,
}

#[derive(Iden)]
enum Content {
    Table,
    Id, // Database primary key
}
