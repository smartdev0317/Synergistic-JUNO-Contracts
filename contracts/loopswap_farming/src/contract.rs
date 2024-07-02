use crate::queriers::{query_loop_farm_stakable_token, query_loop_farm_pending_rewards, query_loop_farm_reward_in_pool, query_loop_farm_distribution_wait_time, query_loop_farm_lock_time_frame, query_loop_farm_pool_rewards, query_loop_farm_last_distribution_time};
//use crate::response::MsgInstantiateContractResponse;
use crate::state::{
    Config, RewardInfo, CONFIG, 
    LIQUIDITY_TOKEN_MAP,
    POOL_TOTAL_COMPOUNDED_AMOUNT, STAKEABLE_INFOS,
    TOTAL_ACCUMULATED_DISTRIBUTED_AMOUNT_IN_POOL_MAP, TOTAL_REWARDS_IN_POOL, TOTAL_STAKED,
    UNCLAIMED_DISTRIBUTED_TOKEN_AMOUNT_MAP, USER_AUTO_COMPOUND_SUBSCRIPTION_MAP,
    USER_REWARD_INFO_MAP, USER_REWARD_STARTING_TIME_MAP,
    USER_STAKED_AMOUNT, POOL_REWARD_WEIGHT_MAP, CurrentStakeInfo, CURRENT_STAKE_INFO, CURRENT_UNSTAKE_INFO, CurrentUnstakeInfo, CURRENT_CLAIM_REWARD_INFO, CurrentClaimRewardInfo, TOTAL_REWARDS, PENDING_REWARDS, USER_ACTION, UserAction, TOTAL_REWARDS_WEIGHT, TREASURY_REWARDS, TreasuryRewardsInfo,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Reply,
    ReplyOn, Response, StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg, ensure_eq, ensure_ne, Decimal,
};

use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use cw_storage_plus::Bound;
use syneswap::asset::{StakeableToken};
use syneswap::factory::MigrateMsg;
use syneswap::farming::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, QueryRewardResponse,
    QueryUserRewardInPoolResponse, LoopFarmCw20HookMsg, LoopFarmExecuteMsg,
};
use syneswap::querier::query_token_balance;
// use syneswap::token::InstantiateMsg as TokenInstantiateMsg;
// use protobuf::Message;
// use crate::parse_reply::parse_reply_instantiate_data;
// const REWARD_CALCULATION_DECIMAL_PRECISION: u128 = 1000000000000u128;
const EXECUTE_STAKE_ID: u64 = 1;
const EXECUTE_UNSTAKE_AND_CLAIM_ID: u64 = 2;
const EXECUTE_CLAIM_REWARD_ID:u64 = 3;
const EXECUTE_DISTRIBUTE_BY_LIMIT: u64 = 4;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CLAIM_ACTION: u64 = 1;
const UNSTAKE_AND_CLAIM_ACTION: u64 = 2;
const STAKE_ACTION: u64 = 3;

//Initialize the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: _info.sender,
        loop_farm_contract: deps.api.addr_validate("juno1l6p6qa0l4hdhedgvwhdqcua9sq4un8ugg5rk4nhh4mnrz7a3sl9sfwzg7l")?,
        treasury_addr: deps.api.addr_validate(&_msg.treasury_addr)?,
        treasury_fee: 200000,
        fee_multiplier: 1000000,
        default_limit: 10,
        max_limit: 30
    };
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new())
}

//Execute the handle messages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { owner } => execute_update_config(deps, env, info, owner),
        ExecuteMsg::UpdateTreasuryAddr {
            treasury_addr,
        } => execute_update_treasury_addr(deps, info, treasury_addr),
        ExecuteMsg::UpdateTreasuryFee { treasury_fee } => {
            execute_update_treasury_fee(deps, info, treasury_fee)
        }
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::ClaimReward { pool_address, start_after } => {
            execute_claim_reward(deps, env, info, pool_address, start_after)
        }
        ExecuteMsg::UnstakeAndClaim { pool_address, amount, start_after } => {
            execute_unstake_and_claim(deps, env, info, pool_address, amount, start_after)
        }
        ExecuteMsg::AddStakeableToken { pool_address, liquidity_token } => {
            execute_add_stakeable_token(deps, env, info, pool_address, liquidity_token)
        }
        ExecuteMsg::AddStakeableTokens { pool_addresses, liquidity_tokens } => {
            execute_add_stakeable_tokens(deps, env, info, pool_addresses, liquidity_tokens)
        }
        ExecuteMsg::DistributeByLimit { start_after, limit } => {
            execute_distribute_by_limit(deps, env, info, start_after, limit)
        }
        ExecuteMsg::WithdrawTreasuryReward { token, amount } => {
            execute_withdraw_treasury_reward(deps, env, info, token, amount)
        }
    }
}

/// Receive cw20 token for staking in the pool
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let pool_contract_addr = info.sender;
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Stake { start_after }) => {
            if STAKEABLE_INFOS
                .may_load(deps.storage, pool_contract_addr.to_string())?
                .is_some()
            {
                execute_stake(
                    deps,
                    env,
                    Addr::unchecked(cw20_msg.sender),
                    pool_contract_addr.to_string(),
                    cw20_msg.amount,
                    start_after
                )
            } else {
                Err(StdError::generic_err("Incorrect Asset Provided"))
            }
        }

        // Ok(Cw20HookMsg::UnstakeAndClaim { start_after }) => {
        //     if LIQUIDITY_TOKEN_MAP
        //         .may_load(deps.storage, pool_contract_addr.to_string())?
        //         .is_some()
        //     {
        //         let stakeable_token_addr =
        //             LIQUIDITY_TOKEN_MAP.load(deps.storage, pool_contract_addr.to_string())?;
        //         execute_unstake_and_claim(
        //             deps,
        //             env,
        //             Addr::unchecked(cw20_msg.sender).to_string(),
        //             stakeable_token_addr,
        //             cw20_msg.amount,
        //             pool_contract_addr.to_string(),
        //             true,
        //             start_after,
        //         )
        //     } else {
        //         Err(StdError::generic_err("Incorrect Asset Provided"))
        //     }
        // }
        // Ok(Cw20HookMsg::UnstakeWithoutClaim { start_after }) => {
        //     if LIQUIDITY_TOKEN_MAP
        //         .may_load(deps.storage, pool_contract_addr.to_string())?
        //         .is_some()
        //     {
        //         let stakeable_token_addr =
        //             LIQUIDITY_TOKEN_MAP.load(deps.storage, pool_contract_addr.to_string())?;
        //         execute_unstake_and_claim(
        //             deps,
        //             env,
        //             Addr::unchecked(cw20_msg.sender).to_string(),
        //             stakeable_token_addr,
        //             cw20_msg.amount,
        //             pool_contract_addr.to_string(),
        //             false,
        //             start_after,
        //         )
        //     } else {
        //         Err(StdError::generic_err("Incorrect Asset Provided"))
        //     }
        // }
        Err(_err) => Err(StdError::generic_err("Unsuccessful")),
    }
}

