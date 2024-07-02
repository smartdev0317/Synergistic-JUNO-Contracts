use crate::minter::{
    execute_mint, query_balance, query_balance_by_duration, query_minter, query_token_info,
    query_total_balance, update_token_info,
};
use crate::msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg, MigrateMsg};
use crate::state::{
    Config, SynePowerIndex, PoolRewardIndex, RewardInfo, UserInfo, UserRewardResponse, CONFIG, DISTRIBUTION_REWARD, SYNE_POWER_DATE_WISE_MAP, MINT_TIME,
    REWARD_INDEX, TOTAL_REWARD, TOTAL_REWARD_IN_CONTRACT, TOTAL_STAKED_DURATION_WISE,
    USER_REWARD_INFO,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order,
    QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw2::set_contract_version;
use cw20_base::enumerable::query_all_accounts;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_storage_plus::Bound;
const REWARD_CALC_UNIT: Uint128 = Uint128::new(1000000000000u128);

use crate::minter::instantiate_token;

// version info for migration info
const CONTRACT_NAME: &str = concat!("crates.io:", env!("CARGO_CRATE_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//Initialize the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner_addr: info.clone().sender,
        token_addr: deps.api.addr_validate(&msg.token)?,
        community_addr: Some(info.clone().sender),
        last_distributed: env.block.time.seconds(),
        freeze: false,
        freeze_lock_time: msg.freeze_lock_time,
        freeze_start_time: env.block.time.seconds(),
        lock_time_frame: msg.lock_time_frame,
        wait_time_for_distribution_in_seconds: 86400,
        restake_reset_flag: msg.restake_reset_flag,
        syne_power_constant: 13_u128,
        latest_syne_power_date: env.block.time.seconds(),
        day_factor_in_seconds: 86400u64,
        vault_address: msg.vault_address,
        duration_values_vector: vec![1, 3, 6, 12, 24],
        last_syne_power_date: env.block.time.seconds(),
        second_owner: Some(info.clone().sender.to_string()),
        //total_user_days: 0u64,
    };

    CONFIG.save(deps.storage, &config)?;
    //TOTAL_ACTIVE_STAKED.save(deps.storage, &Uint128::zero())?;
    // TOTAL_ACTUAL_STAKED.save(deps.storage, &Uint128::zero())?;
    TOTAL_REWARD.save(deps.storage, &Uint128::zero())?;
    TOTAL_REWARD_IN_CONTRACT.save(deps.storage, &Uint128::zero())?;
    DISTRIBUTION_REWARD.save(deps.storage, &Uint128::zero())?;
    REWARD_INDEX.save(
        deps.storage,
        &PoolRewardIndex {
            pool_reward_index: Uint128::zero(),
        },
    )?;
    //PREVIOUS_DAYS_STAKED_AMOUNT.save(deps.storage, &vec![])?;
    instantiate_token(deps, &env, &info, msg.token_instantiate_msg)?;
    Ok(Response::new())
}

//Execute the handle messages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { owner } => execute_update_config(deps, env, info, owner),
        ExecuteMsg::UpdateSecondOwner { second_owner } => {
            execute_update_second_owner(deps, env, info, second_owner)
        }
        ExecuteMsg::UpdateCommunityAddr { community_addr } => {
            execute_update_community_addr(deps, env, info, community_addr)
        }
        ExecuteMsg::UpdateTokenInfo { name, symbol } => {
            update_token_info(deps, env, info, name, symbol)
        }
        ExecuteMsg::UpdateReward { amount } => execute_update_reward(deps, env, info, amount),
        ExecuteMsg::Distribute {} => execute_distribute(deps, env, info),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateFreezeFlag { freeze_flag } => {
            execute_update_freeze_flag(deps, env, info, freeze_flag)
        }
        ExecuteMsg::UpdateFreezeLockTime { freeze_lock_time } => {
            execute_update_freeze_lock_time(deps, env, info, freeze_lock_time)
        }
        ExecuteMsg::UpdateLockTimeFrame { lock_time_frame } => {
            execute_update_lock_time_frame(deps, env, info, lock_time_frame)
        }
        ExecuteMsg::UpdateWaitTimeForDistribution {
            wait_time_for_distribution_in_seconds,
        } => execute_update_wait_time_for_distributions(
            deps,
            env,
            info,
            wait_time_for_distribution_in_seconds,
        ),
        ExecuteMsg::UpdateRestakeResetFlag { flag } => {
            execute_update_restake_reset_flag(deps, env, info, flag)
        }
        ExecuteMsg::UpdateDayFactorInSeconds {
            day_factor_in_secondsc,
        } => execute_update_day_factor_in_seconds(deps, info, day_factor_in_secondsc),
        ExecuteMsg::Claim { duration } => execute_claim(deps, env, info.sender, duration),
        ExecuteMsg::Restake { duration } => execute_restake(deps, env, info.sender, duration),
        ExecuteMsg::UnstakeAndClaim { duration } => {
            execute_unstake(deps, env, info.sender, duration)
        }
        ExecuteMsg::UpdateSynePowerConstant {
            syne_power_constant,
        } => execute_update_syne_power_constant(deps, info, syne_power_constant.u128()),
        ExecuteMsg::AddNewDuration { duration } => execute_add_new_duration(deps, info, duration),
        ExecuteMsg::DepositInVaultAddress { amount } => {
            execute_deposit_in_vault_address(deps, env, info, amount)
        }
    }
}

pub fn execute_deposit_in_vault_address(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    let callback: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        //sending reward to user
        contract_addr: (&env.contract.address).to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: config.vault_address.to_string(),
            amount: amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(callback)
        .add_attribute("action", "execute_re_deposit"))
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let contract_addr = info.sender.to_string();
    // let contract_addr = cw20_msg.sender.clone();
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Stake { duration } => {
            let config = CONFIG.load(deps.storage)?;
            //only asset contract can execute this message
            let mut authorized: bool = false;

            if config.token_addr == contract_addr {
                authorized = true;
            }

            if !authorized {
                return Err(StdError::generic_err("unauthorized"));
            }

            execute_stake(
                deps,
                env,
                Addr::unchecked(cw20_msg.sender),
                cw20_msg.amount,
                duration,
            )
        }
        Cw20HookMsg::Deposit {} => {
            let config = CONFIG.load(deps.storage)?;
            //only asset contract can execute this message
            let mut authorized: bool = false;

            if config.token_addr == contract_addr {
                authorized = true;
            }

            if !authorized {
                return Err(StdError::generic_err("unauthorized"));
            }
            execute_deposit(deps, env, cw20_msg.amount)
        }
    }
}

