use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .create_table(
                    Table::create()
                        .table(Node::Table)
                        .if_not_exists()
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
                        .to_owned(),
                )
                .await?;
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
                        .col(ColumnDef::new(Record::Raw).binary().not_null())
                        .col(
                            ColumnDef::new(Record::CreatedAt)
                                .timestamp_with_time_zone()
                                .not_null(),
                        )
                        .col(ColumnDef::new(Record::SequenceNumber).integer().not_null())
                        .index(
                            Index::create()
                                .unique()
                                .name("idx-record-sequence_number")
                                .col(Record::SequenceNumber),
                        )
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
                .await?;
        }
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
                            .primary_key(),
                    )
                    .col(ColumnDef::new(KeyValue::RecordId).integer().not_null())
                    .col(ColumnDef::new(KeyValue::Key).binary().not_null())
                    .col(ColumnDef::new(KeyValue::Value).binary().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_keyvalue_id-enr_id")
                            .from(KeyValue::Table, KeyValue::RecordId)
                            .to(Record::Table, Record::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
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
            .drop_table(Table::drop().table(Record::Table).to_owned())
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
    NodeId,
}

#[derive(Iden)]
enum Record {
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
    RecordId,
    Key,
    Value,
}
