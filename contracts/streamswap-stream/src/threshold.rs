use cosmwasm_std::{StdError, Storage, Uint128};
use cw_storage_plus::Item;
use thiserror::Error;

use crate::state::Stream;

pub type Threshold = Uint128;

#[derive(Error, Debug, PartialEq)]
pub enum ThresholdError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Threshold not reached")]
    ThresholdNotReached {},

    #[error("Threshold reached")]
    ThresholdReached {},

    #[error("Threshold not set")]
    ThresholdNotSet {},

    #[error("Min price can't be zero")]
    ThresholdZero {},
}
pub const THRESHOLDS_STATE_KEY: &str = "thresholds";

pub struct ThresholdState<'a>(Item<'a, Threshold>);

impl<'a> ThresholdState<'a> {
    pub fn new() -> Self {
        ThresholdState(Item::new(THRESHOLDS_STATE_KEY))
    }
    pub fn set_threshold_if_any(
        &self,
        threshold: Option<Uint128>,
        storage: &mut dyn Storage,
    ) -> Result<(), ThresholdError> {
        match threshold {
            Some(threshold) => {
                if threshold.is_zero() {
                    return Err(ThresholdError::ThresholdZero {});
                }
                self.0.save(storage, &threshold)?;
                Ok(())
            }
            None => Ok(()),
        }
    }
    pub fn error_if_not_reached(
        &self,
        storage: &dyn Storage,
        stream: &Stream,
    ) -> Result<(), ThresholdError> {
        // If threshold is not set, It returns ok
        // If threshold is set, It returns error if threshold is not reached
        let threshold = self.0.may_load(storage)?;
        if let Some(threshold) = threshold {
            if stream.spent_in < threshold {
                Err(ThresholdError::ThresholdNotReached {})
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    pub fn error_if_reached(
        &self,
        storage: &dyn Storage,
        stream: &Stream,
    ) -> Result<(), ThresholdError> {
        let threshold = self.0.may_load(storage)?;
        if let Some(threshold) = threshold {
            if stream.spent_in >= threshold {
                Err(ThresholdError::ThresholdReached {})
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }
    pub fn check_if_threshold_set(&self, storage: &dyn Storage) -> Result<bool, ThresholdError> {
        let threshold = self.0.may_load(storage)?;
        Ok(threshold.is_some())
    }
    pub fn get_threshold(&self, storage: &dyn Storage) -> Result<Option<Threshold>, StdError> {
        let threshold = self.0.may_load(storage)?;
        Ok(threshold)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::state::Stream;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::{Addr, Coin, Decimal, Decimal256, Timestamp, Uint128};

    #[test]
    fn test_thresholds_state() {
        let mut storage = MockStorage::new();
        let thresholds = ThresholdState::new();
        let mut stream = Stream {
            out_asset: Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(1000),
            },
            in_supply: Uint128::new(1000),
            start_time: Timestamp::from_seconds(0),
            end_time: Timestamp::from_seconds(1000),
            last_updated: Timestamp::from_seconds(0),
            pause_date: None,
            current_streamed_price: Decimal::percent(100),
            dist_index: Decimal256::one(),
            in_denom: "uusd".to_string(),
            name: "test".to_string(),
            url: Some("test".to_string()),
            out_remaining: Uint128::new(1000),
            shares: Uint128::new(0),
            spent_in: Uint128::new(0),
            status: crate::state::Status::Active,
            treasury: Addr::unchecked("treasury"),
            stream_admin: Addr::unchecked("admin"),
            create_pool: None,
            vesting: None,
        };
        let threshold = Uint128::new(1_500_000_000_000);

        thresholds
            .set_threshold_if_any(Some(threshold), &mut storage)
            .unwrap();

        stream.spent_in = Uint128::new(1_500_000_000_000 - 1);
        let result = thresholds.error_if_not_reached(&storage, &stream.clone());
        assert_eq!(result.is_err(), true);
        stream.spent_in = Uint128::new(1_500_000_000_000);
        let result = thresholds.error_if_not_reached(&storage, &stream.clone());
        assert_eq!(result.is_err(), false);
    }
}