// Only owner can execute it.
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: String,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.owner_addr = deps.api.addr_validate(&owner)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

// Only owner can execute it.
pub fn execute_update_second_owner(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    second_owner: String,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.second_owner = Some(deps.api.addr_validate(&second_owner)?.to_string());
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_second_owner"))
}
// Only owner can execute it.
pub fn execute_update_community_addr(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    community_addr: String,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.community_addr = Some(deps.api.addr_validate(&community_addr)?);
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_community_addr"))
}

// Only owner can execute it.
pub fn execute_update_freeze_flag(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    freeze_flag: bool,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.freeze = freeze_flag;
    if freeze_flag {
        config.freeze_start_time = env.block.time.seconds();
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_freeze_flag"))
}

// Update syne power constant.
pub fn execute_update_syne_power_constant(
    deps: DepsMut,
    info: MessageInfo,
    syne_power_constant: u128,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.syne_power_constant = syne_power_constant;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_syne_power_constant"))
}

// Only owner can execute it.
pub fn execute_update_freeze_lock_time(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    freeze_lock_time: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.freeze_lock_time = freeze_lock_time;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_freeze_lock_time"))
}

// Only owner can execute it.
pub fn execute_update_lock_time_frame(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    lock_time_frame: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.lock_time_frame = lock_time_frame;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_lock_time_frame"))
}

// Only owner can execute it.
pub fn execute_add_new_duration(
    deps: DepsMut,
    info: MessageInfo,
    duration: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    if !config.duration_values_vector.contains(&duration) {
        config.duration_values_vector.push(duration);
    }
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "execute_add_new_duration"))
}

pub fn execute_update_wait_time_for_distributions(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    wait_time_for_distribution_in_seconds: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    config.wait_time_for_distribution_in_seconds = wait_time_for_distribution_in_seconds;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_wait_time_for_distributions"))
}

// Only owner can execute it.
pub fn execute_update_restake_reset_flag(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    flag: bool,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // permission check
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }

    config.restake_reset_flag = flag;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_restake_reset_flag"))
}

// Allow admin to deposit reward tokens.
pub fn execute_deposit(deps: DepsMut, _env: Env, amount: Uint128) -> StdResult<Response> {
    let mut total_reward_in_contract = TOTAL_REWARD_IN_CONTRACT.load(deps.storage)?;
    total_reward_in_contract += amount;
    TOTAL_REWARD_IN_CONTRACT.save(deps.storage, &total_reward_in_contract)?;

    Ok(Response::new().add_attributes(vec![("action", "deposited")]))
}

// Allow users to stake the tokens.
pub fn execute_stake(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    amount: Uint128,
    duration: u64,
) -> StdResult<Response> {
    let current_reward_index = REWARD_INDEX.load(deps.storage)?;
    let key: String = sender.to_string();
    let mut config = CONFIG.load(deps.storage)?;

    let previous_days =
        (env.block.time.seconds() - config.latest_syne_power_date) / config.day_factor_in_seconds;

    if config.latest_syne_power_date + config.day_factor_in_seconds <= env.block.time.seconds() {
        config.latest_syne_power_date =
            config.latest_syne_power_date + config.day_factor_in_seconds * previous_days;
    }
    if !config.duration_values_vector.contains(&duration) {
        return Err(StdError::generic_err(
            "Invalid duration parameter is passed",
        ));
    }
    //reward token issued to be used during evaluation of user share in queries
    let user_info = MINT_TIME
        .load(deps.storage, (&sender, duration))
        .unwrap_or(UserInfo {
            balance: Uint128::zero(),
            actual_balance: Uint128::zero(),
            last_claimed_time: 0u64,
            mint_time: 0u64,
        });

    //getting user reward index
    let mut user_reward_info = if let Some(user_reward_info) =
        USER_REWARD_INFO.may_load(deps.storage, (key.clone(), duration.clone()))?
    {
        user_reward_info
    } else {
        RewardInfo {
            reward_index: current_reward_index.pool_reward_index,
            pending_reward: Uint128::zero(),
        }
    };
    let mut user_reward_response = UserRewardResponse {
        user_reward: Uint128::zero(),
        calculated_days_of_reward: 0u64,
        pending_reward: Uint128::zero(),
        // start_time: 0u64,
        // end_time: 0u64,
        // last_claimed_time: 0u64,
        // initial_start_time: 0u64,
        // mint_time: 0u64,
        // latest_syne_power_date: 0u64,
    };

    if user_info
        .actual_balance
        .multiply_ratio(1u128, duration as u128)
        > Uint128::zero()
    {
        //calculating reward to be distributed
        user_reward_response = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&QueryMsg::QueryUserReward {
                wallet: sender.to_string(),
                duration: duration.clone(),
            })?,
        }))?;
        // user_reward_response = query_user_reward(deps.as_ref(), env.clone(), sender.to_string(), duration.clone())?;
    }

    TOTAL_STAKED_DURATION_WISE.update(
        deps.storage,
        duration.clone(),
        |d: Option<Uint128>| -> StdResult<Uint128> {
            match d {
                Some(total_staked) => Ok(total_staked + amount),
                None => Ok(amount),
            }
        },
    )?;
    //calculting pending reward
    user_reward_info.pending_reward += user_reward_response.user_reward;
    user_reward_info.reward_index = current_reward_index.pool_reward_index;

    //saving user reward info
    USER_REWARD_INFO.save(
        deps.storage,
        (key.clone(), duration.clone()),
        &user_reward_info,
    )?;

    // let amount_to_stake = (user_info.actual_balance - user_info.balance) + amount;

    CONFIG.save(deps.storage, &config)?;
    execute_mint(deps, env, sender.to_string(), amount, duration)?;
    Ok(Response::new().add_attributes(vec![
        ("action", "staked"),
        ("sender", &sender.to_string()),
        ("amount", &amount.to_string()),
        (
            "claimed days",
            &user_reward_response.calculated_days_of_reward.to_string(),
        ),
        (
            "pending reward",
            &user_reward_info.pending_reward.to_string(),
        ),
        // ("start_time", &user_reward_response.start_time.to_string()),
        // ("end_time", &user_reward_response.end_time.to_string()),
        // (
        //     "initial_start_time",
        //     &user_reward_response.initial_start_time.to_string(),
        // ),
        // ("staked_time", &user_reward_response.mint_time.to_string()),
        // (
        //     "latest_syne_power_date",
        //     &user_reward_response.latest_syne_power_date.to_string(),
        // ),
        // (
        //     "last_claimed_time",
        //     &user_reward_response.last_claimed_time.to_string(),
        // ),
    ]))
}

