use std::str::FromStr;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_slice, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, WasmMsg, ensure_eq, CosmosMsg, Storage,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use cw2::set_contract_version;
use cw_utils::{ensure_from_older_version, maybe_addr};

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, MigrateMsg, QueryMsg, Cw20HookMsg, WithdrawMsg, AdminResponse, ConfigResponse, TotalStakedResponse, RewardResponse, StakedResponse,
};
use crate::queriers::query_loop_protocol_staking_rewards;
use crate::state::{
    Config, TokenInfo, ADMIN, CONFIG, STAKE, TOTAL_STAKED, REWARD_ACTION, RewardAction,
};

use synedao::bloop_vault::InstantiateMsg;

// version info for migration info
const CONTRACT_NAME: &str = concat!("crates.io:", env!("CARGO_CRATE_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Note, you can use StdResult in some functions where you do not
// make use of the custom errors
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let api = deps.api;
    // Set the admin if provided
    ADMIN.set(deps.branch(), Some(api.addr_validate(&msg.admin.clone())?))?;

    // min_bond is at least 1, so 0 stake -> non-membership
    let min_bond = std::cmp::max(msg.min_bond, Uint128::new(1));

    TOTAL_STAKED.save(deps.storage, &TokenInfo::default())?;

    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        token: deps.api.addr_validate(&msg.token)?,
        bloop_converter_and_staker: info.sender,
        min_bond,
        loop_protocol_staking: deps.api.addr_validate(&msg.loop_protocol_staking)?,
        treasury_wallet: None,
        treasury_withdrawer: None,
        syne_staking_reward_distributor: None,
        treasury_fee: Decimal::from_str("0.2").unwrap(),
        syne_staking_fee: Decimal::zero(),
        total_fee_cap: Decimal::from_str("0.3").unwrap(),
        treasury_fee_limit: Decimal::from_str("0.05").unwrap(),
        duration: 12u64,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

// And declare a custom Error variant for the ones where you will want to make use of it
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let api = deps.api;
    match msg {
        ExecuteMsg::UpdateAdmin { admin } => {
            Ok(ADMIN.execute_update_admin(deps, info, maybe_addr(api, admin)?)?)
        }
        ExecuteMsg::UpdateConfig { 
            min_bond, 
            treasury_wallet, 
            treasury_withdrawer, 
            syne_staking_reward_distributor, 
            treasury_fee, 
            syne_staking_fee 
        } => execute_update_config(
            deps, 
            env, 
            info, 
            min_bond, 
            treasury_wallet, 
            treasury_withdrawer, 
            syne_staking_reward_distributor, 
            treasury_fee, 
            syne_staking_fee
        ),
        ExecuteMsg::Receive(msg) => execute_receive_cw20(deps, env, info, msg),
        ExecuteMsg::WithdrawRewards { address } => execute_withdraw_rewards(deps, env, info, address),
        ExecuteMsg::WithdrawTreasuryRewards { amount } => execute_withdraw_treasury_rewards(deps, env, info, amount),
        ExecuteMsg::WithdrawSyneStakingRewards { amount } => execute_withdraw_syne_staking_rewards(deps, env, info, amount),
        ExecuteMsg::Unstake { amount } => execute_unstake(deps, info, amount),
        ExecuteMsg::DistributeRewards { address } => execute_distribute_user_rewards(deps, env, info, address),
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    min_bond: Option<Uint128>,
    treasury_wallet: Option<String>,
    treasury_withdrawer: Option<String>,
    syne_staking_reward_distributor: Option<String>,
    treasury_fee: Option<Decimal>,
    syne_staking_fee: Option<Decimal>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender, cfg.admin, ContractError::Unauthorized {});

    let mut valid = false;

    let mut res = Response::new()
        .add_attribute("action", "update_config");

    if let Some(min_bond) = min_bond {
        cfg.min_bond = min_bond;
        valid = true;
        res = res.add_attribute("min_bond", &cfg.min_bond.to_string());
    }

    if let Some(treasury_wallet) = treasury_wallet {
        cfg.treasury_wallet = Some(deps.api.addr_validate(&treasury_wallet)?);
        valid = true;
        res = res.add_attribute("treasury_wallet", &treasury_wallet.to_string());
    }

    if let Some(treasury_withdrawer) = treasury_withdrawer {
        cfg.treasury_withdrawer = Some(deps.api.addr_validate(&treasury_withdrawer)?);
        valid = true;
        res = res.add_attribute("treasury_withdrawer", &treasury_withdrawer.to_string());
    }

    if let Some(syne_staking_reward_distributor) = syne_staking_reward_distributor {
        cfg.syne_staking_reward_distributor = Some(deps.api.addr_validate(&syne_staking_reward_distributor)?);
        valid = true;
        res = res.add_attribute("syne_staking_reward_distributor", &syne_staking_reward_distributor.to_string());
    }

    if let Some(treasury_fee) = treasury_fee {
        ensure_eq!(treasury_fee.gt(&cfg.treasury_fee_limit), true, ContractError::InvalidFee {});
        cfg.treasury_fee = treasury_fee;
        valid = true;
        res = res.add_attribute("treasury_fee", &treasury_fee.to_string());
    }

    if let Some(syne_staking_fee) = syne_staking_fee {
        cfg.syne_staking_fee = syne_staking_fee;
        valid = true;
        res = res.add_attribute("syne_staking_fee", &syne_staking_fee.to_string());
    }

    ensure_eq!(cfg.treasury_fee.checked_add(cfg.syne_staking_fee).unwrap().le(&cfg.total_fee_cap), true, ContractError::InvalidFee {});

    ensure_eq!(valid, true, ContractError::NoData {});

    Ok(res)
}

pub fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // info.sender is the address of the cw20 contract (that re-sent this message).
    // wrapper.sender is the address of the user that requested the cw20 contract to send this.
    // This cannot be fully trusted (the cw20 contract can fake it), so only use it for actions
    // in the address's favor (like paying/bonding tokens, not withdrawls)
    let cfg = CONFIG.load(deps.storage)?;

    // check token address is correct
    ensure_eq!(info.sender, cfg.token, ContractError::InvalidAsset {});

    let msg: Cw20HookMsg = from_slice(&wrapper.msg)?;
    match msg {
        Cw20HookMsg::Stake {} => execute_stake(deps, env, info, wrapper),
        Cw20HookMsg::DistributeRewards { address } => execute_distribute_rewards(deps, env, info, address, wrapper),
    }
}

