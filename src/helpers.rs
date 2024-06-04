use crate::state::CreatePool;
use crate::ContractError;
use cosmwasm_std::{Decimal256, StdError, Uint128};
use std::str::FromStr;

/// Stream validation related constants
const MIN_NAME_LENGTH: usize = 2;
const MAX_NAME_LENGTH: usize = 64;
const MIN_URL_LENGTH: usize = 12;
const MAX_URL_LENGTH: usize = 128;

/// Special characters that are allowed in stream names and urls
const SAFE_TEXT_CHARS: &str = "<>$!&?#()*+'-./\"";
const SAFE_URL_CHARS: &str = "-_:/?#@!$&()*+,;=.~[]'%";

// calculate the reward with decimal
pub fn get_decimals(value: Decimal256) -> Result<Decimal256, ContractError> {
    let stringed: &str = &value.to_string();
    let parts: &[&str] = &stringed.split('.').collect::<Vec<&str>>();
    match parts.len() {
        1 => Ok(Decimal256::zero()),
        2 => {
            let decimals = Decimal256::from_str(&("0.".to_owned() + parts[1]))?;
            Ok(decimals)
        }
        _ => Err(ContractError::InvalidDecimals {}),
    }
}

pub fn check_name_and_url(name: &String, url: &Option<String>) -> Result<(), ContractError> {
    if name.len() < MIN_NAME_LENGTH {
        return Err(ContractError::StreamNameTooShort {});
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err(ContractError::StreamNameTooLong {});
    }
    if !name.chars().all(|c| {
        c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || SAFE_TEXT_CHARS.contains(c)
    }) {
        return Err(ContractError::InvalidStreamName {});
    }

    if let Some(url) = url {
        if url.len() < MIN_URL_LENGTH {
            return Err(ContractError::StreamUrlTooShort {});
        }
        if url.len() > MAX_URL_LENGTH {
            return Err(ContractError::StreamUrlTooLong {});
        }
        if !url
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || SAFE_URL_CHARS.contains(c))
        {
            return Err(ContractError::InvalidStreamUrl {});
        }
    }
    Ok(())
}

pub fn from_semver(err: semver::Error) -> ContractError {
    ContractError::from(StdError::generic_err(format!("Semver: {}", err)))
}

pub fn check_create_pool_flag(
    out_amount: Uint128,
    flag: &Option<CreatePool>,
) -> Result<(), ContractError> {
    if let Some(pool) = flag {
        if out_amount < pool.out_amount_clp {
            return Err(ContractError::InvalidCreatePool {});
        }
    }
    Ok(())
}