// Allow users to stake the tokens.
pub fn execute_restake(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    duration: u64,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let current_reward_index = REWARD_INDEX.load(deps.storage)?;
    let key: String = sender.to_string();

    //getting user reward index
    let mut user_reward_info = if let Some(user_reward_info) =
        USER_REWARD_INFO.may_load(deps.storage, (key.clone(), duration.clone()))?
    {
        user_reward_info
    } else {
        RewardInfo {
            reward_index: current_reward_index.pool_reward_index,
            pending_reward: Uint128::zero(),
        }
    };
    //reward token issued to be used during evaluation of user share in queries

    let previous_days =
        (env.block.time.seconds() - config.latest_syne_power_date) / config.day_factor_in_seconds;

    if config.latest_syne_power_date + config.day_factor_in_seconds <= env.block.time.seconds() {
        config.latest_syne_power_date =
            config.latest_syne_power_date + config.day_factor_in_seconds * previous_days;
    }

    let user_reward_response: UserRewardResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&QueryMsg::QueryUserReward {
                wallet: sender.to_string(),
                duration: duration.clone(),
            })?,
        }))?;
    // let user_reward_response = query_user_reward(deps.as_ref(), env.clone(), sender.to_string(), duration.clone())?;
    user_reward_info.reward_index = current_reward_index.pool_reward_index;
    let reward_to_add = user_reward_response.user_reward + user_reward_info.pending_reward;
    if reward_to_add.is_zero() {
        return Err(StdError::generic_err("None reward for restaking."));
    }

    TOTAL_REWARD.update(deps.storage, |mut reward| -> StdResult<_> {
        println!("total_user_reward{} reward{}", reward_to_add, reward);
        reward -= reward_to_add;
        Ok(reward)
    })?;
    TOTAL_REWARD_IN_CONTRACT.update(deps.storage, |mut reward| -> StdResult<_> {
        reward -= reward_to_add;
        Ok(reward)
    })?;

    println!("reward_to_add {}", reward_to_add);

    // TOTAL_ACTIVE_STAKED.save(deps.storage, &total_staked)?;

    TOTAL_STAKED_DURATION_WISE.update(
        deps.storage,
        duration.clone(),
        |d: Option<Uint128>| -> StdResult<Uint128> {
            match d {
                Some(total_staked) => Ok(total_staked + reward_to_add),
                None => Ok(reward_to_add),
            }
        },
    )?;

    // let user_info = MINT_TIME
    //     .may_load(deps.storage, (&sender, duration))?
    //     .unwrap_or_default();

    // let total_restake_amount = (user_info.actual_balance - user_info.balance) + reward_to_add;

    //saving user reward info
    user_reward_info.pending_reward = Uint128::zero();

    USER_REWARD_INFO.save(deps.storage, (key.clone(), duration), &user_reward_info)?;

    execute_mint(deps, env, sender.to_string(), reward_to_add, duration)?;
    Ok(Response::new().add_attributes(vec![
        ("action", "restaked"),
        ("sender", &sender.to_string()),
        ("reward to add", &reward_to_add.to_string()),
        (
            "claimed days",
            &user_reward_response.calculated_days_of_reward.to_string(),
        ),
        (
            "pending reward",
            &user_reward_info.pending_reward.to_string(),
        ),
        // ("start_time", &user_reward_response.start_time.to_string()),
        // ("end_time", &user_reward_response.end_time.to_string()),
        // (
        //     "initial_start_time",
        //     &user_reward_response.initial_start_time.to_string(),
        // ),
        // ("staked_time", &user_reward_response.mint_time.to_string()),
        // (
        //     "latest_syne_power_date",
        //     &user_reward_response.latest_syne_power_date.to_string(),
        // ),
        // (
        //     "last_claimed_time",
        //     &user_reward_response.last_claimed_time.to_string(),
        // ),
    ]))
}

// comment below codes to reference when implement convertNFT feature
// Allow users to unstake tokens from staking contract.
pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    receiver: Addr,
    duration: u64,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    if config.clone().freeze
        && env.block.time.seconds()
            < config.clone().freeze_start_time + config.clone().freeze_lock_time
    {
        return Err(StdError::generic_err(
            "Sorry for inconvenience, system is under maintenance. Kindly check again later",
        ));
    }

    let key: String = receiver.to_string();

    let user_info =
        if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&receiver, duration))? {
            user_info
        } else {
            return Err(StdError::generic_err("No staked amount found"));
        };

    if env.block.time.seconds() - user_info.mint_time < config.clone().lock_time_frame * duration {
        return Err(StdError::generic_err(
            "The rewards are still locked. Please wait patiently for the specified time",
        ));
    }

    let contract_addr = config.clone().token_addr;
    //let mut total_staked = TOTAL_STAKED.load(deps.storage)?;
    let mut total_reward = TOTAL_REWARD.load(deps.storage)?;
    let current_reward_index = REWARD_INDEX.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    //sending user staked back to the user
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: receiver.to_string(),
            amount: user_info
                .actual_balance
                .multiply_ratio(1u128, duration.clone() as u128),
        })?,
        funds: vec![],
    }));

    //calcultaing reward and unclaiming----------------------------------------
    let mut user_reward_info = if let Some(user_reward_info) =
        USER_REWARD_INFO.may_load(deps.storage, (key.clone(), duration.clone()))?
    {
        user_reward_info
    } else {
        RewardInfo {
            reward_index: current_reward_index.pool_reward_index,
            pending_reward: Uint128::zero(),
        }
    };
    let user_reward_response: UserRewardResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&QueryMsg::QueryUserReward {
                wallet: receiver.to_string(),
                duration: duration.clone(),
            })?,
        }))?;
    // let user_reward_response = query_user_reward(deps.as_ref(), env.clone(), receiver.to_string(), duration.clone())?;

    let reward_to_be_dist = user_reward_info.pending_reward + user_reward_response.user_reward;

    user_reward_info.pending_reward = Uint128::zero();
    user_reward_info.reward_index = current_reward_index.pool_reward_index;

    println!(
        "reciever {}, reward to be dist{} total rewards {}",
        receiver.to_string(),
        reward_to_be_dist,
        total_reward
    );

    total_reward -= reward_to_be_dist;
    TOTAL_REWARD.save(deps.storage, &total_reward.clone())?;

    let mut total_reward_in_contract = TOTAL_REWARD_IN_CONTRACT.load(deps.storage)?;
    total_reward_in_contract -= reward_to_be_dist; // this will revert tx if actual reward amount in contract is less than withdrawal reward amount
    TOTAL_REWARD_IN_CONTRACT.save(deps.storage, &total_reward_in_contract)?;
    USER_REWARD_INFO.remove(deps.storage, (key.clone(), duration.clone()));

    TOTAL_STAKED_DURATION_WISE.update(
        deps.storage,
        duration.clone(),
        |d: Option<Uint128>| -> StdResult<Uint128> {
            match d {
                Some(total_staked) => Ok(total_staked
                    - user_info
                        .actual_balance
                        .multiply_ratio(1u128, duration.clone() as u128)),
                None => Ok(Uint128::zero()),
            }
        },
    )?;
    if !reward_to_be_dist.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: (&contract_addr).to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: receiver.to_string(),
                amount: reward_to_be_dist,
            })?,
            funds: vec![],
        }));
    }
    MINT_TIME.remove(deps.storage, (&receiver, duration));
    // USER_STAKED_TIME.remove(deps.storage, key);
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "unstake")
        .add_attributes(vec![
            ("claimed_reward", &reward_to_be_dist.to_string()),
            (
                "claimed days",
                &user_reward_response.calculated_days_of_reward.to_string(),
            ),
            (
                "pending reward",
                &user_reward_info.pending_reward.to_string(),
            ),
            // ("start_time", &user_reward_response.start_time.to_string()),
            // ("end_time", &user_reward_response.end_time.to_string()),
            // (
            //     "initial_start_time",
            //     &user_reward_response.initial_start_time.to_string(),
            // ),
            // ("staked_time", &user_reward_response.mint_time.to_string()),
            // (
            //     "latest_syne_power_date",
            //     &user_reward_response.latest_syne_power_date.to_string(),
            // ),
            // (
            //     "last_claimed_time",
            //     &user_reward_response.last_claimed_time.to_string(),
            // ),
        ]))
}

