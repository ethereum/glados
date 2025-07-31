use clap::ValueEnum;
use sea_orm::{
    prelude::*,
    sea_query::{ArrayType, Nullable, ValueType, ValueTypeErr},
    strum::IntoEnumIterator,
    TryGetable,
};
use strum::{Display, EnumString};

use crate::Subprotocol;

#[derive(
    Debug, Clone, Hash, Eq, PartialEq, EnumIter, DeriveActiveEnum, ValueEnum, Display, EnumString,
)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
/// Each strategy is responsible for selecting which content key(s) to begin audits for.
pub enum HistorySelectionStrategy {
    /// Starts from genesis and goes up to the latest available block.
    Sync = 0,
    /// Selects random available block for audit.
    Random = 1,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum SelectionStrategy {
    History(HistorySelectionStrategy),
}

impl SelectionStrategy {
    pub fn subprotocol(&self) -> Subprotocol {
        match self {
            Self::History(_) => Subprotocol::History,
        }
    }

    pub fn try_from_str(subprotocol: Subprotocol, s: &str) -> Result<Self, String> {
        match subprotocol {
            Subprotocol::History => {
                HistorySelectionStrategy::from_str(s, /* ignore_case= */ true).map(Self::History)
            }
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            SelectionStrategy::History(strategy) => strategy.to_string(),
        }
    }
}

impl ActiveEnum for SelectionStrategy {
    type Value = i32;
    type ValueVec = Vec<Self::Value>;

    fn name() -> sea_orm::DynIden {
        SeaRc::new("SelectionStrategy")
    }

    fn to_value(&self) -> Self::Value {
        let (subprotocol_value, strategy_value) = match self {
            Self::History(strategy) => (Subprotocol::History.into_value(), strategy.to_value()),
        };
        (subprotocol_value << 16) | strategy_value
    }

    fn try_from_value(v: &Self::Value) -> std::prelude::v1::Result<Self, DbErr> {
        let sub_protocol_value = v >> 16;
        let strategy_value = v & 0xFFFF;
        match Subprotocol::try_from_value(&sub_protocol_value)? {
            Subprotocol::History => {
                HistorySelectionStrategy::try_from_value(&strategy_value).map(Self::History)
            }
        }
    }

    fn db_type() -> ColumnDef {
        ColumnType::Integer.def()
    }
}

impl From<SelectionStrategy> for Value {
    fn from(strategy: SelectionStrategy) -> Self {
        strategy.to_value().into()
    }
}

impl TryGetable for SelectionStrategy {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &QueryResult,
        index: I,
    ) -> std::prelude::v1::Result<Self, sea_orm::TryGetError> {
        let value = i32::try_get_by(res, index)?;
        SelectionStrategy::try_from_value(&value).map_err(TryGetError::DbErr)
    }
}

impl ValueType for SelectionStrategy {
    fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
        let value = <i32 as ValueType>::try_from(v)?;
        Self::try_from_value(&value).map_err(|_| ValueTypeErr)
    }

    fn type_name() -> String {
        i32::type_name()
    }

    fn array_type() -> ArrayType {
        i32::array_type()
    }

    fn column_type() -> ColumnType {
        i32::column_type()
    }

    fn enum_type_name() -> Option<&'static str> {
        Some(stringify!(SelectionStrategy))
    }
}

impl Nullable for SelectionStrategy {
    fn null() -> Value {
        i32::null()
    }
}

impl IntoEnumIterator for SelectionStrategy {
    type Iterator = std::vec::IntoIter<Self>;

    fn iter() -> Self::Iterator {
        [HistorySelectionStrategy::iter()
            .map(SelectionStrategy::History)
            .collect::<Vec<_>>()]
        .concat()
        .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_value() {
        assert_eq!(
            SelectionStrategy::History(HistorySelectionStrategy::Sync).to_value(),
            0
        );
        assert_eq!(
            SelectionStrategy::History(HistorySelectionStrategy::Random).to_value(),
            1
        );
    }

    #[test]
    fn try_from_value() {
        assert_eq!(
            SelectionStrategy::try_from_value(&0).unwrap(),
            SelectionStrategy::History(HistorySelectionStrategy::Sync)
        );
        assert_eq!(
            SelectionStrategy::try_from_value(&1).unwrap(),
            SelectionStrategy::History(HistorySelectionStrategy::Random)
        );
    }

    #[test]
    fn from_selection_strategy_to_value() {
        assert_eq!(
            Value::from(SelectionStrategy::History(HistorySelectionStrategy::Sync)),
            Value::Int(Some(0))
        );
        assert_eq!(
            Value::from(SelectionStrategy::History(HistorySelectionStrategy::Random)),
            Value::Int(Some(1))
        );
    }

    #[test]
    fn test_selection_strategy_as_str() {
        assert_eq!(
            SelectionStrategy::History(HistorySelectionStrategy::Sync).as_str(),
            "Sync"
        );
        assert_eq!(
            SelectionStrategy::History(HistorySelectionStrategy::Random).as_str(),
            "Random"
        );
    }

    #[test]
    fn try_from_str() {
        assert_eq!(
            SelectionStrategy::try_from_str(Subprotocol::History, "Sync").unwrap(),
            SelectionStrategy::History(HistorySelectionStrategy::Sync),
        );
        assert_eq!(
            SelectionStrategy::try_from_str(Subprotocol::History, "Random").unwrap(),
            SelectionStrategy::History(HistorySelectionStrategy::Random),
        );
    }
}