pub fn execute_stake(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;
    let api = deps.api;

    // user's address and staking amount
    let amount = msg.amount;
    let sender = msg.sender;

    // check if loop reward is existing
    let user_reward_reponse = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_reponse.user_reward.checked_add(user_reward_reponse.pending_reward).unwrap();
    if withdrawable_amount.gt(&Uint128::zero()) {
        let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
        match reward_action {
            RewardAction::NoAction {} => {
                reward_action = RewardAction::Stake { address: deps.api.addr_validate(&sender)?, amount };
        
                let messages = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.bloop_converter_and_staker.clone().to_string(),
                        msg: to_binary(&WithdrawMsg::WithdrawRewards {})?,
                        funds: vec![],
                    }),
                ];
            
                REWARD_ACTION.save(storage, &reward_action)?;
            
                Ok(Response::new().add_messages(messages))
            },
            _ => {Err(ContractError::InvalidAction {})}
        }
    } else {
        let reward_amount = add_stake(storage, api.addr_validate(&sender)?, amount)?;
        let mut res: Response = Response::new()
            .add_attribute("Stake", amount.to_string())
            .add_attribute("From", sender.clone());
        if reward_amount.gt(&Uint128::zero()) {
            let msgs = vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: sender, amount: reward_amount })?,
                    funds: vec![],
                }),
            ];
            res = res.add_messages(msgs);
        }
        Ok(res)
    }
}