//Allow users to claim tokens from staking contract.
pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    receiver: Addr,
    duration: u64,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    if config.clone().freeze
        && env.block.time.seconds()
            < config.clone().freeze_start_time + config.clone().freeze_lock_time
    {
        return Err(StdError::generic_err(
            "Sorry for inconvenience, system is under maintenance. Kindly check again later",
        ));
    }

    let key: String = receiver.to_string();

    let mut user_info =
        if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&receiver, duration))? {
            user_info
        } else {
            return Err(StdError::generic_err("No Staked Amount found"));
        };

    let contract_addr = config.clone().token_addr;
    let current_reward_index = REWARD_INDEX.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    //calcultaing reward and unclaiming----------------------------------------
    let mut user_reward_info = if let Some(user_reward_info) =
        USER_REWARD_INFO.may_load(deps.storage, (key.clone(), duration.clone()))?
    {
        user_reward_info
    } else {
        RewardInfo {
            reward_index: current_reward_index.pool_reward_index,
            pending_reward: Uint128::zero(),
        }
    };

    // getting user reward difference from it's last stake to current pool index

    let user_reward_response: UserRewardResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&QueryMsg::QueryUserReward {
                wallet: receiver.to_string(),
                duration: duration.clone(),
            })?,
        }))?;
    // let mut user_reward_response = query_user_reward(deps.as_ref(), env.clone(), receiver.to_string(), duration.clone())?;

    let reward_to_be_dist = user_reward_response.user_reward + user_reward_info.pending_reward;
    user_reward_info.reward_index = current_reward_index.pool_reward_index;
    user_reward_info.pending_reward = Uint128::zero();
    USER_REWARD_INFO.save(deps.storage, (key, duration.clone()), &user_reward_info)?;
    //adjusting values of calculating reward in pool and contract
    let mut total_reward = TOTAL_REWARD.load(deps.storage)?;
    println!("total_reward {}", total_reward);
    total_reward -= reward_to_be_dist;
    TOTAL_REWARD.save(deps.storage, &total_reward.clone())?;

    let mut total_reward_in_contract = TOTAL_REWARD_IN_CONTRACT.load(deps.storage)?;
    println!("total_reward_in_contract {}", total_reward_in_contract);
    total_reward_in_contract -= reward_to_be_dist; // this will revert tx if actual reward amount in contract is less than withdrawal reward amount

    TOTAL_REWARD_IN_CONTRACT.save(deps.storage, &total_reward_in_contract)?;
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        //sending reward to user
        contract_addr: (&contract_addr).to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: receiver.to_string(),
            amount: reward_to_be_dist,
        })?,
        funds: vec![],
    }));
    user_info.last_claimed_time = env.block.time.seconds();
    MINT_TIME.save(deps.storage, (&receiver, duration), &user_info)?;
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "claim")
        .add_attribute("claimed reward", reward_to_be_dist)
        .add_attributes(vec![
            ("actaul reward", &user_reward_response.user_reward.to_string()),
            (
                "claimed days",
                &user_reward_response.calculated_days_of_reward.to_string(),
            ),
            (
                "pending reward",
                &user_reward_info.pending_reward.to_string(),
            ),
            // ("start_time", &user_reward_response.start_time.to_string()),
            // ("end_time", &user_reward_response.end_time.to_string()),
            // (
            //     "initial_start_time",
            //     &user_reward_response.initial_start_time.to_string(),
            // ),
            // ("staked_time", &user_reward_response.mint_time.to_string()),
            // (
            //     "latest_syne_power_date",
            //     &user_reward_response.latest_syne_power_date.to_string(),
            // ),
            // (
            //     "last_claimed_time",
            //     &user_reward_response.last_claimed_time.to_string(),
            // ),
        ]))
}

