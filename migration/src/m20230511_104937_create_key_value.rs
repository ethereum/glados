use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(KeyValue::Table)
                    .col(
                        ColumnDef::new(KeyValue::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(KeyValue::RecordId).integer().not_null())
                    .col(ColumnDef::new(KeyValue::Key).binary().not_null())
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-unique-record-and-key")
                            .col(Record::Id)
                            .col(KeyValue::Key),
                    )
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
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(KeyValue::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum KeyValue {
    Table,
    Id,
    RecordId,
    Key,
    Value,
}

#[derive(Iden)]
enum Record {
    Table,
    Id,
}