pub fn execute_withdraw_rewards(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;
    let api = deps.api;

    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();
    if withdrawable_amount.gt(&Uint128::zero()) {
        let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
        match reward_action {
            RewardAction::NoAction {} => {
                reward_action = RewardAction::Reward { address: deps.api.addr_validate(&address)? };
        
                let messages = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.bloop_converter_and_staker.clone().to_string(),
                        msg: to_binary(&WithdrawMsg::WithdrawRewards {})?,
                        funds: vec![],
                    }),
                ];
        
                REWARD_ACTION.save(storage, &reward_action)?;
        
                Ok(Response::new().add_messages(messages))
            },
            _ => {Err(ContractError::InvalidAction {})}
        }
    } else {
        let reward_amount = update_rewards(storage, api.addr_validate(&address)?)?;
        let res = Response::new()
            .add_attribute("Send", reward_amount.to_string())
            .add_attribute("To", &address.to_string())
            .add_message(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount: reward_amount })?,
                    funds: vec![],
                }),
            );
        Ok(res)
    }
}

pub fn execute_withdraw_treasury_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;

    ensure_eq!(info.sender, cfg.treasury_withdrawer.clone().unwrap(), ContractError::Unauthorized {});
    
    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking.clone(), cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();

    CONFIG.save(storage, &cfg.clone())?;
    
    if withdrawable_amount.gt(&Uint128::zero()) {    
        let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
        match reward_action {
            RewardAction::NoAction {} => {
                reward_action = RewardAction::TreasuryWithdraw { amount };
        
                let messages = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.bloop_converter_and_staker.clone().to_string(),
                        msg: to_binary(&WithdrawMsg::WithdrawRewards {})?,
                        funds: vec![],
                    }),
                ];
        
                REWARD_ACTION.save(storage, &reward_action)?;
        
                Ok(Response::new().add_messages(messages))
            },
            _ => {Err(ContractError::InvalidAction {})}
        }
    } else {
        let treasury_amount = update_treasury_amounts(storage, info, amount)?;
        ensure_eq!((treasury_amount.gt(&Uint128::zero())), true, ContractError::NoWithdrawable {});

        let res = Response::new()
            .add_attribute("Send", treasury_amount.to_string())
            .add_attribute("To", &cfg.treasury_wallet.clone().unwrap().to_string())
            .add_message(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: cfg.treasury_wallet.clone().unwrap().to_string(), amount: treasury_amount })?,
                    funds: vec![],
                }),
            );
        Ok(res)
    }
}

pub fn execute_withdraw_syne_staking_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;
    
    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();
    
    if withdrawable_amount.gt(&Uint128::zero()) {    
        let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
        match reward_action {
            RewardAction::NoAction {} => {
                reward_action = RewardAction::SyneStakingRewardWithdraw { amount };
        
                let messages = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.bloop_converter_and_staker.clone().to_string(),
                        msg: to_binary(&WithdrawMsg::WithdrawRewards {})?,
                        funds: vec![],
                    }),
                ];
        
                REWARD_ACTION.save(storage, &reward_action)?;
        
                Ok(Response::new().add_messages(messages))
            },
            _ => {Err(ContractError::InvalidAction {})}
        }
    } else {
        let syne_staking_reward_amount = update_syne_staking_amounts(storage, info, amount)?;
        ensure_eq!((syne_staking_reward_amount.gt(&Uint128::zero())), true, ContractError::NoWithdrawable {});

        if let Some(syne_staking_reward_distributor) = cfg.syne_staking_reward_distributor {
            let res = Response::new()
                .add_attribute("Send", syne_staking_reward_amount.to_string())
                .add_attribute("To", &syne_staking_reward_distributor.to_string())
                .add_message(
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.token.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: syne_staking_reward_distributor.to_string(), amount: syne_staking_reward_amount })?,
                        funds: vec![],
                    }),
                );
            Ok(res)
        } else {
            Err(ContractError::InvalidDistributor {})
        }
    }
}

pub fn execute_distribute_user_rewards(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    address: Addr,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;

    let mut res = Response::new();
    res = res
        .add_attribute("Action", "send_rewards");

    let reward_amount = update_rewards(storage, address.clone())?;
    if reward_amount.gt(&Uint128::zero()) {
        res = res
            .add_attribute("Send", reward_amount.to_string())
            .add_attribute("To", &address.to_string())
            .add_message(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount: reward_amount })?,
                    funds: vec![],
                }),
            );
    }
    Ok(res)
}

