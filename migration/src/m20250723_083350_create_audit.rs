use sea_orm_migration::{prelude::*, schema::*};

const FK_CONTENT_ID: &str = "FK-audit-content_id";
const FK_CLIENT_ID: &str = "FK-audit-client_id";
const FK_NODE_ID: &str = "FK-audit-node_id";

const IDX_CONTENT_ID: &str = "IDX-audit-content_id";
const IDX_STRATEGY_ID: &str = "IDX-audit-strategy-id";
const IDX_CREATED_AT_RESULT: &str = "IDX-audit-created_at-result";
const IDX_RESULT_CREATED_AT_CONTENT_ID: &str = "IDX-audit-result-created_at-content_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Audit::Table)
                    .if_not_exists()
                    .col(pk_auto(Audit::Id))
                    .col(integer(Audit::ContentId))
                    .col(integer(Audit::ClientId))
                    .col(integer(Audit::NodeId))
                    .col(unsigned(Audit::Strategy))
                    .col(unsigned(Audit::Result))
                    .col(timestamp_with_time_zone(Audit::CreatedAt))
                    .col(text_null(Audit::Trace))
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_CONTENT_ID)
                            .from(Audit::Table, Audit::ContentId)
                            .to(Content::Table, Content::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_CLIENT_ID)
                            .from(Audit::Table, Audit::ClientId)
                            .to(Client::Table, Client::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_NODE_ID)
                            .from(Audit::Table, Audit::NodeId)
                            .to(Node::Table, Node::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Audit::Table)
                    .name(IDX_CONTENT_ID)
                    .col(Audit::ContentId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Audit::Table)
                    .name(IDX_STRATEGY_ID)
                    .col(Audit::Strategy)
                    .col(Audit::Id)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Audit::Table)
                    .name(IDX_CREATED_AT_RESULT)
                    .col(Audit::CreatedAt)
                    .col(Audit::Result)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Audit::Table)
                    .name(IDX_RESULT_CREATED_AT_CONTENT_ID)
                    .col(Audit::Result)
                    .col(Audit::CreatedAt)
                    .col(Audit::ContentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Audit::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Audit {
    Table,
    Id,
    ContentId,
    ClientId,
    NodeId,
    Strategy,
    Result,
    CreatedAt,
    Trace,
}

#[derive(DeriveIden)]
enum Content {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Client {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Node {
    Table,
    Id,
}