/// Only owner can execute it. To update the owner address
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner_provided: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }
    if let Some(owner) = owner_provided {
        // Validate address format
        let _ = deps.api.addr_validate(&owner)?;

        config.owner = deps.api.addr_validate(&owner)?;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Only owner can execute it. To update the treasury address
pub fn execute_update_treasury_addr(
    deps: DepsMut,
    info: MessageInfo,
    treasury_addr: String,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }
    config.treasury_addr = deps.api.addr_validate(&treasury_addr)?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Only owner can execute it. To update the owner address
pub fn execute_update_treasury_fee(
    deps: DepsMut,
    info: MessageInfo,
    treasury_fee: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.treasury_fee = treasury_fee;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Allow users to stake the tokens.
pub fn execute_stake(
    deps: DepsMut,
    _env: Env,
    account: Addr,
    pool_address: String,
    amount: Uint128,
    start_after: Option<String>
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    USER_ACTION.save(deps.storage, &UserAction {
        action: STAKE_ACTION,
        account: account.clone().to_string(),
        pool_address: pool_address.clone(),
        amount: Some(amount),
        flp_token_address: None,
        is_reward_claimed: None
    })?;

    let sub_msg = SubMsg {
        id: EXECUTE_DISTRIBUTE_BY_LIMIT,
        msg: WasmMsg::Execute {
            contract_addr: config.loop_farm_contract.clone().into_string(),
            msg: to_binary(&ExecuteMsg::DistributeByLimit { start_after, limit: Some(1) })?,
            funds: vec![]
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new()
        .add_submessage(sub_msg))
}

// Allow admin to add tokens so that users can stake that token.
pub fn execute_add_stakeable_token(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_address: String,
    liquidity_token: String
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if STAKEABLE_INFOS
        .may_load(deps.storage, pool_address.to_string())?
        .is_some()
    {
        return Err(StdError::generic_err("Token already exists in list"));
    }

    let stakeable_token = StakeableToken {
        liquidity_token: liquidity_token.to_string(),
        token: pool_address.to_string(),
        distribution: vec![],
    };

    STAKEABLE_INFOS.save(deps.storage, pool_address.to_string(), &stakeable_token)?;

    LIQUIDITY_TOKEN_MAP.save(
        deps.storage,
        liquidity_token,
        &pool_address,
    )?;

    Ok(Response::new().add_attribute("action", "Pool added"))
}

// Allow admin to add tokens so that users can stake that token.
pub fn execute_add_stakeable_tokens(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_addresses: Vec<String>,
    liquidity_tokens: Vec<String>
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    ensure_eq!(pool_addresses.len(), liquidity_tokens.len(), StdError::generic_err("length mismatch"));

    for index in 0..pool_addresses.len() {
        let pool_address = pool_addresses[index].clone();
        let liquidity_token = liquidity_tokens[index].clone();
        if STAKEABLE_INFOS
            .may_load(deps.storage, pool_address.to_string())?
            .is_some()
        {
            return Err(StdError::generic_err("Token already exists in list"));
        }
    
        let stakeable_token = StakeableToken {
            liquidity_token: liquidity_token.to_string(),
            token: pool_address.to_string(),
            distribution: vec![],
        };
    
        STAKEABLE_INFOS.save(deps.storage, pool_address.to_string(), &stakeable_token)?;
    
        LIQUIDITY_TOKEN_MAP.save(
            deps.storage,
            liquidity_token,
            &pool_address,
        )?;
    }

    Ok(Response::new().add_attribute("action", "Pools added"))
}

//Allow users to unstake tokens from farming contract.
pub fn execute_unstake_and_claim(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_address: String,
    amount: Uint128,
    start_after: Option<String>
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if STAKEABLE_INFOS
        .may_load(deps.storage, pool_address.to_string())?
        .is_some()
    {
        USER_ACTION.save(deps.storage, &UserAction {
            action: UNSTAKE_AND_CLAIM_ACTION,
            account: info.sender.clone().to_string(),
            pool_address: pool_address.clone(),
            amount: Some(amount),
            flp_token_address: None,
            is_reward_claimed: None
        })?;
    
        let sub_msg = SubMsg {
            id: EXECUTE_DISTRIBUTE_BY_LIMIT,
            msg: WasmMsg::Execute {
                contract_addr: config.loop_farm_contract.clone().into_string(),
                msg: to_binary(&ExecuteMsg::DistributeByLimit { start_after, limit: Some(1) })?,
                funds: vec![]
            }
            .into(),
            gas_limit: None,
            reply_on: ReplyOn::Success,
        };
    
        Ok(Response::new()
            .add_submessage(sub_msg))
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}

//To claim rewards only
pub fn execute_claim_reward(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    pool_address: String,
    start_after: Option<String>
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    USER_ACTION.save(deps.storage, &UserAction {
        action: CLAIM_ACTION,
        account: info.sender.clone().to_string(),
        pool_address: pool_address.clone(),
        amount: None,
        flp_token_address: None,
        is_reward_claimed: None
    })?;

    let sub_msg = SubMsg {
        id: EXECUTE_DISTRIBUTE_BY_LIMIT,
        msg: WasmMsg::Execute {
            contract_addr: config.loop_farm_contract.clone().into_string(),
            msg: to_binary(&ExecuteMsg::DistributeByLimit { start_after, limit: Some(1) })?,
            funds: vec![]
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new()
        .add_submessage(sub_msg))
}

pub fn execute_distribute_by_limit(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    start_after: Option<String>,
    limit: Option<u32>
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    if let Some(start_after) = start_after.clone() {
        if STAKEABLE_INFOS
            .may_load(deps.storage, start_after)?
            .is_some()
        {
            return Err(StdError::generic_err("Token already exists in list"));
        }
    }
    Ok(
        Response::new()
            .add_message(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: config.loop_farm_contract.to_string(),
                    msg: to_binary(&ExecuteMsg::DistributeByLimit { start_after, limit })?,
                    funds: vec![],
                }
            )
        )
    )
}

pub fn execute_withdraw_treasury_reward(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    token: String,
    amount: Uint128
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut treasury_reward = TREASURY_REWARDS.load(deps.storage, token.clone()).unwrap_or(Uint128::zero());
    ensure_eq!(config.treasury_addr, info.sender, StdError::generic_err("Unauthorized"));
    ensure_eq!(amount.gt(&treasury_reward), true, StdError::generic_err("Invalid amount"));
    let msg = CosmosMsg::Wasm(
        WasmMsg::Execute {
            contract_addr: token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: config.treasury_addr.to_string(), amount: amount })?,
            funds: vec![],
        }
    );
    treasury_reward = treasury_reward.checked_sub(amount).unwrap();
    TREASURY_REWARDS.save(deps.storage, token.clone(), &treasury_reward)?;
    Ok(Response::new()
        .add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryTreasuryAddress {} => to_binary(&query_treasury_address(deps)?),
        QueryMsg::QueryTreasuryFee {} => to_binary(&query_treasury_fee(deps)?),
        QueryMsg::QueryFeeMultiplier {} => to_binary(&query_fee_multiplier(deps)?),
        QueryMsg::QueryRewardInPool {
            pool,
            distribution_token,
        } => to_binary(&query_reward_in_pool(deps, env, pool, distribution_token)?),
        QueryMsg::QueryStakedByUser {
            wallet,
            staked_token,
        } => to_binary(&query_staked_by_user(deps, env, wallet, staked_token)?),
        QueryMsg::QueryTotalStaked { staked_token } => {
            to_binary(&query_staked(deps, env, staked_token)?)
        }
        QueryMsg::QueryListOfStakeableTokens { start_after, limit } => to_binary(
            &query_list_of_stakeable_tokens(deps, env, start_after, limit)?,
        ),
        QueryMsg::QueryListOfDistributableTokensByPool { pool } => {
            to_binary(&query_pool_rewards(deps, env, pool)?)
        }
        // QueryMsg::QueryStakeableInfo { start_after, limit } => {
        //     to_binary(&query_stakeable_info(deps, start_after, limit)?)
        // }
        QueryMsg::QueryUserRewardInPool { wallet, pool } => {
            to_binary(&query_user_reward_in_pool(deps, env, wallet, pool)?)
        }
        QueryMsg::QueryUserStakedTime { wallet, pool } => {
            to_binary(&query_user_staked_time(deps, wallet, pool)?)
        }
        QueryMsg::QueryDistributionWaitTime {} => to_binary(&query_distribution_wait_time(deps)?),
        QueryMsg::QueryLockTimeFrame {} => to_binary(&query_lock_time_frame(deps)?),
        // QueryMsg::QueryLockTimeFrameForAutoCompound {} => {
        //     to_binary(&query_lock_time_frame_for_auto_compound(deps)?)
        // }
        QueryMsg::QueryLastDistributionTime { pool_address } => {
            to_binary(&query_last_distribution_time(deps, pool_address)?)
        }
        // QueryMsg::QueryTotalDistributedAmountInPool {
        //     pool,
        //     dist_token_addr,
        // } => to_binary(&query_total_ditributed_amount_in_pool(
        //     deps,
        //     pool,
        //     dist_token_addr,
        // )?),
        // QueryMsg::QuerySecondAdminAddress {} => to_binary(&query_get_second_admin_address(deps)?),
        QueryMsg::QueryGetDistributeableTokenBalance { dist_token_addr } => to_binary(
            &query_get_distibuteable_token_balance(deps, env, dist_token_addr)?,
        ),
        QueryMsg::QueryGetUserAutoCompoundSubription {
            user_address,
            pool_address,
        } => to_binary(&query_get_user_auto_compound_subscription(
            deps,
            env,
            user_address,
            pool_address,
        )?),

        QueryMsg::QueryGetTotalCompounded { pool_addr } => {
            to_binary(&query_get_total_compounded(deps, pool_addr)?)
        }
        QueryMsg::QueryFlpTokenFromPoolAddress { pool_address } => {
            to_binary(&query_flp_token_address(deps, pool_address)?)
        }
        QueryMsg::TreasuryReward { token } => {
            to_binary(&query_treasury_rewards(deps, token)?)
        }
    }
}

pub fn query_treasury_address(deps: Deps) -> StdResult<String> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.treasury_addr.to_string())
}

