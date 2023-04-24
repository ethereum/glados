use sea_orm_migration::{prelude::*, sea_orm::DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Postgres supports adding foreign keys in later migrations.
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .create_table(
                    Table::create()
                        .table(ClientInfo::Table)
                        .if_not_exists()
                        .col(
                            ColumnDef::new(ClientInfo::Id)
                                .integer()
                                .not_null()
                                .auto_increment()
                                .primary_key(),
                        )
                        .index(
                            Index::create()
                                .unique()
                                .name("idx_client_info-id")
                                .col(ClientInfo::Id),
                        )
                        .col(ColumnDef::new(ClientInfo::VersionInfo).string().not_null())
                        .to_owned(),
                )
                .await?;
            let client_info_key = TableForeignKey::new()
                .name("FK_contentaudit_client_info")
                .from_tbl(ContentAudit::Table)
                .from_col(ContentAudit::ClientInfo)
                .to_tbl(ClientInfo::Table)
                .to_col(ClientInfo::Id)
                .to_owned();

            let record_key = TableForeignKey::new()
                .name("FK_contentaudit_node")
                .from_tbl(ContentAudit::Table)
                .from_col(ContentAudit::Node)
                .to_tbl(Node::Table)
                .to_col(Node::Id)
                .to_owned();

            manager
                .alter_table(
                    Table::alter()
                        .table(ContentAudit::Table)
                        .add_column(ColumnDef::new(ContentAudit::ClientInfo).integer())
                        .add_foreign_key(&client_info_key)
                        .add_column(ColumnDef::new(ContentAudit::Node).integer())
                        .add_foreign_key(&record_key)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentAudit::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ClientInfo {
    Table,
    Id,
    VersionInfo,
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    ClientInfo,
    Node,
}

#[derive(Iden)]
enum Node {
    Table,
    Id,
}
