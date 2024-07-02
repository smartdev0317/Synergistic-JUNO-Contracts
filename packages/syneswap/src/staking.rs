use cosmwasm_std::{StdError, StdResult, Uint128};
use cw20::Cw20ReceiveMsg;
use cw20::{Cw20Coin, Logo, MinterResponse};


use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone,  PartialEq)]
pub struct InstantiateMarketingInfo {
    pub project: Option<String>,
    pub description: Option<String>,
    pub marketing: Option<String>,
    pub logo: Option<Logo>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone,  PartialEq)]
pub struct TokenInstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Vec<Cw20Coin>,
    pub mint: Option<MinterResponse>,
    pub marketing: Option<InstantiateMarketingInfo>,
}

impl TokenInstantiateMsg {
    pub fn get_cap(&self) -> Option<Uint128> {
        self.mint.as_ref().and_then(|v| v.cap)
    }

    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        if !is_valid_symbol(&self.symbol) {
            return Err(StdError::generic_err(
                "Ticker symbol is not in expected format [a-zA-Z\\-]{3,12}",
            ));
        }
        if self.decimals > 18 {
            return Err(StdError::generic_err("Decimals must not exceed 18"));
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

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20QueryMsg {
    /// Returns the current balance of the given address, 0 if unset.
    /// Return type: BalanceResponse.
    Balance {
        address: String,
        duration: u64,
    },
    /// Returns metadata on the contract - name, decimals, supply, etc.
    /// Return type: TokenInfoResponse.
    TokenInfo {},
    /// Only with "mintable" extension.
    /// Returns who can mint and the hard cap on maximum tokens after minting.
    /// Return type: MinterResponse.
    Minter {},

    /// Only with "enumerable" extension
    /// Returns all accounts that have balances. Supports pagination.
    /// Return type: AllAccountsResponse.
    AllAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    TotalBalance {
        duration: u64,
    },
}

#[derive(Serialize, Deserialize, Clone,  PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Cw20ExecuteMsg {
    /// Only with "approval" extension. Allows spender to access an additional amount tokens
    /// from the owner's (env.sender) account. If expires is Some(), overwrites current allowance
    /// expiration with this one.
    /// and adds to the recipient balance.
    Mint {
        recipient: String,
        amount: Uint128,
        duration: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    // Token contract code id for initialization
    pub token: String,
    pub lock_time_frame: u64,
    pub freeze_lock_time: u64,
    pub restake_reset_flag: bool,
    pub vault_address: String,
    pub token_instantiate_msg: TokenInstantiateMsg,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        owner: String,
    },
    UpdateFreezeFlag {
        freeze_flag: bool,
    },
    UpdateFreezeLockTime {
        freeze_lock_time: u64,
    },
    UpdateLockTimeFrame {
        lock_time_frame: u64,
    },
    UpdateWaitTimeForDistribution {
        wait_time_for_distribution_in_seconds: u64,
    },
    UpdateReward {
        amount: Uint128,
    },
    UpdateRestakeResetFlag {
        flag: bool,
    },
    Distribute {},
    Receive(Cw20ReceiveMsg),
    Claim {
        duration: u64,
    },
    Restake {
        duration: u64,
    },
    UnstakeAndClaim {
        duration: u64,
    },
    UpdateSynePowerConstant {
        syne_power_constant: Uint128,
    },
    UpdateDayFactorInSeconds {
        day_factor_in_secondsc: u64,
    },
    AddNewDuration {
        duration: u64,
    },
    DepositInVaultAddress {
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Stake { duration: u64 },
    Deposit {},
}

#[derive(Serialize, Deserialize, Clone, Debug,  PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    QueryTotalReward {},
    QueryTotalRewardInContract {},
    QueryStakedByUser {
        wallet: String,
        duration: u64,
    },
    QueryTotalDailyReward {},
    QueryUserReward {
        wallet: String,
        duration: u64,
    },
    QueryUserStakedTime {
        wallet: String,
        duration: u64,
    },
    QueryDistributionWaitTime {},
    QueryTotalStakedByDuration {
        duration: u64,
    },
    QueryFreezeLockTime {},
    QueryLockTimeFrame {},
    QueryLastDistributionTime {},
    QueryConfig {},
    // QueryUserPower { wallet: String },
    // QueryTotalPower {},
    // QueryVaultAmount {},
    Balance {
        address: String,
        
    },
    /// Returns metadata on the contract - name, decimals, supply, etc.
    /// Return type: TokenInfoResponse.
    TokenInfo {},
    /// Only with "mintable" extension.
    /// Returns who can mint and the hard cap on maximum tokens after minting.
    /// Return type: MinterResponse.
    Minter {},

    /// Only with "enumerable" extension
    /// Returns all accounts that have balances. Supports pagination.
    /// Return type: AllAccountsResponse.
    AllAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    TotalBalance {
        duration: u64,
    },
}