pub fn execute_distribute_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Option<Addr>,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;

    let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
    let mut res = Response::new();
    res = res
        .add_attribute("Action", "distribute_rewards")
        .add_attribute("Amount", wrapper.amount);
    update_total_rewards(storage, wrapper.amount)?;
    if let Some(address) = address {
        reward_action = RewardAction::Reward { address };
    }
    match reward_action {
        RewardAction::NoAction {} => {},
        RewardAction::Reward { address } => {
            let reward_amount = update_rewards(storage, address.clone())?;
            if reward_amount.gt(&Uint128::zero()) {
                res = res
                    .add_attribute("Send", reward_amount.to_string())
                    .add_attribute("To", &address.to_string())
                    .add_message(
                        CosmosMsg::Wasm(WasmMsg::Execute {
                            //sending reward to user
                            contract_addr: cfg.token.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount: reward_amount })?,
                            funds: vec![],
                        }),
                    );
            }
        },
        RewardAction::Stake { address, amount } => {
            let reward_amount = add_stake(storage, address.clone(), amount)?;
            res = res.add_attribute("Stake", amount.to_string())
                .add_attribute("From", address.clone().to_string());
            if reward_amount.gt(&Uint128::zero()) {
                let msgs = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.token.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount: reward_amount })?,
                        funds: vec![],
                    }),
                ];
                res = res.add_messages(msgs);
            }
        },
        RewardAction::TreasuryWithdraw { amount } => {
            let treasury_amount = update_treasury_amounts(storage, info, amount)?;
            res = res
                .add_attribute("Send", treasury_amount.to_string())
                .add_attribute("To", &cfg.treasury_wallet.clone().unwrap().to_string())
                .add_message(
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.token.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: cfg.treasury_wallet.clone().unwrap().to_string(), amount: treasury_amount })?,
                        funds: vec![],
                    }),
                );
        },
        RewardAction::SyneStakingRewardWithdraw { amount } => {
            let syne_staking_amount = update_syne_staking_amounts(storage, info, amount)?;
            if let Some(syne_staking_reward_distributor) = cfg.syne_staking_reward_distributor {
                res = res
                    .add_attribute("Send", syne_staking_amount.to_string())
                    .add_attribute("To", &syne_staking_reward_distributor.to_string())
                    .add_message(
                        CosmosMsg::Wasm(WasmMsg::Execute {
                            //sending reward to user
                            contract_addr: cfg.token.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: syne_staking_reward_distributor.to_string(), amount: syne_staking_amount })?,
                            funds: vec![],
                        }),
                    );
            } else {
                return Err(ContractError::InvalidDistributor {});
            }
        },
        RewardAction::Unstake { address, amount } => {
            let pending_rewards = remove_stake(storage, address.clone(), amount)?;
            let mut msgs = vec![
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount })?,
                    funds: vec![],
                }),
            ];
            res = res
                .add_attribute("Unstake", amount.to_string())
                .add_attribute("To", address.to_string());

            if pending_rewards.gt(&Uint128::zero()) {
                msgs.push(
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.token.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: address.to_string(), amount: pending_rewards })?,
                        funds: vec![],
                    }),
                );
                res = res
                    .add_attribute("Reward", pending_rewards.to_string())
                    .add_attribute("To", address.to_string())
            }
            res = res.add_messages(msgs);

        }
    }
    reward_action = RewardAction::NoAction {};
    REWARD_ACTION.save(storage, &reward_action)?;
    Ok(res)
}

pub fn update_total_rewards(
    storage: &mut dyn Storage,
    reward: Uint128,
) -> Result<(), ContractError> {
    let mut total_stake = TOTAL_STAKED.load(storage)?;
    let cfg = CONFIG.load(storage)?;
    let treasury_rewards = Decimal::from_atomics(reward, 0).unwrap().checked_mul(cfg.treasury_fee).unwrap().to_uint_floor();
    let syne_staking_rewards = Decimal::from_atomics(reward, 0).unwrap().checked_mul(cfg.syne_staking_fee).unwrap().to_uint_floor();
    total_stake.pending_treasury_rewards = total_stake.pending_treasury_rewards.checked_add(
        treasury_rewards
    ).unwrap();
    total_stake.pending_syne_staking_rewards = total_stake.pending_syne_staking_rewards.checked_add(syne_staking_rewards).unwrap();
    if total_stake.staked.gt(&Uint128::zero()) {
        total_stake.power = total_stake.power.checked_add(
            Decimal::from_atomics(
                reward.checked_sub(treasury_rewards).unwrap().checked_sub(syne_staking_rewards).unwrap(), 0
            ).unwrap().checked_div(Decimal::from_atomics(total_stake.staked, 0).unwrap()).unwrap()
        ).unwrap();
    }

    TOTAL_STAKED.save(storage,&total_stake)?;

    Ok(())
}