//This will distribute the reward.
pub fn execute_distribute(deps: DepsMut, env: Env, _info: MessageInfo) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    //let mut total_staked = TOTAL_ACTIVE_STAKED.load(deps.storage)?;

    let distribution_reward = DISTRIBUTION_REWARD.load(deps.storage)?;
    let total_reward_in_contract = TOTAL_REWARD_IN_CONTRACT.load(deps.storage)?;
    let mut total_reward = TOTAL_REWARD.load(deps.storage)?;
    let mut reward_index = REWARD_INDEX.load(deps.storage)?;
    let non_distributed_days = (env.block.time.seconds() - config.last_distributed)
        / config.wait_time_for_distribution_in_seconds;
    let last_distributed = config.last_distributed
        + config.wait_time_for_distribution_in_seconds * non_distributed_days;
    // distribution_reward = distribution_reward.multiply_ratio(non_distributed_days, 1u64);
    if non_distributed_days <= 0 {
        return Err(StdError::generic_err(
            "please wait patiently for the specified time",
        ));
    }
    let previous_days =
        (env.block.time.seconds() - config.latest_syne_power_date) / config.day_factor_in_seconds;

    let mut day_start_time = config.last_distributed;
    // let mut total_lopo = Uint128::zero();

    for _ in 0..non_distributed_days {
        /*
        Checks for time last distirbution was called,
        add 25920000 to the time its number of sec in 1 day.
        Save the reward in the storage.
        */
        let mut total_lopo = Uint128::zero();
        for i in &config.duration_values_vector {
            total_lopo += query_total_balance(deps.as_ref(), day_start_time, *i)?.balance;
        }

        if distribution_reward != Uint128::zero() {
            //checking if contract has sufficient funds to allow admin to distribute

            if distribution_reward <= (total_reward_in_contract - total_reward) {
                let old_syne_power_reward_index = reward_index.pool_reward_index.clone();
                if !total_lopo.is_zero() {
                    reward_index.pool_reward_index +=
                        REWARD_CALC_UNIT.multiply_ratio(distribution_reward, total_lopo);
                    println!(
                        "reward_index.pool_reward_index {} old_syne_power_reward_index {} total_staked {}, day_start_time {} ",
                        reward_index.pool_reward_index, old_syne_power_reward_index, total_lopo, day_start_time,
                    );
                    total_reward += distribution_reward;
                }
                // println!(" day_start_time {} last_syne_power_date {}", day_start_time, last_syne_power_date);
                // PREVIOUS_DAYS_STAKED_AMOUNT.save(deps.storage, &previous_days_staked_amount)?;
                if day_start_time + config.wait_time_for_distribution_in_seconds
                    >= config.last_syne_power_date + config.day_factor_in_seconds
                {
                    println!(
                        " day_start_time {} last_syne_power_date {}",
                        day_start_time, config.last_syne_power_date
                    );
                    if let Some(mut old_syne_power_index) = SYNE_POWER_DATE_WISE_MAP
                        .may_load(deps.storage, config.last_syne_power_date)?
                    {
                        old_syne_power_index.last_reward_index = old_syne_power_reward_index;

                        SYNE_POWER_DATE_WISE_MAP.save(
                            deps.storage,
                            config.last_syne_power_date,
                            &old_syne_power_index,
                        )?;
                    }
                    config.last_syne_power_date += config.day_factor_in_seconds;
                }
                println!("last_syne_power_date {}", config.last_syne_power_date);
                let syne_power_index = if let Some(syne_power_index) =
                    SYNE_POWER_DATE_WISE_MAP.may_load(deps.storage, config.last_syne_power_date)?
                {
                    syne_power_index
                } else {
                    let mut syne_power_index = SynePowerIndex {
                        first_reward_index: Uint128::zero(),
                        last_reward_index: Uint128::zero(),
                    };
                    syne_power_index.first_reward_index = reward_index.pool_reward_index;
                    syne_power_index
                };
                day_start_time += config.wait_time_for_distribution_in_seconds;
                SYNE_POWER_DATE_WISE_MAP.save(
                    deps.storage,
                    config.last_syne_power_date,
                    &syne_power_index,
                )?;

                //TOTAL_ACTIVE_STAKED.save(deps.storage, &total_staked)?;
                REWARD_INDEX.save(deps.storage, &reward_index)?;
                TOTAL_REWARD.save(deps.storage, &total_reward)?;
            } else {
                return Err(StdError::generic_err("insufficient funds"));
            }

            if config.latest_syne_power_date + config.day_factor_in_seconds
                <= env.block.time.seconds()
            {
                config.latest_syne_power_date =
                    config.latest_syne_power_date + config.day_factor_in_seconds * previous_days;
            }
        }
    }

    // let previous_days =
    //     (env.block.time.seconds() - config.creation_timestamp) / config.day_factor_in_seconds;

    // if config.creation_timestamp + config.day_factor_in_seconds <= env.block.time.seconds() {
    //     config.creation_timestamp =
    //         config.creation_timestamp + config.day_factor_in_seconds * previous_days;
    // }

    config.last_distributed = last_distributed;
    println!(
        "config.last_distributed {} config.creation_timestamp {}",
        config.last_distributed, config.latest_syne_power_date
    );
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("last_distributed", last_distributed.to_string())
        .add_attribute("reward_index", reward_index.pool_reward_index))
}

//Allow admin to update and set the daily reward. If admin do not set any reward it
// will continue the same tokens.
pub fn execute_update_reward(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr
        && (config.second_owner.clone().unwrap().is_empty()
            || (!config.second_owner.clone().unwrap().is_empty()
                && info.sender != config.clone().second_owner.unwrap()))
    {
        return Err(StdError::generic_err("unauthorized"));
    }
    DISTRIBUTION_REWARD.save(deps.storage, &amount)?;

    Ok(Response::new().add_attribute("action", "update reward"))
}

pub fn execute_update_day_factor_in_seconds(
    deps: DepsMut,
    info: MessageInfo,
    day_factor_in_seconds: u64,
) -> StdResult<Response> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    if info.sender != config.owner_addr {
        return Err(StdError::generic_err("unauthorized"));
    }
    config.day_factor_in_seconds = day_factor_in_seconds;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_day_factor_in_seconds"))
}

// pub fn calculate_reward(
//     store: &mut dyn Storage,
//     wallet: String,
//     config: &Config,
//     current_time: u64,
//     duration: u64,
//     address: &Addr,
// ) -> StdResult<UserRewardResponse> {
//     let current_reward_index = REWARD_INDEX.load(store)?;

//     //getting user reward index

//     let user_info = if let Some(user_info) = MINT_TIME.may_load(store, (&address, duration))? {
//         user_info
//     } else {
//         return Err(StdError::generic_err("No Staked Found"));
//     };

//     let user_end_time =
//         (user_info.mint_time + config.lock_time_frame * duration) - config.day_factor_in_seconds;
//     let day_passed =
//         (config.creation_timestamp - user_info.mint_time) / config.day_factor_in_seconds;
//     let claimed_days =
//         (user_info.last_claimed_time - user_info.mint_time) / config.day_factor_in_seconds;

