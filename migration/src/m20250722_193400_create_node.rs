use sea_orm_migration::{prelude::*, schema::*};

const IDX_NODE_ID: &str = "IDX-node-node_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Node::Table)
                    .col(pk_auto(Node::Id))
                    .col(binary_len_uniq(Node::NodeId, 32))
                    .col(big_unsigned(Node::NodeIdHigh))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .unique()
                    .table(Node::Table)
                    .name(IDX_NODE_ID)
                    .col(Node::NodeId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Node::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Node {
    Table,
    Id,
    NodeId,
    NodeIdHigh,
}