pub fn update_rewards(
    storage: &mut dyn Storage,
    address: Addr,
) -> Result<Uint128, ContractError> {
    let total_stake = TOTAL_STAKED.load(storage)?;
    let mut pending_rewards = Uint128::zero();
    STAKE.update(
        storage,
        &address,
        |staking_info| -> StdResult<_> {
            let mut staking_info = staking_info.unwrap_or_default();
            pending_rewards = (total_stake.power
                .checked_sub(staking_info.power_diff)
                .unwrap_or_default()
            ).checked_mul(Decimal::from_atomics(staking_info.stake, 0).unwrap()).unwrap_or_default()
            .to_uint_floor();
            staking_info.power_diff = total_stake.power;
            Ok(staking_info)
        },
    )?;
    Ok(pending_rewards)
}

pub fn add_stake(
    storage: &mut dyn Storage,
    address: Addr,
    amount: Uint128,
) -> Result<Uint128, ContractError> {
    let mut total_stake = TOTAL_STAKED.load(storage)?;
    let mut pending_rewards = Uint128::zero();
    STAKE.update(
        storage,
        &address,
        |staking_info| -> StdResult<_> {
            let mut staking_info = staking_info.unwrap_or_default();
            pending_rewards = (total_stake.power
                .checked_sub(staking_info.power_diff)
                .unwrap_or_default()
            ).checked_mul(Decimal::from_atomics(staking_info.stake, 0).unwrap()).unwrap_or_default()
            .to_uint_floor();
            staking_info.stake = staking_info.stake.checked_add(amount).unwrap_or_default();
            staking_info.power_diff = total_stake.power;
            Ok(staking_info)
        }
    )?;
    total_stake.staked = total_stake.staked.checked_add(amount).unwrap_or_default();
    TOTAL_STAKED.save(storage, &total_stake)?;
    Ok(pending_rewards)
}

pub fn remove_stake(
    storage: &mut dyn Storage,
    address: Addr,
    amount: Uint128,
) -> Result<Uint128, ContractError> {
    let mut total_stake = TOTAL_STAKED.load(storage)?;
    let mut pending_rewards = Uint128::zero();
    STAKE.update(
        storage,
        &address,
        |staking_info| -> StdResult<_> {
            let mut staking_info = staking_info.unwrap_or_default();
            pending_rewards = (total_stake.power
                .checked_sub(staking_info.power_diff)
                .unwrap_or_default()
            ).checked_mul(Decimal::from_atomics(staking_info.stake, 0).unwrap()).unwrap_or_default()
            .to_uint_floor();
            staking_info.stake = staking_info.stake.checked_sub(amount).unwrap_or_default();
            staking_info.power_diff = total_stake.power;
            Ok(staking_info)
        }
    )?;
    total_stake.staked = total_stake.staked.checked_sub(amount).unwrap_or_default();
    TOTAL_STAKED.save(storage, &total_stake)?;
    Ok(pending_rewards)
}

