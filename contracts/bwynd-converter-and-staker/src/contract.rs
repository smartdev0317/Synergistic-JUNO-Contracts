use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, MigrateMsg, Cw20HookMsg, VaultCw20HookMsg, ReceiveDelegationMsg, VaultExecuteMsg, GaugeConfigResponse};
use crate::queriers::query_wynd_dao_core_rewards;
use crate::state::{
    Config, CONFIG, GaugeConfig, GAUGE_CONFIG, Vote
};
use crate::error::ContractError;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, CosmosMsg, WasmMsg, from_binary, to_binary, ensure_eq, SubMsg, ReplyOn, Reply, StdError, Addr, 
};
use cw2::set_contract_version;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use cw_utils::MsgInstantiateContractResponse;

use cw20_base::msg::InstantiateMsg as Cw20InstantiateMsg;

use synedao::{
    cw20_vesting::ExecuteMsg as Cw20VestingExecuteMsg,
    bwynd_vault::InstantiateMsg as VaultInstantiateMsg,
    wynd_dao_core::ExecuteMsg as WyndDaoCoreExecuteMsg
};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_BTOKEN_ID: u64 = 1;
const INSTANTIATE_VAULT_ID: u64 = 2;

//Initialize the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    let config = Config {
        admin: deps.api.addr_validate(&msg.admin)?,
        cw20_code_id: msg.cw20_code_id,
        vault_code_id: msg.vault_code_id,
        wynd_token: deps.api.addr_validate("juno1mkw83sv6c7sjdvsaplrzc8yaes9l42p4mhy0ssuxjnyzl87c9eps7ce3m9")?,
        wynd_staking_module: deps.api.addr_validate("juno1sy9mlw47w44f94zea7g98y5ff4cvtc8rfv75jgwphlet83wlf4ssa050mv")?,
        unbonding_period: 63072000,
        bwynd: None,
        bwynd_vault: None,
    };

    CONFIG.save(deps.storage, &config)?;

    let gauge_config = GaugeConfig {
        wynd_gauge_contract: "juno14va0k6whnaptyr3pl8ajdjdu5p420sywyyuer3mqsvtl4xugh8lqatjcz6".to_string(),
        synergistic_wynd_gauge_contract: msg.synergistic_wynd_gauge_contract,
    };

    GAUGE_CONFIG.save(deps.storage, &gauge_config)?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_BTOKEN_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.admin.to_string()),
            code_id: msg.cw20_code_id,
            msg: to_binary(&Cw20InstantiateMsg {
                name: "bWYND Token".to_string(),
                symbol: "bWYND".to_string(),
                decimals: 6,
                initial_balances: [].into(),
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None
                }),
                marketing: None
            })?,
            funds: vec![],
            label: "bWYND Contract".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
    )
}

//Execute the handle messages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateAdmin {
            address
        } => execute_update_admin(
            deps,
            info,
            address
        ),
        ExecuteMsg::UpdateConfig {
            unbonding_period,
        } => execute_update_config(
            deps,
            info,
            unbonding_period,
        ),
        ExecuteMsg::UpdateGaugeConfig { 
            wynd_gauge_contract, 
            synergistic_wynd_gauge_contract 
        } => execute_update_gauge_config(
            deps, 
            env,
            info,
            wynd_gauge_contract, 
            synergistic_wynd_gauge_contract
        ),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::WithdrawRewards {} => execute_withdraw_rewards(deps, env, info),
        ExecuteMsg::ExecuteCosmosMsgs {msgs} => execute_cosmos_msgs(deps, env, info, msgs),
        ExecuteMsg::Mint {recipient, amount} => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::PlaceVotes {
            gauge,
            votes,
        } => execute_vote(deps, env, info.sender, gauge, votes),
    }
}

pub fn execute_update_admin(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    
    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    config.admin = deps.api.addr_validate(&address)?;
    
    CONFIG.save(deps.storage, &config)?;

    Ok(
        Response::new()
            .add_attribute("action", "update_admin")
            .add_attribute("admin", address)
    )
}

// Only admin can execute it.
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    unbonding_period: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    let mut changed = false;

    let mut response = Response::new().add_attribute("action", "update_config");

    if let Some(unbonding_period) = unbonding_period {
        changed = true;
        config.unbonding_period = unbonding_period;
        response = response.add_attribute("unbonding_period", unbonding_period.to_string());
    }

    ensure_eq!(changed, true, ContractError::InvalidParams {});
    
    CONFIG.save(deps.storage, &config)?;

    Ok(response)
}

