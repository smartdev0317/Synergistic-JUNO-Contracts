use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ConfigResponse, MigrateMsg, Cw20HookMsg, VaultCw20HookMsg, VaultExecuteMsg, GaugeConfigResponse};
use crate::state::{
    Config, CONFIG, GaugeConfig, GAUGE_CONFIG, MultipleChoiceVote
};
use crate::error::ContractError;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128, CosmosMsg, WasmMsg, from_binary, to_binary, ensure_eq, SubMsg, ReplyOn, Reply, StdError, 
};
use cw2::set_contract_version;

use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use cw_utils::MsgInstantiateContractResponse;

use cw20_base::msg::InstantiateMsg as Cw20InstantiateMsg;

use synedao::loop_protocol_staking::ExecuteMsg as LoopProtocolStakingExecuteMsg;

use synedao::bloop_vault::InstantiateMsg as VaultInstantiateMsg;

use crate::queriers::query_loop_protocol_staking_rewards;

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
        loop_token: deps.api.addr_validate("juno1qsrercqegvs4ye0yqg93knv73ye5dc3prqwd6jcdcuj8ggp6w0us66deup")?,
        loop_protocol_staking: deps.api.addr_validate("juno1rl77a4jpwf5rzmk9d6krrjekukty0m207h0sky97ls7zwq06htdqq6eq7r")?,
        duration: 12,
        bloop_token: None,
        bloop_vault: None,
        cw20_code_id: msg.cw20_code_id,
        vault_code_id: msg.vault_code_id,
    };

    CONFIG.save(deps.storage, &config)?;

    let gauge_config = GaugeConfig {
        loop_gauge_contract: "juno18kvahfjnn2kmjvae3hmmgff8gn65swcf8tk83twlfu5hr2qrjwns7k4x4z".to_string(),
        synergistic_loop_gauge_contract: msg.synergistic_loop_gauge_contract,
    };

    GAUGE_CONFIG.save(deps.storage, &gauge_config)?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_BTOKEN_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.admin.to_string()),
            code_id: msg.cw20_code_id,
            msg: to_binary(&Cw20InstantiateMsg {
                name: "bLOOP Token".to_string(),
                symbol: "bLOOP".to_string(),
                decimals: 6,
                initial_balances: [].into(),
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None
                }),
                marketing: None
            })?,
            funds: vec![],
            label: "bLOOP Contract".to_string(),
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
            duration
        } => execute_update_config(
            deps,
            info,
            duration,
        ),
        ExecuteMsg::UpdateGaugeConfig { 
            loop_gauge_contract, 
            synergistic_loop_gauge_contract 
        } => execute_update_gauge_config(
            deps, 
            env,
            info,
            loop_gauge_contract, 
            synergistic_loop_gauge_contract
        ),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::WithdrawRewards {} => execute_withdraw_rewards(deps, env, info),
        ExecuteMsg::ExecuteCosmosMsgs {msgs} => execute_cosmos_msgs(deps, env, info, msgs),
        ExecuteMsg::Mint {recipient, amount} => execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::Vote { 
            proposal_id, 
            vote 
        } => execute_vote(
            deps, 
            env, 
            info, 
            proposal_id, 
            vote
        ),
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
    duration: Option<u64>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    ensure_eq!(info.sender, config.admin, ContractError::Unauthorized {});

    let mut changed = false;

    let mut response = Response::new().add_attribute("action", "update_config");

    if let Some(duration) = duration {
        changed = true;
        config.duration = duration;
        response = response.add_attribute("duration", duration.to_string());
    }

    ensure_eq!(changed, true, ContractError::InvalidParams {});
    
    CONFIG.save(deps.storage, &config)?;

    Ok(response)
}