pub fn query_treasury_fee(deps: Deps) -> StdResult<u64> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.treasury_fee)
}

pub fn query_fee_multiplier(deps: Deps) -> StdResult<u64> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.fee_multiplier)
}

pub fn query_get_total_compounded(deps: Deps, pool_address: String) -> StdResult<Uint128> {
    Ok(POOL_TOTAL_COMPOUNDED_AMOUNT
        .may_load(deps.storage, pool_address)?
        .unwrap_or_else(Uint128::zero))
}

pub fn query_get_user_auto_compound_subscription(
    deps: Deps,
    _env: Env,
    user_address: String,
    pool_address: String,
) -> StdResult<bool> {
    let mut user_pool_address = user_address;
    user_pool_address.push_str(&pool_address);
    let user_opt_for_auto_compound = if let Some(user_opt_for_auto_compound) =
        USER_AUTO_COMPOUND_SUBSCRIPTION_MAP.may_load(deps.storage, user_pool_address)?
    {
        user_opt_for_auto_compound
    } else {
        false
    };
    Ok(user_opt_for_auto_compound)
}

//for testing only. gives us amount of particular reward in a pool
pub fn query_reward_in_pool(
    deps: Deps,
    _env: Env,
    pool: String,
    distribution_token: String,
) -> StdResult<Uint128> {
    let config = CONFIG.load(deps.storage)?;

    if STAKEABLE_INFOS
        .may_load(deps.storage, pool.clone())?
        .is_some()
    {
        query_loop_farm_reward_in_pool(deps.querier, config.loop_farm_contract, pool, distribution_token)
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}

// Tells us about the staked value of pool by user.
pub fn query_staked_by_user(
    deps: Deps,
    _env: Env,
    wallet: String,
    staked_token: String,
) -> StdResult<Uint128> {
    if STAKEABLE_INFOS
        .may_load(deps.storage, staked_token.clone())?
        .is_some()
    {
        let mut resp = Uint128::zero();
        let mut key = wallet;
        key.push_str(&staked_token);
        let result = USER_STAKED_AMOUNT.may_load(deps.storage, key.clone())?;
        if let Some(result) = result {
            resp = result;
        }

        Ok(resp)
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}

//Informs us about all staked value in a pool
pub fn query_staked(deps: Deps, _env: Env, staked_token: String) -> StdResult<Uint128> {
    if STAKEABLE_INFOS
        .may_load(deps.storage, staked_token.clone())?
        .is_some()
    {
        let mut resp = Uint128::zero();
        let key = staked_token;
        let result = TOTAL_STAKED.may_load(deps.storage, key)?;
    
        if let Some(result) = result {
            resp = result;
        }
    
        Ok(resp)
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}

// paginated list of stakeable_tokens
pub fn query_list_of_stakeable_tokens(
    deps: Deps,
    _env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<StakeableToken>> {
    let mut bound = None;
    if let Some(start_after) = start_after {
        if STAKEABLE_INFOS
            .may_load(deps.storage, start_after.to_string())?
            .is_some()
        {
            bound = Some(Bound::exclusive(start_after));
        } else {
            return Err(StdError::generic_err("not a valid address passed"));
        }
    }
    let config = CONFIG.load(deps.storage).unwrap();
    let limit = limit.unwrap_or(config.default_limit).min(config.max_limit) as usize;
    let stakeable_infos_tokens_result: StdResult<Vec<_>> = STAKEABLE_INFOS
        .range(deps.storage, bound, None, Order::Ascending)
        .take(limit)
        .collect();
    let mut st: Vec<StakeableToken> = vec![];
    if let Ok(stakeable_infos_token) = stakeable_infos_tokens_result {
        for i in stakeable_infos_token {
            let stakeable_info = query_loop_farm_stakable_token(deps.querier, config.loop_farm_contract.clone().to_string(), i.0)?;
            st.push(stakeable_info);
        }
    }

    Ok(st)
}

//Will pass the pool info. Tell about all tokens to be distributed to that pool...
pub fn query_pool_rewards(
    deps: Deps,
    _env: Env,
    pool: String,
) -> StdResult<Vec<QueryRewardResponse>> {
    let config = CONFIG.load(deps.storage)?;

    if STAKEABLE_INFOS
        .may_load(deps.storage, pool.clone())?
        .is_some()
    {
        query_loop_farm_pool_rewards(deps.querier, config.loop_farm_contract.clone().to_string(), pool)
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}
// testing only. gives us the paginated LP token generated against the stakeable token
// pub fn query_stakeable_info(
//     deps: Deps,
//     start_after: Option<String>,
//     limit: Option<u32>,
// ) -> StdResult<Vec<StakeableToken>> {
//     let mut bound = None;
//     if let Some(start_after) = start_after {
//         if STAKEABLE_INFOS
//             .may_load(deps.storage, start_after.to_string())?
//             .is_some()
//         {
//             bound = Some(Bound::exclusive(start_after));
//         } else {
//             return Err(StdError::generic_err("not a valid address passed"));
//         }
//     }
//     let config = CONFIG.load(deps.storage).unwrap();
//     let limit = limit.unwrap_or(config.default_limit).min(config.max_limit) as usize;
//     let stakeable_tokens_result: StdResult<Vec<_>> = STAKEABLE_INFOS
//         .range(deps.storage, bound, None, Order::Ascending)
//         .take(limit)
//         .collect();
//     let mut st: Vec<StakeableToken> = vec![];
//     if let Ok(stakeable_tokens) = stakeable_tokens_result {
//         for i in stakeable_tokens {
//             st.push(i.1);
//         }
//     }
//     Ok(st)
// }

// Tell reward of users of the requested pools.
pub fn query_user_reward_in_pool(
    deps: Deps,
    env: Env,
    wallet: String,
    pool_address: String,
) -> StdResult<Vec<QueryUserRewardInPoolResponse>> {
    let config = CONFIG.load(deps.storage)?;

    if STAKEABLE_INFOS
        .may_load(deps.storage, pool_address.clone())?
        .is_some()
    {
        let mut resp: Vec<QueryUserRewardInPoolResponse> = vec![];
        let stakeable_token = query_loop_farm_stakable_token(deps.querier, config.loop_farm_contract.clone().to_string(), pool_address.clone())?;
        // get total_pending_rewards before staking
        let current_pending_rewards = query_loop_farm_pending_rewards(deps.querier, config.loop_farm_contract, env.contract.address.to_string(), pool_address.clone()).unwrap_or(vec![]);
        let total_rewards = TOTAL_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
        let total_rewards_weight = TOTAL_REWARDS_WEIGHT.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
        let pending_rewards = PENDING_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);

        let mut resp2 = QueryUserRewardInPoolResponse {
            pool: pool_address.clone(),
            rewards_info: vec![],
        };
        let mut user_pool_key: String = wallet.to_string();
        user_pool_key.push_str(&pool_address);
        // let mut user_opt_for_auto_compound = false;
        // if let Some(true) =
        //     USER_AUTO_COMPOUND_SUBSCRIPTION_MAP.may_load(deps.storage, user_pool_key.to_string())?
        // {
        //     user_opt_for_auto_compound = true;
        // }
        let user_staked = if let Some(user_staked) =
            USER_STAKED_AMOUNT.may_load(deps.storage, user_pool_key.clone())?
        {
            user_staked
        } else {
            Uint128::zero()
        };
        let total_staked =
            get_total_staked_amount_in_pool_from_map_storage(deps.storage, pool_address.to_string());
        if total_staked.gt(&Uint128::zero()) {
            for distt in stakeable_token.distribution.iter() {
                let mut user_pool_dist_key = wallet.to_string();
                let mut pool_dist_key: String = pool_address.to_string();
                pool_dist_key.push_str(&distt.token.to_string());
                user_pool_dist_key.push_str(&pool_dist_key.to_string());
    
                //geting pool reward
                let current_pending_reward = current_pending_rewards.clone().into_iter().find(|reward| reward.0.eq(&distt.token)).unwrap_or((String::default(), Uint128::zero()));
                let mut current_pending_reward = current_pending_reward.1;
                for reward in pending_rewards.clone().iter() {
                    if reward.0.eq(&distt.token) {
                        current_pending_reward -= reward.1;
                    }
    
                };
    
                let mut _current_reward_weight = Decimal::zero();
                let total_reward_index = 
                    total_rewards
                        .clone()
                        .into_iter()
                        .position(|reward| reward.0.eq(&distt.token));
                if let Some(total_reward_index) = total_reward_index {
                    _current_reward_weight = total_rewards_weight[total_reward_index].1 + Decimal::from_ratio(current_pending_reward, total_staked);
                } else {
                    _current_reward_weight = Decimal::from_ratio(current_pending_reward, total_staked);
                }
    
                //getting user reward index
                let user_reward_info = 
                    if let Some(user_reward_info) = USER_REWARD_INFO_MAP.may_load(deps.storage, user_pool_dist_key.to_string())? {
                        user_reward_info
                    } else {
                        RewardInfo {
                            pool_reward_weight: _current_reward_weight,
                            pending_reward: Uint128::zero(),
                        }
                    };
    
                // getting user reward difference from it's last stake to current pool index
                let diff_priv_and_curr_reward_weight = 
                    _current_reward_weight - user_reward_info.pool_reward_weight;
    
                //calculating reward to be distributed
                let mut reward_to_be_dist = 
                    if total_staked == Uint128::zero() { 
                        Uint128::zero() 
                    } else {
                        diff_priv_and_curr_reward_weight
                            .checked_mul(Decimal::from_ratio(user_staked, Uint128::one()))
                            .unwrap()
                            .to_uint_floor()
                    };
                reward_to_be_dist += user_reward_info.pending_reward;
                // if user_opt_for_auto_compound {
                //     let user_compounded_reward_index = USER_COMPOUNDED_REWARD_INFO_MAP
                //         .may_load(deps.storage, user_pool_dist_key.to_string())
                //         .unwrap()
                //         .unwrap_or_else(|| RewardInfo {
                //             reward_index: Uint128::zero(),
                //             pending_reward: Uint128::zero(),
                //         });
    
                //     let pool_compound_reward_index = POOL_COMPOUNDED_INDEX_MAP
                //         .may_load(deps.storage, pool_dist_key.to_string())
                //         .unwrap()
                //         .unwrap_or_else(Uint128::zero);
    
                //     let diff_priv_and_curr_compound_reward_index =
                //         pool_compound_reward_index - user_compounded_reward_index.reward_index;
                //     let mut compound_reward = diff_priv_and_curr_compound_reward_index
                //         .multiply_ratio(user_staked, Uint128::new(1u128));
                //     compound_reward += user_compounded_reward_index.pending_reward;
                //     reward_to_be_dist += compound_reward;
                // }
                // reward_to_be_dist = reward_to_be_dist.multiply_ratio(
                //     Uint128::new(1),
                //     Uint128::new(REWARD_CALCULATION_DECIMAL_PRECISION),
                // );
    
                if reward_to_be_dist.gt(&Uint128::zero()) {
                    resp2
                        .rewards_info
                        .push(
                            (
                                distt.token.clone(),
                                reward_to_be_dist
                                    .multiply_ratio(
                                        config.fee_multiplier - config.treasury_fee,
                                        config.fee_multiplier
                                    )
                            )
                        )
                }
            }
        }

        resp.push(resp2);
        Ok(resp)
    } else {
        Err(StdError::generic_err("Incorrect Asset Provided"))
    }
}

pub fn get_unclaimed_distirbuted_token_amount_from_map_storage(
    store: &dyn Storage,
    distributed_token_address: String,
) -> Uint128 {
    let unclaimed_amount_of_distributed_token_in_contract: Uint128 =
        if let Some(unclaimed_amount_of_distributed_token_in_contract) =
            UNCLAIMED_DISTRIBUTED_TOKEN_AMOUNT_MAP
                .may_load(store, distributed_token_address)
                .unwrap()
        {
            unclaimed_amount_of_distributed_token_in_contract
        } else {
            Uint128::zero()
        };
    unclaimed_amount_of_distributed_token_in_contract
}

pub fn get_user_reward_info_from_map_storage(
    store: &dyn Storage,
    user_pool_dist_address: String,
    default_reward_weight: Decimal,
) -> RewardInfo {
    let user_reward_info: RewardInfo = if let Some(user_reward_info) = USER_REWARD_INFO_MAP
        .may_load(store, user_pool_dist_address)
        .unwrap()
    {
        user_reward_info
    } else {
        RewardInfo {
            pool_reward_weight: default_reward_weight,
            pending_reward: Uint128::zero(),
        }
    };
    user_reward_info
}

pub fn get_reward_weight_map_from_map_storage(
    store: &dyn Storage,
    pool_dist_address: String,
) -> Uint128 {
    let reward_weight: Uint128 = if let Some(reward_weight) = POOL_REWARD_WEIGHT_MAP
        .may_load(store, pool_dist_address)
        .unwrap()
    {
        reward_weight
    } else {
        Uint128::zero()
    };
    reward_weight
}

pub fn get_total_accumulated_distributed_amount_in_pool_from_map_storage(
    store: &dyn Storage,
    pool_dist_address: String,
) -> Uint128 {
    let total_accumulate_amount_in_pool: Uint128 = if let Some(total_accumulate_amount_in_pool) =
        TOTAL_ACCUMULATED_DISTRIBUTED_AMOUNT_IN_POOL_MAP
            .may_load(store, pool_dist_address)
            .unwrap()
    {
        total_accumulate_amount_in_pool
    } else {
        Uint128::zero()
    };
    total_accumulate_amount_in_pool
}

pub fn get_total_reward_in_pool_from_map_storage(
    store: &dyn Storage,
    pool_address: String,
) -> Uint128 {
    let total_reward_in_pool: Uint128 = if let Some(total_reward_in_pool) =
        TOTAL_REWARDS_IN_POOL.may_load(store, pool_address).unwrap()
    {
        total_reward_in_pool
    } else {
        Uint128::zero()
    };
    total_reward_in_pool
}

pub fn get_total_staked_amount_in_pool_from_map_storage(
    store: &dyn Storage,
    pool_address: String,
) -> Uint128 {
    let total_staked_amount_in_pool: Uint128 = if let Some(total_staked_amount_in_pool) =
        TOTAL_STAKED.may_load(store, pool_address).unwrap()
    {
        total_staked_amount_in_pool
    } else {
        Uint128::zero()
    };
    total_staked_amount_in_pool
}

pub fn get_user_staked_amount_in_pool_from_map_storage(
    store: &dyn Storage,
    user_pool_address: String,
) -> Uint128 {
    let user_reward_issued_token_amount_in_pool: Uint128 =
        if let Some(user_reward_issued_token_amount_in_pool) = USER_STAKED_AMOUNT
            .may_load(store, user_pool_address)
            .unwrap()
        {
            user_reward_issued_token_amount_in_pool
        } else {
            Uint128::zero()
        };
    user_reward_issued_token_amount_in_pool
}

//query to get user staked time
pub fn query_user_staked_time(deps: Deps, wallet: String, pool: String) -> StdResult<String> {
    let mut user_pool_key = String::from(&wallet);
    user_pool_key.push_str(&pool);
    if let Some(user_staked_time) =
        USER_REWARD_STARTING_TIME_MAP.may_load(deps.storage, user_pool_key)?
    {
        Ok(user_staked_time.to_string())
    } else {
        Ok("".to_string())
    }
}

//query lock time frame
pub fn query_lock_time_frame(deps: Deps) -> StdResult<u64> {
    let config = CONFIG.load(deps.storage)?;
    query_loop_farm_lock_time_frame(deps.querier, config.loop_farm_contract)
}

// //query lock time frame
// pub fn query_lock_time_frame_for_auto_compound(deps: Deps) -> StdResult<u64> {
//     Ok(CONFIG
//         .load(deps.storage)?
//         .lock_time_frame_for_compound_reward)
// }

// //query distribution wait time frame
pub fn query_distribution_wait_time(deps: Deps) -> StdResult<u64> {
    let config = CONFIG.load(deps.storage)?;
    query_loop_farm_distribution_wait_time(deps.querier, config.loop_farm_contract)
}

// pub fn query_total_ditributed_amount_in_pool(
//     deps: Deps,
//     pool_addr: String,
//     dist_token_addr: String,
// ) -> StdResult<Uint128> {
//     let mut pool_dist_addr = pool_addr;
//     pool_dist_addr.push_str(dist_token_addr.as_str());
//     Ok(TOTAL_ACCUMULATED_DISTRIBUTED_AMOUNT_IN_POOL_MAP
//         .load(deps.storage, pool_dist_addr)
//         .unwrap_or_else(|_| Uint128::zero()))
// }

// pub fn query_get_second_admin_address(deps: Deps) -> StdResult<String> {
//     let config: Config = CONFIG.load(deps.storage).unwrap();
//     Ok(config.second_owner)
// }

pub fn query_get_distibuteable_token_balance(
    deps: Deps,
    env: Env,
    dist_token_addr: String,
) -> StdResult<String> {
    let balance = get_unclaimed_distirbuted_token_amount_from_map_storage(
        deps.storage,
        dist_token_addr.to_string(),
    );

    Ok((query_token_balance(
        &deps.querier,
        deps.api.addr_validate(&dist_token_addr)?,
        env.contract.address,
    )? - balance)
        .to_string())
}

pub fn query_last_distribution_time(deps: Deps, pool_address: String) -> StdResult<u64> {
    let config = CONFIG.load(deps.storage)?;
    query_loop_farm_last_distribution_time(deps.querier, config.loop_farm_contract, pool_address)
}

pub fn query_flp_token_address(deps: Deps, pool_address: String) -> StdResult<String> {
    let stakeable_token =
        if let Some(stakeable_token) = STAKEABLE_INFOS.may_load(deps.storage, pool_address)? {
            stakeable_token
        } else {
            return Err(StdError::generic_err("Pool not found"));
        };

    Ok(stakeable_token.liquidity_token)
}

pub fn query_treasury_rewards(deps: Deps, token: String) -> StdResult<TreasuryRewardsInfo> {
    let treasury_reward = TREASURY_REWARDS.load(deps.storage, token.clone()).unwrap_or(Uint128::zero());
    Ok(TreasuryRewardsInfo { token, amount: treasury_reward })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        EXECUTE_STAKE_ID => handle_stake_reply(deps, env, msg),
        EXECUTE_UNSTAKE_AND_CLAIM_ID => handle_unstake_and_claim_reply(deps, env, msg),
        EXECUTE_CLAIM_REWARD_ID => handle_claim_reward_reply(deps, env, msg),
        EXECUTE_DISTRIBUTE_BY_LIMIT => handle_distribute_by_limit_reply(deps, env, msg),
        id => Err(StdError::generic_err(format!("Unknown reply id: {}", id))),
    }
}

fn handle_distribute_by_limit_reply(deps: DepsMut, env: Env, msg: Reply) ->  StdResult<Response> {
    if msg.result.is_err() {
        return Err(StdError::generic_err(
            "no successful response get from staking reply data",
        ));
    }

    let user_action = USER_ACTION.load(deps.storage)?;

    match user_action.action {
        UNSTAKE_AND_CLAIM_ACTION => handle_unstake_and_claim(deps, env, user_action),
        CLAIM_ACTION => handle_claim(deps, env, user_action),
        STAKE_ACTION => handle_stake(deps, env, user_action),
        id => Err(StdError::generic_err(format!("Unknown user action id: {}", id)))
    }
}

fn handle_unstake_and_claim(deps: DepsMut, env: Env, user_action: UserAction) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let pool_address = user_action.pool_address;
    let sender = user_action.account;
    let amount = user_action.amount.unwrap();
    ensure_eq!(amount.gt(&Uint128::zero()), true, StdError::generic_err("Invalid zero amount"));
    let stakeable_info = STAKEABLE_INFOS.load(deps.storage, pool_address.to_string())?;
    ensure_ne!(stakeable_info.liquidity_token, "", StdError::generic_err("Undefined liquidity token address"));

    // get current_pending_rewards before staking
    let current_pending_rewards = query_loop_farm_pending_rewards(deps.querier, config.loop_farm_contract.clone(), env.contract.address.to_string(), pool_address.clone())?;

    CURRENT_UNSTAKE_INFO.save(deps.storage, &CurrentUnstakeInfo { sender, pool_address: pool_address.clone(), amount, current_pending_rewards })?;

    let total_staked = TOTAL_STAKED.load(deps.storage, pool_address)?;

    let sub_msg;
        sub_msg = SubMsg {
            id: EXECUTE_UNSTAKE_AND_CLAIM_ID,
            msg: WasmMsg::Execute {
                contract_addr: stakeable_info.liquidity_token,
                msg: to_binary(&Cw20ExecuteMsg::Send { contract: config.loop_farm_contract.to_string(), amount: total_staked, msg: to_binary(&LoopFarmCw20HookMsg::UnstakeAndClaim {})? })?,
                funds: vec![]
            }
            .into(),
            gas_limit: None,
            reply_on: ReplyOn::Success,
        };

    Ok(Response::new()
        .add_submessage(sub_msg))
}

fn handle_claim(deps: DepsMut, env: Env, user_action: UserAction) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let user = user_action.account;
    let pool_address = user_action.pool_address;

    // get current_pending_rewards before staking
    let current_pending_rewards = query_loop_farm_pending_rewards(deps.querier, config.loop_farm_contract.clone(), env.contract.address.to_string(), pool_address.clone())?;

    // save staking data and current_pending_rewards for using after sub_msg
    CURRENT_CLAIM_REWARD_INFO.save(deps.storage, &CurrentClaimRewardInfo { account: user, pool_address: pool_address.clone(), current_pending_rewards })?;

    let sub_msg = SubMsg {
        id: EXECUTE_CLAIM_REWARD_ID,
        msg: WasmMsg::Execute {
            contract_addr: config.loop_farm_contract.to_string(),
            msg: to_binary(&LoopFarmExecuteMsg::ClaimReward { pool_address })?,
            funds: vec![]
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };
    Ok(Response::new()
        .add_submessage(sub_msg))
}

fn handle_stake(deps: DepsMut, env: Env, user_action: UserAction) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let account = user_action.account;
    let pool_address = user_action.pool_address;
    let amount = user_action.amount.unwrap_or(Uint128::zero());

    // get total_pending_rewards before staking
    let current_pending_rewards = query_loop_farm_pending_rewards(
        deps.querier, 
        config.loop_farm_contract.clone(), 
        env.contract.address.to_string(),
        pool_address.clone()
    )?;

    // save staking data and total_pending_rewards for using after sub_msg
    CURRENT_STAKE_INFO.save(deps.storage, &CurrentStakeInfo { sender: account.to_string(), amount, pool_address: pool_address.clone(), current_pending_rewards })?;

    // stake lp tokens to loop_farm_contract
    let sub_msg = SubMsg {
        id: EXECUTE_STAKE_ID,
        msg: WasmMsg::Execute {
            contract_addr: pool_address,
            msg: to_binary(&Cw20ExecuteMsg::Send { contract: config.loop_farm_contract.to_string(), amount, msg: to_binary(&LoopFarmCw20HookMsg::Stake {})? })?,
            funds: vec![]
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };
    Ok(Response::new()
        .add_submessage(sub_msg))
}

fn handle_stake_reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    if msg.result.is_err() {
        return Err(StdError::generic_err(
            "no successful response get from staking reply data",
        ));
    }

    let config = CONFIG.load(deps.storage)?;
    let current_stake_info = CURRENT_STAKE_INFO.load(deps.storage)?;
    let pool_address = current_stake_info.pool_address;
    let amount = current_stake_info.amount;
    let account = current_stake_info.sender;
    let current_pending_rewards = current_stake_info.current_pending_rewards;
    let mut total_rewards = TOTAL_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let mut total_rewards_weight = TOTAL_REWARDS_WEIGHT.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let mut pending_rewards = PENDING_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);

    // get stakeable token info from loop farm
    let stakeable_token = query_loop_farm_stakable_token(deps.querier, config.loop_farm_contract.clone().to_string(), pool_address.clone())?;

    let asset_infos = pool_address.clone();

    // get total staked before staking
    let total_staked =
        get_total_staked_amount_in_pool_from_map_storage(deps.storage, pool_address.to_string());
    let mut user_pool_key = account.clone();
    user_pool_key.push_str(&pool_address);

    // get user staked before staking
    let mut user_staked =
        get_user_staked_amount_in_pool_from_map_storage(deps.storage, user_pool_key.to_string());

    let share: Uint128 = amount;

    // // let user_opt_for_auto_compound = USER_AUTO_COMPOUND_SUBSCRIPTION_MAP
    // //     .may_load(deps.storage, user_pool_key.to_string())?
    // //     .unwrap_or(false);

    // if total staked before staking is zero, no pending reward difference
    if total_staked == Uint128::zero() {
        // Initial share = collateral amount
        for dist_itr in stakeable_token.distribution.iter() {
            let mut pool_dist_key: String = asset_infos.to_string();
            pool_dist_key.push_str(&dist_itr.token.to_string());
            let mut user_pool_dist_key = account.clone();

            //geting pool reward
            let current_pending_reward = current_pending_rewards.clone().into_iter().find(|reward| reward.0.eq(&dist_itr.token)).unwrap_or_default();
            let mut current_pending_reward = current_pending_reward.1;
            for reward in pending_rewards.clone().iter() {
                if reward.0.eq(&dist_itr.token) {
                    current_pending_reward -= reward.1;
                }
            };

            let mut _total_reward_weight = Decimal::zero();
            let total_reward_index = total_rewards.clone().into_iter().position(|reward| reward.0.eq(&dist_itr.token));
            if let Some(total_reward_index) = total_reward_index {
                total_rewards[total_reward_index].1 += current_pending_reward;
                _total_reward_weight = total_rewards_weight[total_reward_index].1;
            } else {
                total_rewards.push((dist_itr.token.clone(), current_pending_reward));
                total_rewards_weight.push((dist_itr.token.clone(), Decimal::zero()));
                _total_reward_weight = Decimal::zero();
            }

            user_pool_dist_key.push_str(&pool_dist_key.to_string());

            //getting user reward index
            let user_reward_info = get_user_reward_info_from_map_storage(
                deps.storage,
                user_pool_dist_key.to_string(),
                _total_reward_weight,
            );

            USER_REWARD_INFO_MAP.save(
                deps.storage,
                user_pool_dist_key.to_string(),
                &user_reward_info,
            )?;
        }
    // update user pending reward and user's pool_reward snapshot
    } else {
        for dist_itr in stakeable_token.distribution.iter() {
            let mut pool_dist_key: String = asset_infos.to_string();
            pool_dist_key.push_str(&dist_itr.token.to_string());
            let mut user_pool_dist_key = account.to_string();
            user_pool_dist_key.push_str(&pool_dist_key.to_string());

            //geting pool reward
            let current_pending_reward = current_pending_rewards.clone().into_iter().find(|reward| reward.0.eq(&dist_itr.token)).unwrap_or_default();
            let mut current_pending_reward = current_pending_reward.1;
            for reward in pending_rewards.clone().iter() {
                if reward.0.eq(&dist_itr.token) {
                    current_pending_reward -= reward.1;
                }
            };

            let mut _total_reward_weight = Decimal::zero();
            let total_reward_index = total_rewards.clone().into_iter().position(|reward| reward.0.eq(&dist_itr.token));
            if let Some(total_reward_index) = total_reward_index {
                total_rewards[total_reward_index].1 += current_pending_reward;
                total_rewards_weight[total_reward_index].1 += Decimal::from_ratio(current_pending_reward, total_staked);
                _total_reward_weight = total_rewards_weight[total_reward_index].1;
            } else {
                total_rewards.push((dist_itr.token.clone(), current_pending_reward));
                total_rewards_weight.push((dist_itr.token.clone(), Decimal::from_ratio(current_pending_reward, total_staked)));
                _total_reward_weight = Decimal::from_ratio(current_pending_reward, total_staked);
            }

            //getting user reward weight
            let mut user_reward_info = get_user_reward_info_from_map_storage(
                deps.storage,
                user_pool_dist_key.to_string(),
                _total_reward_weight,
            );

            //calculating reward to be distributed
            let diff_priv_and_curr_reward_weight =
            _total_reward_weight - user_reward_info.pool_reward_weight;

            user_reward_info.pending_reward +=
            diff_priv_and_curr_reward_weight.checked_mul(Decimal::from_ratio(user_staked, Uint128::one())).unwrap().to_uint_floor();
            user_reward_info.pool_reward_weight = _total_reward_weight;

            USER_REWARD_INFO_MAP.save(
                deps.storage,
                user_pool_dist_key.to_string(),
                &user_reward_info,
            )?;

            // calculating reward index for auto compounding
            // if user_opt_for_auto_compound {
            //     let current_compounded_reward_index = POOL_COMPOUNDED_INDEX_MAP
            //         .may_load(deps.storage, pool_dist_key.to_string())?
            //         .unwrap_or_else(Uint128::zero);

            //     let mut user_compounded_reward_index = USER_COMPOUNDED_REWARD_INFO_MAP
            //         .may_load(deps.storage, user_pool_dist_key.to_string())?
            //         .unwrap_or_else(|| RewardInfo {
            //             reward_index: Uint128::zero(),
            //             pending_reward: Uint128::zero(),
            //         });

            //     let diff_priv_and_curr_compound_reward_index =
            //         current_compounded_reward_index - user_compounded_reward_index.reward_index;

            //     user_compounded_reward_index.pending_reward +=
            //         diff_priv_and_curr_compound_reward_index
            //             .multiply_ratio(user_staked, Uint128::from(1u128));

            //     user_compounded_reward_index.reward_index = current_compounded_reward_index;

            //     USER_COMPOUNDED_REWARD_INFO_MAP.save(
            //         deps.storage,
            //         user_pool_dist_key.to_string(),
            //         &user_compounded_reward_index,
            //     )?;
            // }
        }
    };
    pending_rewards = current_pending_rewards;
    TOTAL_REWARDS.save(deps.storage, pool_address.clone(), &total_rewards)?;
    PENDING_REWARDS.save(deps.storage, pool_address.clone(), &pending_rewards)?;
    TOTAL_REWARDS_WEIGHT.save(deps.storage, pool_address.clone(), &total_rewards_weight)?;
    // adding amount share to the user staked
    user_staked += share;

    // transfer FLP token to user
    // let cosmos_msg = CosmosMsg::Wasm(WasmMsg::Execute {
    //     contract_addr: stakeable_token.liquidity_token,
    //     msg: to_binary(&Cw20ExecuteMsg::Transfer {
    //         recipient: account.clone(),
    //         amount: share,
    //     })?,
    //     funds: vec![],
    // });

    // resetting time for non-compounding user and if user has subscribed for auto compounded then updating the pool total compounded amount
    // if user_opt_for_auto_compound {
    //     let mut total_compounded = POOL_TOTAL_COMPOUNDED_AMOUNT
    //         .may_load(deps.storage, asset_infos.to_string())?
    //         .unwrap_or_else(Uint128::zero);
    //     total_compounded += share;
    //     POOL_TOTAL_COMPOUNDED_AMOUNT.save(deps.storage, asset_infos, &total_compounded)?;
    // } else {
    USER_REWARD_STARTING_TIME_MAP.save(
        deps.storage,
        user_pool_key.to_string(),
        &env.block.time.seconds(),
    )?;
    // }

    USER_STAKED_AMOUNT.save(deps.storage, user_pool_key, &user_staked)?;
    TOTAL_STAKED.save(deps.storage, pool_address, &(total_staked + amount))?;
    Ok(Response::new()
        .add_attributes(vec![
            ("action", "stake"),
            ("from", &account),
            ("amount", &amount.to_string())
        ])
        // .add_message(cosmos_msg)
    )
}

