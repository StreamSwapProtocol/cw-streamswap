use cosmwasm_std::{StdError, Storage, Uint256};
use cw_storage_plus::Map;
use thiserror::Error;

use crate::state::Stream;

pub type Threshold = Uint256;

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

pub struct ThresholdState<'a>(Map<'a, u64, Threshold>);

impl<'a> ThresholdState<'a> {
    pub fn new() -> Self {
        ThresholdState(Map::new(THRESHOLDS_STATE_KEY))
    }
    pub fn set_threshold_if_any(
        &self,
        threshold: Option<Uint256>,
        stream_id: u64,
        storage: &mut dyn Storage,
    ) -> Result<(), ThresholdError> {
        match threshold {
            Some(threshold) => {
                if threshold.is_zero() {
                    return Err(ThresholdError::ThresholdZero {});
                }
                self.0.save(storage, stream_id, &threshold)?;
                Ok(())
            }
            None => Ok(()),
        }
    }
    pub fn error_if_not_reached(
        &self,
        stream_id: u64,
        storage: &dyn Storage,
        stream: &Stream,
    ) -> Result<(), ThresholdError> {
        // If threshold is not set, It returns ok
        // If threshold is set, It returns error if threshold is not reached
        let threshold = self.0.may_load(storage, stream_id)?;
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
        stream_id: u64,
        storage: &dyn Storage,
        stream: &Stream,
    ) -> Result<(), ThresholdError> {
        let threshold = self.0.may_load(storage, stream_id)?;
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
    pub fn check_if_threshold_set(
        &self,
        stream_id: u64,
        storage: &dyn Storage,
    ) -> Result<bool, ThresholdError> {
        let threshold = self.0.may_load(storage, stream_id)?;
        Ok(threshold.is_some())
    }
    pub fn get_threshold(
        &self,
        stream_id: u64,
        storage: &dyn Storage,
    ) -> Result<Option<Threshold>, StdError> {
        let threshold = self.0.may_load(storage, stream_id)?;
        Ok(threshold)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::state::Stream;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::{Addr, Decimal, Decimal256, Timestamp, Uint128};

    #[test]
    fn test_thresholds_state() {
        let mut storage = MockStorage::new();
        let thresholds = ThresholdState::new();
        let mut stream = Stream {
            out_supply: Uint256::from(1000u128),
            in_supply: Uint256::from(1000u128),
            start_time: Timestamp::from_seconds(0),
            end_time: Timestamp::from_seconds(100),
            current_streamed_price: Decimal256::percent(100),
            dist_index: Decimal256::one(),
            in_denom: "uusd".to_string(),
            last_updated: Timestamp::from_seconds(0),
            name: "test".to_string(),
            url: Some("test".to_string()),
            out_denom: "uluna".to_string(),
            out_remaining: Uint256::from(1000u128),
            pause_date: None,
            shares: Uint256::zero(),
            spent_in: Uint256::zero(),
            status: crate::state::Status::Active,
            stream_creation_denom: "uusd".to_string(),
            stream_creation_fee: Uint128::new(0),
            stream_exit_fee_percent: Decimal256::from_str("0.042").unwrap(),
            treasury: Addr::unchecked("treasury"),
        };
        let threshold = Uint256::from(1_500_000_000_000u128);
        let stream_id = 1;

        thresholds
            .set_threshold_if_any(Some(threshold), stream_id, &mut storage)
            .unwrap();

        stream.spent_in = Uint256::from(1_500_000_000_000u128 - 1);
        let result = thresholds.error_if_not_reached(stream_id, &storage, &stream.clone());
        assert_eq!(result.is_err(), true);
        stream.spent_in = Uint256::from(1_500_000_000_000u128);
        let result = thresholds.error_if_not_reached(stream_id, &storage, &stream.clone());
        assert_eq!(result.is_err(), false);
    }
}