pub fn execute_update_gauge_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    wynd_gauge_contract: Option<String>,
    synergistic_wynd_gauge_contract: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut gauge_cfg = GAUGE_CONFIG.load(deps.storage).unwrap_or(GaugeConfig { 
        wynd_gauge_contract: "juno14va0k6whnaptyr3pl8ajdjdu5p420sywyyuer3mqsvtl4xugh8lqatjcz6".to_string(), 
        synergistic_wynd_gauge_contract: None
    });

    ensure_eq!(info.sender, cfg.admin, ContractError::Unauthorized {});

    let mut valid = false;

    let mut res = Response::new()
        .add_attribute("action", "update_gauge_config");

    if let Some(wynd_gauge_contract) = wynd_gauge_contract {
        gauge_cfg.wynd_gauge_contract = wynd_gauge_contract;
        valid = true;
        res = res.add_attribute(
            "loop_gauge_contract", 
            &gauge_cfg.wynd_gauge_contract.to_string()
        );
    }

    if synergistic_wynd_gauge_contract.is_some() {
        gauge_cfg.synergistic_wynd_gauge_contract = synergistic_wynd_gauge_contract.clone();
        valid = true;
        res = res.add_attribute("synergistic_loop_gauge_contract", &synergistic_wynd_gauge_contract.unwrap().to_string());
    }

    GAUGE_CONFIG.save(deps.storage, &gauge_cfg)?;
    
    ensure_eq!(valid, true, ContractError::InvalidParams {});

    Ok(res)
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Convert {}) => execute_convert_token(
            deps,
            env,
            info,
            cw20_msg.sender,
            cw20_msg.amount,
        ),
        Err(_err) => Err(ContractError::GenericErr {}),
    }
}

pub fn execute_convert_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    amount: Uint128
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender.clone(), config.wynd_token, ContractError::InvalidToken {});
    ensure_eq!((amount.gt(&Uint128::zero())), true, ContractError::InvalidZeroAmount {});
    ensure_eq!((config.unbonding_period > 0), true, ContractError::UnbondingPeriodErr {});

    let bwynd = config.bwynd.clone().unwrap().to_string();

    let mut attrs = vec![
        ("action", "stake_token".to_string()),
        ("token", config.wynd_token.to_string()),
        ("to", config.wynd_staking_module.to_string()),
        ("action", "mint_token".to_string()),
        ("token", bwynd.clone()),
        ("to", sender.clone()),
        ("amount", amount.to_string()),
    ];
    
    let mut messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: config.wynd_token.to_string(),
            msg: to_binary(
                &Cw20VestingExecuteMsg::Delegate { 
                    amount: amount, 
                    msg: to_binary(
                        &ReceiveDelegationMsg::Delegate {
                            unbonding_period: config.unbonding_period,
                        }
                    )? 
                }
            )?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: bwynd.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: sender.to_string(), amount })?,
            funds: vec![],
        }),
    ];

    let withdrawable_amount = query_wynd_dao_core_rewards(deps.querier, config.wynd_staking_module.clone(), env.contract.address.clone())?.rewards;
    if withdrawable_amount.gt(&Uint128::zero()) {
        attrs.push(("action", "restake_token".to_string()));
        attrs.push(("token", config.wynd_token.to_string()));
        attrs.push(("to", config.wynd_staking_module.to_string()));
        attrs.push(("amount", withdrawable_amount.to_string()));

        attrs.push(("action", "mint_token".to_string()));
        attrs.push(("token", bwynd.clone()));
        attrs.push(("to", env.contract.address.to_string()));
        attrs.push(("amount", withdrawable_amount.to_string()));

        attrs.push(("action", "distribute_rewards".to_string()));
        attrs.push(("token", bwynd.clone()));
        attrs.push(("to", env.contract.address.to_string()));
        attrs.push(("amount", withdrawable_amount.to_string()));

        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute  {
                contract_addr: config.wynd_staking_module.clone().to_string(),
                msg: to_binary(&WyndDaoCoreExecuteMsg::WithdrawRewards {
                    owner: None,
                    receiver: None,
                })?,
                funds: vec![] 
            })
        );
        
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute  {
                contract_addr: config.wynd_token.clone().to_string(),
                msg: to_binary(
                    &Cw20VestingExecuteMsg::Delegate { 
                        amount: withdrawable_amount, 
                        msg: to_binary(
                            &ReceiveDelegationMsg::Delegate {
                                unbonding_period: config.unbonding_period,
                            }
                        )? 
                    }
                )?,
                funds: vec![] 
            }),
        );
        
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                //sending reward to user
                contract_addr: config.bwynd.clone().unwrap().to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: env.contract.address.clone().to_string(), amount: withdrawable_amount })?,
                funds: vec![],
            })
        );

        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.bwynd.clone().unwrap().to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.bwynd_vault.clone().unwrap().to_string(),
                    amount: withdrawable_amount,
                    msg: to_binary(&VaultCw20HookMsg::DistributeRewards { address: Some(deps.api.addr_validate(&sender)?) })?,
                })?,
                funds: vec![] 
            })
        );
    } else {
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.bwynd_vault.clone().unwrap().to_string(),
                msg: to_binary(&VaultExecuteMsg::DistributeRewards { address: deps.api.addr_validate(&sender)?, })?,
                funds: vec![]
            })
        );
    }

    Ok(Response::new()
        .add_attributes(attrs)
        .add_messages(messages)
    )
}