//     let mut start_time = config.creation_timestamp - config.day_factor_in_seconds * day_passed;
//     if user_info.mint_time < start_time {
//         start_time -= config.day_factor_in_seconds;
//     }

//     let end_time = start_time + config.lock_time_frame * duration;
//     let start_time = claimed_days * config.day_factor_in_seconds + start_time;
//     let remainder_time = user_info.last_claimed_time % config.day_factor_in_seconds;

//     let start_bound = Some(Bound::inclusive(start_time));
//     let end_bound = Some(Bound::inclusive(end_time));

//     let syne_power_date_wise_map: StdResult<Vec<_>> = SYNE_POWER_DATE_WISE_MAP
//         .range(store, start_bound, end_bound, Order::Ascending)
//         .collect();

//     let user_reward_info =
//         if let Some(user_reward_info) = USER_REWARD_INFO.may_load(store, wallet.to_string())? {
//             user_reward_info
//         } else {
//             RewardInfo {
//                 reward_index: current_reward_index.pool_reward_index,
//                 pending_reward: Uint128::zero(),
//             }
//         };
//     let mut user_reward_index = user_reward_info.reward_index;
//     let mut user_reward_without_power = Uint128::zero();
//     let mut user_reward = Uint128::zero();
//     if let Ok(syne_power_date_wise_vec) = syne_power_date_wise_map {
//         if syne_power_date_wise_vec.is_empty() {
//             return Ok(UserRewardResponse {
//                 user_reward: Uint128::zero(),
//                 // reward_without_power: Uint128::zero(),
//             });
//         }
//         let mut c = 0;

//         for i in syne_power_date_wise_vec.iter() {
//             let mut user_diff_priv_and_curr_reward_index;

//             if i.1.last_reward_index.is_zero() {
//                 user_diff_priv_and_curr_reward_index =
//                     current_reward_index.pool_reward_index - user_reward_index;
//             } else {
//                 user_diff_priv_and_curr_reward_index = i.1.last_reward_index - user_reward_index;
//             }

//             user_reward_index = i.1.last_reward_index;

//             let mut user_resp = Uint128::zero();
//             if i.0 + remainder_time < user_end_time {
//                 user_resp = user_diff_priv_and_curr_reward_index
//                     .multiply_ratio(user_info.balance.clone(), Uint128::new(1u128));
//                 user_resp = user_resp.multiply_ratio(
//                     user_end_time as u128 - (i.0 as u128 + remainder_time as u128),
//                     config.lock_time_frame as u128 * duration as u128,
//                 );

//             } else {
//                 continue;
//             }
//             let user_reward_to_be_dist = user_diff_priv_and_curr_reward_index
//                 .multiply_ratio(user_info.balance.clone(), Uint128::new(1u128));
//             println!(
//                 "c {} user_reward_without_power {} user_reward_to_be_dist {} user_reward {} user_diff_priv_and_curr_reward_index{} " ,
//                 c, user_reward_without_power, user_reward_to_be_dist, user_resp, user_diff_priv_and_curr_reward_index,
//             );
//             c += 1;

//             user_reward_without_power += user_reward_to_be_dist;
//             user_reward += user_resp;
//             // SYNE_POWER_DATE_WISE_MAP.save(store, i.0.clone(), &i.1)?;
//         }
//     }

//     let user_reward_power_response = UserRewardResponse {
//         user_reward: user_reward.multiply_ratio(Uint128::from(1u128), REWARD_CALC_UNIT),
//         //total_power,
//         // reward_without_power: user_reward_without_power
//         //     .multiply_ratio(Uint128::from(1u128), REWARD_CALC_UNIT),
//         // user_power,
//     };

//     Ok(user_reward_power_response)
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryTotalReward {} => to_binary(&query_total_reward(deps, env)?),
        QueryMsg::QueryTotalRewardInContract {} => {
            to_binary(&query_total_reward_in_contract(deps, env)?)
        }
        QueryMsg::QueryStakedByUser { wallet, duration } => {
            to_binary(&query_staked_by_user(deps, env, wallet, duration)?)
        }
        QueryMsg::QueryTotalDailyReward {} => to_binary(&query_total_daily_reward(deps, env)?),
        QueryMsg::QueryUserReward { wallet, duration } => {
            to_binary(&query_user_reward(deps, env, wallet, duration)?)
        }
        QueryMsg::QueryUserStakedTime { wallet, duration } => {
            to_binary(&query_user_staked_time(deps, wallet, duration)?)
        }
        QueryMsg::QueryDistributionWaitTime {} => to_binary(&query_distribution_wait_time(deps)?),
        QueryMsg::QueryFreezeLockTime {} => to_binary(&query_freeze_lock_time(deps)?),
        QueryMsg::QueryLockTimeFrame {} => to_binary(&query_lock_time_frame(deps)?),
        QueryMsg::QueryLastDistributionTime {} => to_binary(&query_last_distribution_time(deps)?),
        QueryMsg::QueryConfig {} => to_binary(&query_config(deps)?),

        // QueryMsg::QuerySyneDateWiseMap { wallet, duration } => {
        //     to_binary(&query_syne_date_wise_map(deps, wallet, duration)?)
        // }
        // QueryMsg::QueryUserRewardInfo { wallet, duration } => {
        //     to_binary(&query_user_reward_info(deps, wallet, duration)?)
        // }
        // QueryMsg::QueryRewardIndex {} => to_binary(&query_reward_index(deps)?),
        // QueryMsg::QueryMintTime { wallet, duration } => {
        //     to_binary(&query_mint_time(deps, wallet, duration)?)
        // }
        QueryMsg::QueryTotalStakedByDuration { duration } => {
            to_binary(&query_total_staked_by_duration(deps, duration)?)
        }
        // QueryMsg::QueryTotalPower {} => to_binary(&query_total_power(deps)?),
        // QueryMsg::QueryVaultAmount {} => to_binary(&query_vault_amount(deps)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, env, address)?),

        QueryMsg::BalanceByDuration { address, duration } => {
            to_binary(&query_balance_by_duration(deps, env, address, duration)?)
        }

        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::QueryCommunityAddr {} => to_binary(&query_community_addr(deps)?),
        QueryMsg::TotalBalance { duration } => to_binary(&query_total_balance(
            deps,
            env.block.time.seconds(),
            duration,
        )?),
    }
}

pub fn query_total_staked_by_duration(deps: Deps, duration: u64) -> StdResult<Uint128> {
    TOTAL_STAKED_DURATION_WISE.load(deps.storage, duration)
}

