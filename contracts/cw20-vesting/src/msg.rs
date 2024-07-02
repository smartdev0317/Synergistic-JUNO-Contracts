use cosmwasm_std::{Addr, BlockInfo, Timestamp, Uint128};
use cw20::Logo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContractError;
use syne_curve_utils::Curve;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMarketingInfo {
    pub project: Option<String>,
    pub description: Option<String>,
    pub marketing: Option<String>,
    pub logo: Option<Logo>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<InitBalance>,
    pub mint: Option<MinterInfo>,
    pub marketing: Option<InstantiateMarketingInfo>,
    pub allowed_vesters: Option<Vec<String>>,
    pub max_curve_complexity: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct InitBalance {
    pub address: String,
    pub amount: Uint128,
    /// Optional vesting schedule
    /// It must be a decreasing curve, ending at 0, and never exceeding amount
    pub vesting: Option<Curve>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
pub struct MinterInfo {
    pub minter: String,
    /// cap is a hard cap on total supply that can be achieved by minting.
    /// This can be a monotonically increasing curve based on block time
    /// (constant value being a special case of this).
    ///
    /// Note that cap refers to total_supply.
    /// If None, there is unlimited cap.
    pub cap: Option<Curve>,
}

impl InstantiateMsg {
    pub fn get_curve(&self) -> Option<&Curve> {
        self.mint.as_ref().and_then(|v| v.cap.as_ref())
    }

    pub fn get_cap(&self, block_time: &Timestamp) -> Option<Uint128> {
        self.get_curve().map(|v| v.value(block_time.seconds()))
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(ContractError::InvalidName);
        }
        if !is_valid_symbol(&self.symbol) {
            return Err(ContractError::InvalidSymbol);
        }
        if self.decimals > 18 {
            return Err(ContractError::TooManyDecimals);
        }
        if let Some(curve) = self.get_curve() {
            curve.validate_monotonic_increasing()?;
        }
        Ok(())
    }
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 12 {
        return false;
    }
    for byte in bytes.iter() {
        if (*byte != 45) && (*byte < 65 || *byte > 90) && (*byte < 97 || *byte > 122) {
            return false;
        }
    }
    true
}

/// Asserts the vesting schedule decreases to 0 eventually, and is never more than the
/// amount being sent. If it doesn't match these conditions, returns an error.
pub fn assert_schedule_vests_amount(
    schedule: &Curve,
    amount: Uint128,
) -> Result<(), ContractError> {
    schedule.validate_monotonic_decreasing()?;
    let (low, high) = schedule.range();
    if low != 0 {
        Err(ContractError::NeverFullyVested)
    } else if high > amount.u128() {
        Err(ContractError::VestsMoreThanSent)
    } else {
        Ok(())
    }
}

/// Returns true if curve is already at 0
pub fn fully_vested(schedule: &Curve, block: &BlockInfo) -> bool {
    schedule.value(block.time.seconds()).is_zero()
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
pub struct MigrateMsg {
    pub picewise_linear_curve: Curve,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct MinterResponse {
    pub minter: String,
    /// cap is a hard cap on total supply that can be achieved by minting.
    /// This can be a monotonically increasing curve based on block time
    /// (constant value being a special case of this).
    ///
    /// Note that cap refers to total_supply.
    /// If None, there is unlimited cap.
    pub cap: Option<Curve>,
    /// This is cap evaluated at the current time
    pub current_cap: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct VestingResponse {
    /// The total vesting schedule
    pub schedule: Option<Curve>,
    /// The current amount locked. Always 0 if schedule is None
    pub locked: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct VestingAllowListResponse {
    pub allow_list: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct DelegatedResponse {
    pub delegated: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct StakingAddressResponse {
    pub address: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct MaxVestingComplexityResponse {
    pub complexity: u64,
}
