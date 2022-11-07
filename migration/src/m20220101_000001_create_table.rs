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
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Node::Id)
                            .binary_len(32)
                            .not_null()
                            .primary_key()
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Enr::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Enr::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key()
                    )
                    .col(
                        ColumnDef::new(Enr::NodeId)
                            .binary_len(32)
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(Enr::Raw)
                            .binary()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(Enr::CreatedAt)
                            .date_time()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(Enr::SequenceNumber)
                            .integer()
                            .not_null()

                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_enr_id_node_id")
                            .from(Enr::Table, Enr::NodeId)
                            .to(Node::Table, Node::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(KeyValue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(KeyValue::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key()
                    )
                    .col(
                        ColumnDef::new(KeyValue::EnrId)
                            .integer()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(KeyValue::Key)
                            .binary()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(KeyValue::Value)
                            .binary()
                            .not_null()
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_keyvalue_id_enr_id")
                            .from(KeyValue::Table, KeyValue::EnrId)
                            .to(Enr::Table, Enr::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KeyValue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Enr::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Node::Table).to_owned())
            .await?;
        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Node {
    Table,
    Id,
}

#[derive(Iden)]
enum Enr {
    Table,
    Id,
    NodeId,
    SequenceNumber,
    Raw,
    CreatedAt,
}

#[derive(Iden)]
enum KeyValue {
    Table,
    Id,
    EnrId,
    Key,
    Value,
}
