use sea_orm_migration::{prelude::*, schema::*};

const FK_NODE_ID: &str = "FK-node_enr-node_id";

const IDX_NODE_ID_SEQ: &str = "IDX-node_enr-node_id-seq";
const IDX_PROTOCOL_VERSION: &str = "IDX-node_enr-protocol_version";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NodeEnr::Table)
                    .if_not_exists()
                    .col(pk_auto(NodeEnr::Id))
                    .col(integer(NodeEnr::NodeId))
                    .col(text(NodeEnr::Raw))
                    .col(big_unsigned(NodeEnr::SequenceNumber))
                    .col(binary_len(NodeEnr::ProtocolVersions, 32))
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_NODE_ID)
                            .from(NodeEnr::Table, NodeEnr::NodeId)
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
                    .unique()
                    .table(NodeEnr::Table)
                    .name(IDX_NODE_ID_SEQ)
                    .col(NodeEnr::NodeId)
                    .col(NodeEnr::SequenceNumber)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(NodeEnr::Table)
                    .name(IDX_PROTOCOL_VERSION)
                    .col(NodeEnr::ProtocolVersions)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NodeEnr::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum NodeEnr {
    Table,
    Id,
    NodeId,
    Raw,
    SequenceNumber,
    ProtocolVersions,
}

#[derive(DeriveIden)]
enum Node {
    Table,
    Id,
}