fn handle_unstake_and_claim_reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {

    if msg.result.is_err() {
        return Err(StdError::generic_err(
            "no successful response get from unstaking reply data",
        ));
    }

    let config = CONFIG.load(deps.storage)?;
    let current_unstake_info = CURRENT_UNSTAKE_INFO.load(deps.storage)?;
    let pool_address = current_unstake_info.pool_address;
    let account = current_unstake_info.sender;
    let amount = current_unstake_info.amount;
    let current_pending_rewards = current_unstake_info.current_pending_rewards;
    let mut total_rewards = TOTAL_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let mut total_rewards_weight = TOTAL_REWARDS_WEIGHT.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let pending_rewards = PENDING_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);

    let mut res = Response::new();

    res = res
        .add_attribute("action", "Unstake and Claim rewards");

    // get stakeable token info from loop farm
    let stakeable_token = query_loop_farm_stakable_token(deps.querier, config.loop_farm_contract.clone().to_string(), pool_address.clone())?;
    
    let mut user_pool_key = account.clone();
    user_pool_key.push_str(&stakeable_token.token.to_string());

    let user_staked =
        get_user_staked_amount_in_pool_from_map_storage(deps.storage, user_pool_key.to_string());
    ensure_eq!(user_staked.gt(&amount), true, StdError::generic_err("Unstake amount is bigger than user staked amount"));

    let mut messages: Vec<CosmosMsg> = vec![];

    // sending user staked amount back to the user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: stakeable_token.token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: account.clone(),
            amount: amount,
        })?,
        funds: vec![],
    }));
    // updating user record
    let mut total_staked = get_total_staked_amount_in_pool_from_map_storage(
        deps.storage,
        stakeable_token.token.to_string(),
    );

    USER_STAKED_AMOUNT.save(deps.storage, user_pool_key.clone(), &user_staked.checked_sub(amount.clone()).unwrap())?;

    // let mut user_opt_for_auto_compound = false;
    // if let Some(true) = USER_AUTO_COMPOUND_SUBSCRIPTION_MAP
    //     .may_load(deps.storage, user_pool_key.to_string())
    //     .unwrap()
    // {
    //     user_opt_for_auto_compound = true;
    // }
    // let user_staked_time =
    //     USER_REWARD_STARTING_TIME_MAP.load(deps.storage, user_pool_key.to_string())?;
    // let mut total_compounded = POOL_TOTAL_COMPOUNDED_AMOUNT
    //     .may_load(deps.storage, pool_address.to_string())?
    //     .unwrap_or_else(Uint128::zero);

    // subtracting user staked amount from total compounded amount in the pool
    // if user_opt_for_auto_compound {
    //     total_compounded -= user_staked;
    //     POOL_TOTAL_COMPOUNDED_AMOUNT.save(
    //         deps.storage,
    //         pool_address.to_string(),
    //         &total_compounded,
    //     )?;
    //     USER_AUTO_COMPOUND_SUBSCRIPTION_MAP
    //         .save(deps.storage, user_pool_key.to_string(), &false)
    //         .unwrap();
    // }

    //getting reward amounts from the linked distributed tokens of the stakeable token
    for dist_tkn in stakeable_token.distribution.iter() {
        let mut pool_dist_key: String = stakeable_token.token.to_string();
        let mut treasury_reward = TREASURY_REWARDS.load(deps.storage, dist_tkn.token.clone()).unwrap_or(Uint128::zero());
        pool_dist_key.push_str(&dist_tkn.token.to_string());
        let mut user_pool_dist_key = account.clone();
        user_pool_dist_key.push_str(&pool_dist_key.to_string());

        //geting pool reward
        let current_pending_reward = current_pending_rewards.clone().into_iter().find(|reward| reward.0.eq(&dist_tkn.token)).unwrap_or_default();
        let mut current_pending_reward = current_pending_reward.1;
        for reward in pending_rewards.clone().iter() {
            if reward.0.eq(&dist_tkn.token) {
                current_pending_reward -= reward.1;
            }
        };
        res = res
            // .add_attribute("user_pool_dist_key", user_pool_dist_key.clone().to_string())
            .add_attribute("token", dist_tkn.token.clone().to_string())
            .add_attribute("total_reward", current_pending_reward.clone().to_string());

        let mut _current_rewards_weight = Decimal::zero();
        let total_reward_index = total_rewards.clone().into_iter().position(|reward| reward.0.eq(&dist_tkn.token.clone()));
        if let Some(total_reward_index) = total_reward_index {
            total_rewards[total_reward_index].1 += current_pending_reward;
            total_rewards_weight[total_reward_index].1 += Decimal::from_ratio(current_pending_reward, total_staked);
            _current_rewards_weight = total_rewards_weight[total_reward_index].1;
        } else {
            total_rewards.push((dist_tkn.token.clone(), current_pending_reward));
            total_rewards_weight.push((dist_tkn.token.clone(), Decimal::from_ratio(current_pending_reward, total_staked)));
            _current_rewards_weight = Decimal::from_ratio(current_pending_reward, total_staked);
        }

        //getting user reward weight
        let mut user_reward_info = get_user_reward_info_from_map_storage(
            deps.storage,
            user_pool_dist_key.to_string(),
            _current_rewards_weight,
        );

        //calculating reward to be distributed
        let diff_priv_and_curr_reward_weight =
        _current_rewards_weight - user_reward_info.pool_reward_weight;

        let pending_reward = user_reward_info.pending_reward.checked_add(diff_priv_and_curr_reward_weight.checked_mul(Decimal::from_ratio(user_staked, Uint128::one())).unwrap().to_uint_floor()).unwrap();
        let user_pending_reward = pending_reward
            .multiply_ratio(
                config.fee_multiplier - config.treasury_fee,
                config.fee_multiplier
            );
        let fee_amount = pending_reward.checked_sub(user_pending_reward).unwrap_or_default();
        treasury_reward = treasury_reward.checked_add(fee_amount).unwrap();
        res = res
            // .add_attribute("diff_priv_and_curr_reward_weight", diff_priv_and_curr_reward_weight.clone().to_string())
            // .add_attribute("user_staked", user_staked.clone().to_string())
            // .add_attribute("pending_reward", pending_reward.clone().to_string())
            .add_attribute("user_reward", user_pending_reward.clone().to_string());
        if user_pending_reward.gt(&Uint128::zero()) {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                //sending reward to user
                contract_addr: dist_tkn.token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: account.clone(),
                    amount: user_pending_reward,
                })?,
                funds: vec![],
            }));
        }
        TREASURY_REWARDS.save(deps.storage, dist_tkn.token.clone(), &treasury_reward)?;
        if user_staked.gt(&amount) {
            user_reward_info.pending_reward = Uint128::zero();
            user_reward_info.pool_reward_weight = _current_rewards_weight.clone();
            USER_REWARD_INFO_MAP.save(deps.storage, user_pool_dist_key.to_string(), &user_reward_info)?;
        } else {
            USER_REWARD_INFO_MAP.remove(deps.storage, user_pool_dist_key.to_string());
        }
    }

    TOTAL_REWARDS.save(deps.storage, pool_address.clone(), &total_rewards)?;
    TOTAL_REWARDS_WEIGHT.save(deps.storage, pool_address.clone(), &total_rewards_weight)?;
    PENDING_REWARDS.remove(deps.storage, pool_address.clone());

    let mut message = String::from("");

    message.push_str("Unstake and claim");

    total_staked -= amount;

    if total_staked.gt(&Uint128::zero()) {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: pool_address,
            msg: to_binary(&Cw20ExecuteMsg::Send { contract: config.loop_farm_contract.to_string(), amount: total_staked, msg: to_binary(&LoopFarmCw20HookMsg::Stake {})? })?,
            funds: vec![]
        }));
    }

    TOTAL_STAKED.save(
        deps.storage,
        stakeable_token.token.to_string(),
        &total_staked,
    )?;
    Ok(res
        .add_messages(messages))
}

