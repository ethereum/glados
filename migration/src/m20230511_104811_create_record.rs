use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Record::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Record::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Record::NodeId).integer().not_null())
                    .col(ColumnDef::new(Record::Raw).text().not_null())
                    .col(ColumnDef::new(Record::SequenceNumber).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-enr_id-node_id")
                            .from(Record::Table, Record::NodeId)
                            .to(Node::Table, Node::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Record::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Record {
    Table,
    Id,
    NodeId,
    Raw,
    SequenceNumber,
}

#[derive(Iden)]
enum Node {
    Table,
    Id,
}