pub fn update_treasury_amounts(
    storage: &mut dyn Storage,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Uint128, ContractError> {
    let cfg = CONFIG.load(storage)?;
    ensure_eq!(info.sender, cfg.bloop_converter_and_staker.clone(), ContractError::Unauthorized {});
    
    let mut total_staked = TOTAL_STAKED.load(storage)?;
    let amount = amount.unwrap_or(total_staked.pending_treasury_rewards);
    ensure_eq!((amount.le(&total_staked.pending_treasury_rewards)), true, ContractError::InvalidAmount {});

    total_staked.pending_treasury_rewards = total_staked.pending_treasury_rewards.checked_sub(amount).unwrap();
    TOTAL_STAKED.save(storage, &total_staked)?;

    Ok(amount)
}

pub fn update_syne_staking_amounts(
    storage: &mut dyn Storage,
    info: MessageInfo,
    amount: Option<Uint128>,
) -> Result<Uint128, ContractError> {
    let cfg = CONFIG.load(storage)?;
    ensure_eq!(info.sender, cfg.bloop_converter_and_staker.clone(), ContractError::Unauthorized {});
    
    let mut total_staked = TOTAL_STAKED.load(storage)?;
    let amount = amount.unwrap_or(total_staked.pending_syne_staking_rewards);
    ensure_eq!((amount.le(&total_staked.pending_syne_staking_rewards)), true, ContractError::InvalidAmount {});

    total_staked.pending_syne_staking_rewards = total_staked.pending_syne_staking_rewards.checked_sub(amount).unwrap();
    TOTAL_STAKED.save(storage, &total_staked)?;

    Ok(amount)
}

pub fn execute_unstake(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128
) -> Result<Response, ContractError> {
    let stake = STAKE.may_load(deps.storage, &info.sender)?.unwrap_or_default();
    ensure_eq!((amount.gt(&Uint128::zero())), true, ContractError::InvalidAmount {});
    ensure_eq!((amount.le(&stake.stake)), true, ContractError::InvalidAmount {});

    let cfg = CONFIG.load(deps.storage)?;
    let storage = deps.storage;
    
    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();
    
    if withdrawable_amount.gt(&Uint128::zero()) {
        let mut reward_action = REWARD_ACTION.load(storage).unwrap_or(RewardAction::NoAction {});
        match reward_action {
            RewardAction::NoAction {} => {
                reward_action = RewardAction::Unstake { address: info.sender, amount };
            
                let messages = vec![
                    CosmosMsg::Wasm(WasmMsg::Execute {
                        //sending reward to user
                        contract_addr: cfg.bloop_converter_and_staker.clone().to_string(),
                        msg: to_binary(&WithdrawMsg::WithdrawRewards {})?,
                        funds: vec![],
                    }),
                ];
            
                REWARD_ACTION.save(storage, &reward_action)?;
            
                Ok(Response::new().add_messages(messages))
            },
            _ => {Err(ContractError::InvalidAction {})}
        }
    } else {
        let pending_rewards = remove_stake(storage, info.sender.clone(), amount)?;
        let mut msgs = vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                //sending reward to user
                contract_addr: cfg.token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: info.sender.to_string(), amount })?,
                funds: vec![],
            }),
        ];
        let mut res = Response::new()
            .add_attribute("Unstake", amount.to_string())
            .add_attribute("To", info.sender.to_string());

        if pending_rewards.gt(&Uint128::zero()) {
            msgs.push(
                CosmosMsg::Wasm(WasmMsg::Execute {
                    //sending reward to user
                    contract_addr: cfg.token.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer { recipient: info.sender.to_string(), amount: pending_rewards })?,
                    funds: vec![],
                }),
            );
            res = res
                .add_attribute("Reward", pending_rewards.to_string())
                .add_attribute("To", info.sender.to_string())
        }
        res = res.add_messages(msgs);
        Ok(res)
    }

}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Admin {} => {
            to_binary(&query_admin(deps)?)
        },
        QueryMsg::Config {} => {
            to_binary(&query_config(deps)?)
        },
        QueryMsg::TotalStake {} => {
            to_binary(&query_total_staked(deps)?)
        },
        QueryMsg::Stake { address } => {
            to_binary(&query_staked(deps, address)?)
        },
        QueryMsg::Reward { address } => {
            to_binary(&query_reward(deps, address)?)
        },
        QueryMsg::TotalPendingReward {} => {
            to_binary(&staker_pending_reward(deps)?)
        },
        QueryMsg::TreasuryReward {} => {
            to_binary(&query_treasury_reward(deps)?)
        },
        QueryMsg::SyneStakingReward {} => {
            to_binary(&query_syne_staking_reward(deps)?)
        }
    }
}

pub fn query_admin(
    deps: Deps
) -> StdResult<AdminResponse> {
    let cfg = CONFIG.load(deps.storage).unwrap();
    Ok(AdminResponse { admin: cfg.admin })
}