pub fn execute_withdraw_rewards(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let withdrawable_amount = query_wynd_dao_core_rewards(deps.querier, config.wynd_staking_module.clone(), env.contract.address.clone())?.rewards;

    ensure_eq!(withdrawable_amount.gt(&Uint128::zero()), true, ContractError::InvalidZeroAmount {});

    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute  {
            contract_addr: config.wynd_staking_module.clone().to_string(),
            msg: to_binary(&WyndDaoCoreExecuteMsg::WithdrawRewards {
                owner: None,
                receiver: None,
            })?,
            funds: vec![] 
        }),
        CosmosMsg::Wasm(WasmMsg::Execute  {
            contract_addr: config.wynd_token.clone().to_string(),
            msg: to_binary(
                &Cw20VestingExecuteMsg::Delegate { 
                    amount: withdrawable_amount, 
                    msg: to_binary(
                        &ReceiveDelegationMsg::Delegate {
                            unbonding_period: config.unbonding_period,
                        }
                    )? 
                }
            )?,
            funds: vec![] 
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: config.bwynd.clone().unwrap().to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: env.contract.address.clone().to_string(), amount: withdrawable_amount })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.bwynd.clone().unwrap().to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: config.bwynd_vault.unwrap().to_string(),
                amount: withdrawable_amount,
                msg: to_binary(&VaultCw20HookMsg::DistributeRewards {address: None})?,
            })?,
            funds: vec![] 
        })
    ];

    Ok(Response::new().add_attribute("action", "withdraw_rewards").add_messages(messages))
}

pub fn execute_cosmos_msgs(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msgs: Vec<CosmosMsg>
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    Ok(Response::new().add_messages(msgs))
}

pub fn execute_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    let attrs = vec![
        ("action", "mint_token".to_string()),
        ("token", config.bwynd.clone().unwrap().to_string()),
        ("to", recipient.clone()),
        ("amount", amount.to_string()),
    ];

    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: config.bwynd.unwrap().to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { recipient, amount })?,
            funds: vec![],
        }),
    ];

    Ok(Response::new()
        .add_attributes(attrs)
        .add_messages(messages)
    )
}

pub fn execute_vote(
    deps: DepsMut, 
    _env: Env, 
    sender: Addr, 
    gauge: u64, 
    votes: Option<Vec<Vote>>
) -> Result<Response, ContractError> {
    let gauge_cfg = GAUGE_CONFIG.load(deps.storage)?;
    
    ensure_eq!(gauge_cfg.synergistic_wynd_gauge_contract.is_some(), true, ContractError::InvalidAddr {});
    ensure_eq!(gauge_cfg.synergistic_wynd_gauge_contract.unwrap(), sender.to_string(), ContractError::Unauthorized {});

    let message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: gauge_cfg.wynd_gauge_contract.clone(),
        msg: to_binary(&ExecuteMsg::PlaceVotes { gauge, votes })?,
        funds: vec![],
    });
    Ok(Response::new()
        .add_attribute("action", "synergistic wynd gauge vote")
        .add_attribute("proposal_id", gauge.to_string())
        .add_message(message)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryConfig {} => to_binary(&query_config(deps)?),
        QueryMsg::QueryGaugeConfig {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    Ok(ConfigResponse {
        config: CONFIG.load(deps.storage)?,
    })
}

pub fn query_gauge_config(deps: Deps) -> StdResult<GaugeConfigResponse> {
    Ok(GaugeConfigResponse {
        gauge_config: GAUGE_CONFIG.load(deps.storage)?,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.result.is_err() {
        return Err(ContractError::ErrorReply {});
    }
    match msg.id {
        INSTANTIATE_BTOKEN_ID => {
            // parse the reply
            let res = cw_utils::parse_reply_instantiate_data(msg).map_err(|_| {
                StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
            })?;
            reply_btoken_instantiate(deps, env, res)
        },
        INSTANTIATE_VAULT_ID => {
            let res = cw_utils::parse_reply_instantiate_data(msg).map_err(|_| {
                StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
            })?;
            reply_btoken_vault_instantiate(deps, env, res)
        },
        x => Err(ContractError::UnknownReply(x)),
    }
}

pub fn reply_btoken_instantiate(deps: DepsMut, _env: Env, res: MsgInstantiateContractResponse) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let bwynd = deps.api.addr_validate(&res.contract_address)?;

    config.bwynd = Some(bwynd.clone());

    CONFIG.save(deps.storage, &config)?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_VAULT_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.admin.to_string()),
            code_id: config.vault_code_id,
            msg: to_binary(&VaultInstantiateMsg {
                admin: config.admin.to_string(),
                token: bwynd.to_string(),
                wynd_staking_module: config.wynd_staking_module.to_string(),
                min_bond: Uint128::from(1000000u128),
            })?,
            funds: vec![],
            label: "btoken Vault Contract".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_attribute("bToken", bwynd.to_string())
    )
}

pub fn reply_btoken_vault_instantiate(deps: DepsMut, _env: Env, res: MsgInstantiateContractResponse) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let btoken_vault_address = deps.api.addr_validate(&res.contract_address)?;

    config.bwynd_vault = Some(btoken_vault_address.clone());

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("btoken Vault Contract", btoken_vault_address.to_string())
    )
}
