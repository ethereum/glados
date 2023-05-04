use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClientInfo::Table)
                    .col(
                        ColumnDef::new(ClientInfo::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ClientInfo::Version).string().not_null())
                    .col(ColumnDef::new(ClientInfo::NodeId).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-clientinfo_nodeid-node_id")
                            .from(ClientInfo::Table, ClientInfo::NodeId)
                            .to(Node::Table, Node::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-client_info-version-node_id") 
                            .col(ClientInfo::Version)
                            .col(ClientInfo::NodeId)
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ContentAudit::Table)
                    .add_column(
                        ColumnDef::new(ContentAudit::ClientInfoId)
                            .integer()
                    )
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk-content_audit-client_info_id")
                            .from_tbl(ContentAudit::Table)
                            .from_col(ContentAudit::ClientInfoId)
                            .to_tbl(ClientInfo::Table)
                            .to_col(ClientInfo::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ContentAudit::Table)
                    .drop_foreign_key(Alias::new("fk-content_audit-client_info_id"))
                    .drop_column(ContentAudit::ClientInfoId)
                    .to_owned()
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ClientInfo::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum ClientInfo {
    Table,
    Id,
    Version,
    NodeId,
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    ClientInfoId,
}

#[derive(Iden)]
enum Node {
    Table,
    Id,
}