pub fn query_config(
    deps: Deps
) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage).unwrap();
    Ok(ConfigResponse {
        token: cfg.token,
        bloop_converter_and_staker: cfg.bloop_converter_and_staker.clone(),
        loop_protocol_staking: cfg.loop_protocol_staking,
        min_bond: cfg.min_bond,
        treasury_wallet: cfg.treasury_wallet,
        treasury_withdrawer: cfg.treasury_withdrawer,
        syne_staking_reward_distributor: cfg.syne_staking_reward_distributor,
        treasury_fee: cfg.treasury_fee,
        syne_staking_fee: cfg.syne_staking_fee,
        duration: cfg.duration,
    })
}

pub fn query_total_staked(
    deps: Deps
) -> StdResult<TotalStakedResponse> {
    let total_staked = TOTAL_STAKED.load(deps.storage).unwrap();
    Ok(TotalStakedResponse { total_staked: total_staked.staked, power: total_staked.power, pending_treasury_rewards: total_staked.pending_treasury_rewards })
}

pub fn query_staked(
    deps: Deps,
    address: String
) -> StdResult<StakedResponse> {
    let staked = STAKE.may_load(deps.storage, &deps.api.addr_validate(&address).unwrap())?.unwrap_or_default();
    Ok(StakedResponse { stake: staked.stake, power_diff: staked.power_diff })
}

pub fn staker_pending_reward(
    deps: Deps,
) -> StdResult<RewardResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let staker_pending_reward = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();
    Ok(RewardResponse { rewards: staker_pending_reward })
}

pub fn query_reward(
    deps: Deps,
    address: String
) -> StdResult<RewardResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let total_stake = TOTAL_STAKED.load(deps.storage)?;

    if total_stake.staked.eq(&Uint128::zero()) {
        return Ok(RewardResponse { rewards: Uint128::zero() });
    }
    let stake = STAKE.may_load(deps.storage, &deps.api.addr_validate(&address).unwrap())?.unwrap_or_default();

    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking, cfg.bloop_converter_and_staker.clone())?;
    let staker_pending_reward = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();
    let rewards = (total_stake.power.checked_sub(stake.power_diff).unwrap())
        .checked_mul(Decimal::from_atomics(stake.stake, 0).unwrap()).unwrap().to_uint_floor()
        .checked_add(
            Decimal::from_atomics(staker_pending_reward, 0).unwrap()
                .checked_mul(Decimal::one().checked_sub(cfg.treasury_fee).unwrap().checked_sub(cfg.syne_staking_fee).unwrap()).unwrap()
                .checked_mul(Decimal::from_atomics(stake.stake, 0).unwrap()).unwrap()
                .checked_div(Decimal::from_atomics(total_stake.staked, 0).unwrap()).unwrap()
                .to_uint_floor()
        )?;
    Ok(RewardResponse { rewards })
}

pub fn query_treasury_reward(
    deps: Deps,
) -> StdResult<RewardResponse> {
    let storage = deps.storage.clone();
    let cfg = CONFIG.load(storage)?;

    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking.clone(), cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();

    let total_staked = TOTAL_STAKED.load(storage)?;
    let pending_treasury_rewards = total_staked.pending_treasury_rewards.checked_add(Decimal::from_atomics(withdrawable_amount, 0).unwrap().checked_mul(cfg.treasury_fee).unwrap().to_uint_floor()).unwrap();

    Ok(RewardResponse { rewards: pending_treasury_rewards })
}

pub fn query_syne_staking_reward(
    deps: Deps,
) -> StdResult<RewardResponse> {
    let storage = deps.storage.clone();
    let cfg = CONFIG.load(storage)?;

    let user_reward_response = query_loop_protocol_staking_rewards(deps.querier, cfg.loop_protocol_staking.clone(), cfg.bloop_converter_and_staker.clone())?;
    let withdrawable_amount = user_reward_response.user_reward.checked_add(user_reward_response.pending_reward).unwrap();

    let total_staked = TOTAL_STAKED.load(storage)?;
    let pending_syne_staking_rewards = total_staked.pending_syne_staking_rewards.checked_add(Decimal::from_atomics(withdrawable_amount, 0).unwrap().checked_mul(cfg.syne_staking_fee).unwrap().to_uint_floor()).unwrap();

    Ok(RewardResponse { rewards: pending_syne_staking_rewards })
}

/// Manages the contract migration.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new())
}
