use sea_orm_migration::{prelude::*, schema::*};

const FK_AUDIT_ID: &str = "FK-audit_transfer_failure-audit_id";
const FK_SENDER_NODE_ENR_ID: &str = "FK-audit_transfer_failure-sender_node_enr_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditTransferFailure::Table)
                    .if_not_exists()
                    .col(pk_auto(AuditTransferFailure::Id))
                    .col(integer(AuditTransferFailure::AuditId))
                    .col(integer(AuditTransferFailure::SenderNodeEnrId))
                    .col(unsigned(AuditTransferFailure::FailureType))
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_SENDER_NODE_ENR_ID)
                            .from(AuditTransferFailure::Table, AuditTransferFailure::AuditId)
                            .to(Audit::Table, Audit::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_AUDIT_ID)
                            .from(AuditTransferFailure::Table, AuditTransferFailure::SenderNodeEnrId)
                            .to(NodeEnr::Table, NodeEnr::Id)
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
            .drop_table(Table::drop().table(AuditTransferFailure::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditTransferFailure {
    Table,
    Id,
    AuditId,
    SenderNodeEnrId,
    FailureType,
}

#[derive(DeriveIden)]
enum Audit {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NodeEnr {
    Table,
    Id,
}