pub fn query_total_reward(deps: Deps, _env: Env) -> StdResult<Uint128> {
    TOTAL_REWARD.load(deps.storage)
}

pub fn query_total_reward_in_contract(deps: Deps, _env: Env) -> StdResult<Uint128> {
    TOTAL_REWARD_IN_CONTRACT.load(deps.storage)
}

// Tells us about the staked value of pool by user.
pub fn query_staked_by_user(
    deps: Deps,
    _env: Env,
    wallet: String,
    duration: u64,
) -> StdResult<Uint128> {
    let user_info = MINT_TIME
        .load(deps.storage, (&deps.api.addr_validate(&wallet)?, duration))
        .unwrap_or(UserInfo {
            balance: Uint128::zero(),
            actual_balance: Uint128::zero(),
            last_claimed_time: 0u64,
            mint_time: 0u64,
        });
    Ok(user_info
        .actual_balance
        .multiply_ratio(1u128, duration.clone() as u128))
}

//Informs us about distribution reward
pub fn query_total_daily_reward(deps: Deps, _env: Env) -> StdResult<Uint128> {
    let distribution_reward = DISTRIBUTION_REWARD
        .load(deps.storage)
        .unwrap_or_else(|_| Uint128::zero());
    Ok(distribution_reward)
}

// Tell reward of users of the staking pool.
pub fn query_user_reward(
    deps: Deps,
    _env: Env,
    wallet: String,
    duration: u64,
) -> StdResult<UserRewardResponse> {
    let current_reward_index = REWARD_INDEX.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    let address = deps.api.addr_validate(&wallet)?;
    //getting user reward index
    let user_info =
        if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
            user_info
        } else {
            return Err(StdError::generic_err("No Staked Found"));
        };

    let user_end_time = user_info.mint_time + config.lock_time_frame * duration;

    println!(
        "latest_syne_power_date {} mint time {} user ent {} duration {} config.lock_time_fram {}",
        config.latest_syne_power_date,
        user_info.mint_time,
        user_end_time,
        duration,
        config.lock_time_frame
    );

    let mut days = 0u64;
    let mut day_passed = 0u64;
    if config.latest_syne_power_date > user_info.mint_time {
        day_passed =
            (config.latest_syne_power_date - user_info.mint_time) / config.day_factor_in_seconds;
    }

    let claimed_days =
        (user_info.last_claimed_time - user_info.mint_time) / config.day_factor_in_seconds;

    let mut start_time = config.latest_syne_power_date - config.day_factor_in_seconds * day_passed;
    // let initial_start_time = start_time.clone();
    if user_info.mint_time < start_time {
        start_time -= config.day_factor_in_seconds;
    }
    println!(
        "user_info.last_claimed_time {}  user_info.mint_time {} start_time {} config.creation_timestamp {}, day_passed {}, claimed_days {}",
        user_info.last_claimed_time, user_info.mint_time, start_time, config.latest_syne_power_date, day_passed, claimed_days
    );
    let end_time = start_time + config.lock_time_frame * duration;
    let start_time = claimed_days * config.day_factor_in_seconds + start_time;

    println!(
        "user_info.last_claimed_time {}  user_info.mint_time {} start_time {} end_time {}",
        user_info.last_claimed_time, user_info.mint_time, start_time, end_time
    );
    println!("end_time {:?}, start_time {}", end_time, start_time);
    let start_bound = Some(Bound::inclusive(start_time.clone()));
    let end_bound = Some(Bound::inclusive(end_time.clone()));
    if user_info.last_claimed_time >= config.last_distributed {
        return Ok(UserRewardResponse {
            user_reward: Uint128::zero(),
            calculated_days_of_reward: 0u64,
            pending_reward: Uint128::zero(),
            // start_time,
            // end_time,
            // last_claimed_time: user_info.last_claimed_time,
            // mint_time: user_info.mint_time,
            // initial_start_time,
            // latest_syne_power_date: config.latest_syne_power_date,
        });
    }
    let remainder_time = user_info.last_claimed_time % config.day_factor_in_seconds;

    let user_reward_info = if let Some(user_reward_info) =
        USER_REWARD_INFO.may_load(deps.storage, (wallet.to_string(), duration.clone()))?
    {
        user_reward_info
    } else {
        RewardInfo {
            reward_index: current_reward_index.pool_reward_index,
            pending_reward: Uint128::zero(),
        }
    };
    let mut user_reward_index = user_reward_info.reward_index;

    let mut user_reward_without_power = Uint128::zero();
    let mut user_reward = Uint128::zero();
    let syne_power_date_wise_map: StdResult<Vec<_>> = SYNE_POWER_DATE_WISE_MAP
        .range(deps.storage, start_bound, end_bound, Order::Ascending)
        .collect();

    if let Ok(syne_power_date_wise_vec) = syne_power_date_wise_map {
        println!("syne_power_date_wise_vec {:?}", syne_power_date_wise_vec);
        if syne_power_date_wise_vec.is_empty() {
            return Ok(UserRewardResponse {
                user_reward: Uint128::zero(),
                calculated_days_of_reward: 0u64,
                pending_reward: Uint128::zero(),
                // start_time,
                // end_time,
                // last_claimed_time: user_info.last_claimed_time,
                // mint_time: user_info.mint_time,
                // initial_start_time,
                // latest_syne_power_date: config.latest_syne_power_date,
            });
        }
        let mut c = 0;
        for i in syne_power_date_wise_vec {
            let mut user_diff_priv_and_curr_reward_index = Uint128::zero();
            if i.1.last_reward_index.is_zero() {
                println!(
                    "user_reward_index {}, pool_reward_index {}",
                    user_reward_index, current_reward_index.pool_reward_index,
                );
                user_diff_priv_and_curr_reward_index =
                    current_reward_index.pool_reward_index - user_reward_index;
            } else {
                println!(
                    "user_reward_index {}, i.1.last_reward_index {}",
                    user_reward_index, i.1.last_reward_index
                );
                if i.1.last_reward_index > user_reward_index {
                    user_diff_priv_and_curr_reward_index =
                        i.1.last_reward_index - user_reward_index;
                }
            }
            user_reward_index = i.1.last_reward_index;
            let mut user_resp;

            println!(
                "i.0 {} user end time{}, total days {}",
                i.0,
                user_end_time,
                (config.lock_time_frame as u128 * duration as u128),
            );
            if i.0 + remainder_time < user_end_time {
                user_resp = user_diff_priv_and_curr_reward_index
                    .multiply_ratio(user_info.actual_balance.clone(), Uint128::new(1u128));
                user_resp = user_resp.multiply_ratio(
                    user_end_time as u128 - (i.0 as u128 + remainder_time as u128),
                    config.lock_time_frame as u128 * duration as u128,
                );
            } else {
                continue;
            }
            let user_reward_to_be_dist = user_diff_priv_and_curr_reward_index
                .multiply_ratio(user_info.actual_balance.clone(), Uint128::new(1u128));

            println!(
                "c {} user_reward_without_power {} user_reward_to_be_dist {} user_reward {}, user_diff_priv_and_curr_reward_index {} i.0 {} remaining day {}, total days {}, user_info.actual_balance.clone() {}",
                c, user_reward_without_power, user_reward_to_be_dist, user_resp, user_diff_priv_and_curr_reward_index, i.0, (user_end_time as u128 - (i.0 as u128 + remainder_time as u128))
               , (config.lock_time_frame as u128 * duration as u128 ), user_info.actual_balance.clone(),
            );
            user_reward_without_power += user_reward_to_be_dist;
            c += 1;
            days += 1;

            user_reward += user_resp;
        }
    }

    let user_reward_power_response = UserRewardResponse {
        user_reward: user_reward.multiply_ratio(Uint128::from(1u128), REWARD_CALC_UNIT),
        calculated_days_of_reward: days,
        pending_reward: user_reward_info.pending_reward,
        // start_time,
        // end_time,
        // last_claimed_time: user_info.last_claimed_time,
        // mint_time: user_info.mint_time,
        // initial_start_time,
        // latest_syne_power_date: 0u64,
        // reward_without_power: user_reward_without_power
        //     .multiply_ratio(Uint128::from(1u128), REWARD_CALC_UNIT),
    };
    println!("user_reward {}, ", user_reward);
    Ok(user_reward_power_response)
}

