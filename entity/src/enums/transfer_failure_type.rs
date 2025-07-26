use ethportal_api::types::query_trace::QueryFailureKind;
use sea_orm::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum TransferFailureType {
    InvalidContent = 0,
    UtpConnectionFailed = 1,
    UtpTransferFailed = 2,
}

impl From<&QueryFailureKind> for TransferFailureType {
    fn from(kind: &QueryFailureKind) -> Self {
        match kind {
            QueryFailureKind::InvalidContent => Self::InvalidContent,
            QueryFailureKind::UtpConnectionFailed => Self::UtpConnectionFailed,
            QueryFailureKind::UtpTransferFailed => Self::UtpTransferFailed,
        }
    }
}
