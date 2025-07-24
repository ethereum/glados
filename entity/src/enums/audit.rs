use sea_orm::entity::prelude::*;
use strum::{Display, EnumString};

#[derive(Debug, Clone, Hash, Eq, PartialEq, EnumIter, DeriveActiveEnum, Display, EnumString)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum AuditResult {
    Failure = 0,
    Success = 1,
}