fn handle_claim_reward_reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    // Handle the msg data and save the contract address
    // See: https://github.com/CosmWasm/cw-plus/blob/main/packages/utils/src/parse_reply.rs

    if msg.result.is_err() {
        return Err(StdError::generic_err(
            "no successful response get from staking reply data",
        ));
    }

    let config = CONFIG.load(deps.storage)?;
    let current_claim_reward_info = CURRENT_CLAIM_REWARD_INFO.load(deps.storage)?;
    let pool_address = current_claim_reward_info.pool_address;
    let account = current_claim_reward_info.account;
    let current_pending_rewards = current_claim_reward_info.current_pending_rewards;
    let mut total_rewards = TOTAL_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let mut total_rewards_weight = TOTAL_REWARDS_WEIGHT.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);
    let pending_rewards = PENDING_REWARDS.load(deps.storage, pool_address.clone()).unwrap_or(vec![]);

    // get stakeable token info from loop farm
    let stakeable_token = query_loop_farm_stakable_token(deps.querier, config.loop_farm_contract.clone().to_string(), pool_address.clone())?;

    let asset_infos = pool_address.clone();

    // get total staked before staking
    let total_staked =
        get_total_staked_amount_in_pool_from_map_storage(deps.storage, pool_address.to_string());
    let mut user_pool_key = account.clone();
    user_pool_key.push_str(&pool_address);

    // get user staked before staking
    let user_staked =
        get_user_staked_amount_in_pool_from_map_storage(deps.storage, user_pool_key.to_string());
    
    let mut messages: Vec<CosmosMsg> = vec![];
    let mut res = Response::new();
    res = res
        .add_attribute("action", "Claim rewards");

    for dist_itr in stakeable_token.distribution.iter() {
        let mut pool_dist_key: String = asset_infos.to_string();
        let mut treasury_reward = TREASURY_REWARDS.load(deps.storage, dist_itr.token.clone()).unwrap_or(Uint128::zero());
        pool_dist_key.push_str(&dist_itr.token.to_string());
        let mut user_pool_dist_key = account.to_string();
        user_pool_dist_key.push_str(&pool_dist_key.to_string());

        //geting pool reward
        let current_pending_reward = current_pending_rewards.clone().into_iter().find(|reward| reward.0.eq(&dist_itr.token)).unwrap_or_default();
        let mut current_pending_reward = current_pending_reward.1;
        for reward in pending_rewards.clone().iter() {
            if reward.0.eq(&dist_itr.token) {
                ensure_eq!(current_pending_reward.gt(&reward.1), true, StdError::generic_err("current_pending_reward must be greater than prev reward"));
                current_pending_reward = current_pending_reward.checked_sub(reward.1)?;
            }
        };
        res = res
            // .add_attribute("user_pool_dist_key", user_pool_dist_key.clone().to_string())
            .add_attribute("token", dist_itr.token.clone().to_string())
            .add_attribute("total_reward", current_pending_reward.clone().to_string());

        let mut _current_rewards_weight = Decimal::zero();
        let total_reward_index = total_rewards.clone().into_iter().position(|reward| reward.0.eq(&dist_itr.token.clone()));
        if let Some(total_reward_index) = total_reward_index {
            total_rewards[total_reward_index].1 += current_pending_reward;
            total_rewards_weight[total_reward_index].1 += Decimal::from_ratio(current_pending_reward, total_staked);
            _current_rewards_weight = total_rewards_weight[total_reward_index].1;
        } else {
            total_rewards.push((dist_itr.token.clone(), current_pending_reward));
            total_rewards_weight.push((dist_itr.token.clone(), Decimal::from_ratio(current_pending_reward, total_staked)));
            _current_rewards_weight = Decimal::from_ratio(current_pending_reward, total_staked);
        }
        // res = res
            // .add_attribute("current_rewards_weight", _current_rewards_weight.clone().to_string());

        //getting user reward weight
        let mut user_reward_info = get_user_reward_info_from_map_storage(
            deps.storage,
            user_pool_dist_key.to_string(),
            _current_rewards_weight,
        );

        //calculating reward to be distributed
        let diff_priv_and_curr_reward_weight =
        _current_rewards_weight.checked_sub(user_reward_info.pool_reward_weight).unwrap_or(Decimal::zero());

        let pending_reward = user_reward_info.pending_reward.checked_add(diff_priv_and_curr_reward_weight.checked_mul(Decimal::from_ratio(user_staked, Uint128::one())).unwrap().to_uint_floor())?;
        let user_pending_reward = pending_reward
            .multiply_ratio(
                config.fee_multiplier - config.treasury_fee,
                config.fee_multiplier
            );
        res = res
            // .add_attribute("diff_priv_and_curr_reward_weight", diff_priv_and_curr_reward_weight.clone().to_string())
            // .add_attribute("user_staked", user_staked.clone().to_string())
            // .add_attribute("pending_reward", pending_reward.clone().to_string())
            .add_attribute("user_reward", user_pending_reward.clone().to_string());
        let fee_amount = pending_reward.checked_sub(user_pending_reward).unwrap_or_default();
        treasury_reward = treasury_reward.checked_add(fee_amount).unwrap();

        if user_pending_reward.gt(&Uint128::zero()) {
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                //sending reward to user
                contract_addr: dist_itr.token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: account.to_string(),
                    amount: user_pending_reward,
                })?,
                funds: vec![],
            }));
        }
        user_reward_info.pending_reward = Uint128::zero();
        user_reward_info.pool_reward_weight = _current_rewards_weight;
        TREASURY_REWARDS.save(deps.storage, dist_itr.token.clone(), &treasury_reward)?;
        USER_REWARD_INFO_MAP.save(deps.storage, user_pool_dist_key.to_string(), &user_reward_info)?;
    }

    TOTAL_REWARDS.save(deps.storage, pool_address.clone(), &total_rewards)?;
    TOTAL_REWARDS_WEIGHT.save(deps.storage, pool_address.clone(), &total_rewards_weight)?;
    PENDING_REWARDS.remove(deps.storage, pool_address);
    // USER_AUTO_COMPOUND_SUBSCRIPTION_MAP
    //     .save(deps.storage, user_pool_key.to_string(), &false)
    //     .unwrap();
    
    Ok(res
        .add_messages(messages))
}
