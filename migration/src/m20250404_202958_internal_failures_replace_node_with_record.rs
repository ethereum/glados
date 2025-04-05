use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuditInternalFailure::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(AuditInternalFailure::SenderRecordId).integer(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("FK_auditinternalfailure_sender_record_id")
                    .from(
                        AuditInternalFailure::Table,
                        AuditInternalFailure::SenderRecordId,
                    )
                    .to(Record::Table, Record::Id)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;

        // For all internal transfer failures, infer the sender record from the from the sender node.
        // Use the most recent record available for the node. This is helpful, even if wrong.
        manager.get_connection().execute_unprepared(
            "WITH maxsn AS ( SELECT r.*, ROW_NUMBER() OVER (PARTITION BY node_id ORDER BY sequence_number DESC) AS rn FROM record AS r) UPDATE audit_internal_failure AS aif SET sender_record_id=maxsn.id FROM maxsn WHERE aif.sender_node=maxsn.node_id AND maxsn.rn=1;",
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the new sender record column
        manager
            .alter_table(
                Table::alter()
                    .table(AuditInternalFailure::Table)
                    .drop_column(AuditInternalFailure::SenderRecordId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum AuditInternalFailure {
    Table,
    // Foreign key
    SenderRecordId,
}

#[derive(Iden)]
enum Record {
    Table,
    Id,
}
