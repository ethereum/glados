use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Node::Table)
                    .col(
                        ColumnDef::new(Node::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Node::NodeId).binary_len(32).not_null())
                    .index(
                        Index::create()
                            .unique()
                            .name("idx_node-node_id")
                            .col(Node::NodeId),
                    )
                    .col(
                        ColumnDef::new(Node::NodeIdHigh)
                            .big_unsigned()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Node::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
#[allow(clippy::enum_variant_names)]
enum Node {
    Table,
    Id,
    NodeId,
    NodeIdHigh,
}
