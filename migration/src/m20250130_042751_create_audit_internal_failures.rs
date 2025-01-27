use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditInternalFailure::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditInternalFailure::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::Audit)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_auditinternalfailure_audit")
                            .from(AuditInternalFailure::Table, AuditInternalFailure::Audit)
                            .to(ContentAudit::Table, ContentAudit::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::SenderNode)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_auditinternalfailure_sender_node")
                            .from(
                                AuditInternalFailure::Table,
                                AuditInternalFailure::SenderNode,
                            )
                            .to(Node::Table, Node::Id)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::FailureType)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditInternalFailure::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AuditInternalFailure {
    Table,
    Id,
    // Foreign key
    Audit,
    // Foreign key
    SenderNode,
    // Custom enum
    FailureType,
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    Id,
}

#[derive(Iden)]
enum Node {
    Table,
    Id,
}