pub fn execute_update_gauge_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    loop_gauge_contract: Option<String>,
    synergistic_loop_gauge_contract: Option<String>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut gauge_cfg = GAUGE_CONFIG.load(deps.storage).unwrap_or(GaugeConfig { 
        loop_gauge_contract: "juno18kvahfjnn2kmjvae3hmmgff8gn65swcf8tk83twlfu5hr2qrjwns7k4x4z".to_string(), 
        synergistic_loop_gauge_contract: None
    });

    ensure_eq!(info.sender, cfg.admin, ContractError::Unauthorized {});

    let mut valid = false;

    let mut res = Response::new()
        .add_attribute("action", "update_gauge_config");

    if let Some(loop_gauge_contract) = loop_gauge_contract {
        gauge_cfg.loop_gauge_contract = loop_gauge_contract;
        valid = true;
        res = res.add_attribute(
            "loop_gauge_contract", 
            &gauge_cfg.loop_gauge_contract.to_string()
        );
    }

    if synergistic_loop_gauge_contract.is_some() {
        gauge_cfg.synergistic_loop_gauge_contract = synergistic_loop_gauge_contract.clone();
        valid = true;
        res = res.add_attribute("synergistic_loop_gauge_contract", &synergistic_loop_gauge_contract.unwrap().to_string());
    }

    GAUGE_CONFIG.save(deps.storage, &gauge_cfg)?;
    
    ensure_eq!(valid, true, ContractError::NoData {});

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

    ensure_eq!(info.sender.clone(), config.loop_token, ContractError::InvalidToken {});
    ensure_eq!((amount.gt(&Uint128::zero())), true, ContractError::InvalidZeroAmount {});

    let bloop_token = config.bloop_token.clone().unwrap().to_string();

    let mut attrs = vec![
        ("action", "stake_token".to_string()),
        ("token", config.loop_token.to_string()),
        ("to", config.loop_protocol_staking.to_string()),
        ("amount", amount.to_string()),
        ("action", "mint_token".to_string()),
        ("token", bloop_token.clone()),
        ("to", sender.clone()),
        ("amount", amount.to_string()),
    ];
    
    let mut messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: config.loop_token.to_string(),
            msg: to_binary(
                &Cw20ExecuteMsg::Send { 
                    contract: config.loop_protocol_staking.to_string(), 
                    amount: amount,
                    msg: to_binary(
                        &LoopProtocolStakingExecuteMsg::Stake { 
                            duration: config.duration,
                        }
                    )? 
                }
            )?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: bloop_token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: sender.to_string(), amount })?,
            funds: vec![],
        }),
    ];

    let user_reward_reponse = query_loop_protocol_staking_rewards(deps.querier, config.loop_protocol_staking.clone(), env.contract.address.clone())?;
    let withdrawable_amount = user_reward_reponse.user_reward.checked_add(user_reward_reponse.pending_reward).unwrap();

    if withdrawable_amount.gt(&Uint128::zero()) {
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                //sending reward to user
                contract_addr: bloop_token.clone(),
                msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: env.contract.address.to_string(), amount: withdrawable_amount })?,
                funds: vec![],
            })
        );
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: bloop_token,
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.bloop_vault.clone().unwrap().to_string(),
                    amount: withdrawable_amount,
                    msg: to_binary(&VaultCw20HookMsg::DistributeRewards {address: Some(deps.api.addr_validate(&sender)?)})?,
                })?,
                funds: vec![],
            })
        );
        attrs.push(("action", "distribute_rewards".to_string()));
        attrs.push(("to", config.bloop_vault.unwrap().to_string()));
        attrs.push(("amount", withdrawable_amount.to_string()));
        attrs.push(("user", deps.api.addr_validate(&sender)?.to_string()));
    } else {
        messages.push(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.bloop_vault.clone().unwrap().to_string(),
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

    let bloop_token = config.bloop_token.clone().unwrap().to_string();
    let bloop_vault = config.bloop_vault.unwrap().to_string();

    let user_reward_reponse = query_loop_protocol_staking_rewards(deps.querier, config.loop_protocol_staking.clone(), env.contract.address.clone())?;
    let withdrawable_amount = user_reward_reponse.user_reward.checked_add(user_reward_reponse.pending_reward).unwrap();

    ensure_eq!(withdrawable_amount.gt(&Uint128::zero()), true, ContractError::InvalidZeroAmount {});

    let attrs = vec![
        ("amount", withdrawable_amount.to_string()),
        ("action", "mint_token".to_string()),
        ("token", bloop_token.clone()),
        ("to", bloop_vault.clone()),
        ("amount", withdrawable_amount.to_string()),
    ];
    
    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.loop_protocol_staking.clone().to_string(),
            msg: to_binary(&LoopProtocolStakingExecuteMsg::Restake {
                duration: config.duration,
            })?,
            funds: vec![] 
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: bloop_token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Mint { recipient: env.contract.address.to_string(), amount: withdrawable_amount })?,
            funds: vec![],
        }),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: bloop_token,
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: bloop_vault,
                amount: withdrawable_amount,
                msg: to_binary(&VaultCw20HookMsg::DistributeRewards { address: None })?,
            })?,
            funds: vec![],
        })
    ];

    Ok(Response::new()
        .add_attributes(attrs)
        .add_messages(messages)
    )
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
        ("token", config.bloop_token.clone().unwrap().to_string()),
        ("to", recipient.clone()),
        ("amount", amount.to_string()),
    ];

    let messages = vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            //sending reward to user
            contract_addr: config.bloop_token.unwrap().to_string(),
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
    info: MessageInfo, 
    proposal_id: u64, 
    vote: Vec<MultipleChoiceVote>
) -> Result<Response, ContractError> {
    let gauge_cfg = GAUGE_CONFIG.load(deps.storage)?;
    
    ensure_eq!(gauge_cfg.synergistic_loop_gauge_contract.is_some(), true, ContractError::NoAddr {});
    ensure_eq!(gauge_cfg.synergistic_loop_gauge_contract.unwrap(), info.sender.to_string(), ContractError::Unauthorized {});

    let message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: gauge_cfg.loop_gauge_contract.clone(),
        msg: to_binary(&ExecuteMsg::Vote { 
            proposal_id, 
            vote,
        })?,
        funds: vec![],
    });
    Ok(Response::new()
        .add_attribute("action", "synergistic loop gauge vote")
        .add_attribute("proposal_id", "proposal_id")
        .add_message(message)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryConfig {} => to_binary(&query_config(deps)?),
        QueryMsg::QueryGaugeConfig {} => to_binary(&query_gauge_config(deps)?),
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
            reply_bloop_instantiate(deps, env, res)
        },
        INSTANTIATE_VAULT_ID => {
            let res = cw_utils::parse_reply_instantiate_data(msg).map_err(|_| {
                StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
            })?;
            reply_bloop_vault_instantiate(deps, env, res)
        },
        x => Err(ContractError::UnknownReply(x)),
    }
}

pub fn reply_bloop_instantiate(deps: DepsMut, _env: Env, res: MsgInstantiateContractResponse) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let bloop_address = deps.api.addr_validate(&res.contract_address)?;

    config.bloop_token = Some(bloop_address.clone());

    CONFIG.save(deps.storage, &config)?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_VAULT_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.admin.to_string()),
            code_id: config.vault_code_id,
            msg: to_binary(&VaultInstantiateMsg {
                admin: config.admin.to_string(),
                token: bloop_address.to_string(),
                loop_protocol_staking: config.loop_protocol_staking.to_string(),
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
        .add_attribute("bToken", bloop_address.to_string())
    )
}

pub fn reply_bloop_vault_instantiate(deps: DepsMut, _env: Env, res: MsgInstantiateContractResponse) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let bloop_vault_address = deps.api.addr_validate(&res.contract_address)?;

    config.bloop_vault = Some(bloop_vault_address.clone());

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("btoken Vault Contract", bloop_vault_address.to_string())
    )
}