//query to get user staked time
pub fn query_user_staked_time(deps: Deps, wallet: String, duration: u64) -> StdResult<u64> {
    let address = deps.api.addr_validate(&wallet)?;
    if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
        Ok(user_info.mint_time)
    } else {
        Ok(0u64)
    }
}
//////////
pub fn query_community_addr(deps: Deps) -> StdResult<Addr> {
    Ok(CONFIG.load(deps.storage)?.community_addr.unwrap())
}
// Query to get Syne Datewise map
// pub fn query_syne_date_wise_map(
//     deps: Deps,
//     wallet: String,
//     duration: u64,
// ) -> StdResult<Vec<(u64, SynePowerIndex)>> {
//     let config = CONFIG.load(deps.storage)?;
//     let address = deps.api.addr_validate(&wallet)?;
//     let user_info =
//         if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
//             user_info
//         } else {
//             return Err(StdError::generic_err("No Staked Found"));
//         };

//     // let user_end_time = user_info.mint_time + config.lock_time_frame * duration;

//     // println!(
//     //     "latest_syne_power_date {} mint time {} user ent {} duration {} config.lock_time_fram {}",
//     //     config.latest_syne_power_date,
//     //     user_info.mint_time,
//     //     user_end_time,
//     //     duration,
//     //     config.lock_time_frame
//     // );

//     // let mut days = 0u64;
//     let mut day_passed = 0u64;
//     if config.latest_syne_power_date > user_info.mint_time {
//         day_passed =
//             (config.latest_syne_power_date - user_info.mint_time) / config.day_factor_in_seconds;
//     }

//     let claimed_days =
//         (user_info.last_claimed_time - user_info.mint_time) / config.day_factor_in_seconds;

//     let mut start_time = config.latest_syne_power_date - config.day_factor_in_seconds * day_passed;
//     if user_info.mint_time < start_time {
//         start_time -= config.day_factor_in_seconds;
//     }
//     // println!(
//     //     "user_info.last_claimed_time {}  user_info.mint_time {} start_time {} config.creation_timestamp {}, day_passed {}",
//     //     user_info.last_claimed_time, user_info.mint_time, start_time, config.latest_syne_power_date, day_passed
//     // );
//     let end_time = start_time + config.lock_time_frame * duration;
//     let start_time = claimed_days * config.day_factor_in_seconds + start_time;
//     let start_bound = Some(Bound::inclusive(start_time));
//     let end_bound = Some(Bound::inclusive(end_time));
//     let syne_power_date_wise_map: StdResult<Vec<_>> = SYNE_POWER_DATE_WISE_MAP
//         .range(deps.storage, start_bound, end_bound, Order::Ascending)
//         .collect();
//     // let result_map = Some(syne_power_date_wise_map);
//     return syne_power_date_wise_map;
// }
// Query to get User reward info detail
// pub fn query_user_reward_info(deps: Deps, wallet: String, duration: u64) -> StdResult<RewardInfo> {
//     let address = deps.api.addr_validate(&wallet)?.to_string();
//     Ok(USER_REWARD_INFO.load(deps.storage, (address, duration))?)
// }
////// Query to get  reward index detail
// pub fn query_reward_index(deps: Deps) -> StdResult<PoolRewardIndex> {
//     Ok(REWARD_INDEX.load(deps.storage)?)
// }
//// Query to get  mint time map details
// pub fn query_mint_time(deps: Deps, wallet: String, duration: u64) -> StdResult<UserInfo> {
//     let address = deps.api.addr_validate(&wallet)?;
//     if let Some(user_info) = MINT_TIME.may_load(deps.storage, (&address, duration))? {
//         Ok(user_info)
//     } else {
//         Ok(UserInfo {
//             balance: Uint128::from(0u128),
//             actual_balance: Uint128::from(0u128),
//             mint_time: 0u64,
//             last_claimed_time: 0u64,
//         })
//     }
// }

//query freeze lock time
pub fn query_freeze_lock_time(deps: Deps) -> StdResult<u64> {
    Ok(CONFIG.load(deps.storage)?.freeze_lock_time)
}

//query lock time frame
pub fn query_lock_time_frame(deps: Deps) -> StdResult<u64> {
    Ok(CONFIG.load(deps.storage)?.lock_time_frame)
}

//query distribution wait time frame
pub fn query_distribution_wait_time(deps: Deps) -> StdResult<u64> {
    Ok(CONFIG
        .load(deps.storage)?
        .wait_time_for_distribution_in_seconds)
}

pub fn query_last_distribution_time(deps: Deps) -> StdResult<u64> {
    Ok(CONFIG.load(deps.storage)?.last_distributed)
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}