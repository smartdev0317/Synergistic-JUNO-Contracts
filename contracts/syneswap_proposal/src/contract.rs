// use std::fmt::format;
// use std::ops::Add;

#[cfg(not(feature = "library"))]
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::proposal::{advance_proposal_id, SingleChoiceProposal};
use crate::query::{
    ProposalListResponse, ProposalResponse, VoteInfo, VoteListResponse, VoteResponse,
};
use crate::state::{
    Ballot, Config, BALLOTS, CONFIG, PROPOSALS, PROPOSAL_COUNT, PROPOSERS_INFO,
};

use crate::status::{Status};
use crate::threshold::Threshold;
use crate::voting::{validate_voting_period, Vote, Votes};
use cosmwasm_std::{
    entry_point, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    QueryRequest, Response, StdError, StdResult, Uint128, WasmQuery,
};
use cw2::set_contract_version;
use cw20::BalanceResponse;
use cw_storage_plus::Bound;
use cw_utils::Duration;
use syneswap::factory::MigrateMsg;
use syneswap::staking::QueryMsg as stakingMsg;
use syneswap_staking::state::Config as StakingConfig;
// use syneswap_staking::{msg::QueryMsg as QueryStakingMsg, state::Config as StakingConfig};

pub const DEFAULT_LIMIT: u64 = 30;
pub const MAX_PROPOSAL_SIZE: u64 = 30_000;

pub(crate) const CONTRACT_NAME: &str = "crates.io:cwd-proposal-single";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    msg.threshold.validate()?;

    let (min_voting_period, max_voting_period) =
        validate_voting_period(msg.min_voting_period, msg.max_voting_period)?;

    // let (initial_policy, pre_propose_messages) = msg
    //     .pre_propose_info
    //     .into_initial_policy_and_messages(dao.clone())?;

    let config = Config {
        threshold: msg.threshold,
        max_voting_period,
        min_voting_period,
        dao: msg.dao,
        admin: info.sender.to_string(),
        proposal_creation_token_limit: msg.proposal_creation_token_limit,
        token_hold_duration: msg.token_hold_duration,
    };

    // Initialize proposal count to zero so that queries return zero
    // instead of None.
    PROPOSAL_COUNT.save(deps.storage, &0)?;
    CONFIG.save(deps.storage, &config)?;
    // CREATION_POLICY.save(deps.storage, &initial_policy)?;

    Ok(Response::default()
        // .add_submessages(pre_propose_messages)
        .add_attribute("action", "instantiate")
        .add_attribute("dao", config.dao))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Propose {
            title,
            description,
            msgs,
            voting_period,
        } => execute_propose(
            deps,
            env,
            info.sender,
            title,
            description,
            msgs,
            voting_period,
        ),
        ExecuteMsg::Vote { proposal_id, vote } => execute_vote(deps, env, info, proposal_id, vote),
        ExecuteMsg::Execute { proposal_id } => execute_execute(deps, env, info, proposal_id),
        ExecuteMsg::Close { proposal_id } => execute_close(deps, env, info, proposal_id),
        ExecuteMsg::UpdateConfig {
            threshold,
            max_voting_period,
            min_voting_period,
            dao,
            token_hold_duration,
            proposal_creation_token_limit,
        } => execute_update_config(
            deps,
            info,
            threshold,
            max_voting_period,
            min_voting_period,
            dao,
            token_hold_duration,
            proposal_creation_token_limit,
        ),
    }
}

pub fn execute_propose(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    title: String,
    description: String,
    msgs: Vec<CosmosMsg<Empty>>,
    voting_period: Duration,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    if !verifying_voting_period(
        &voting_period,
        &config.min_voting_period,
        &config.max_voting_period,
    ) {
        return Err(StdError::generic_err("invalid voting period"));
    }

    let vote_power = get_voting_power(deps.as_ref(), sender.clone(), config.dao.to_string())?;
    // let power = Uint128::from(1000000u128);
    if vote_power.is_zero() {
        return Err(StdError::generic_err("Power is zero can't create proposal"));
    }

    let expiration = voting_period.after(&env.block);
    let mut locked_amount = PROPOSERS_INFO
        .load(deps.storage, sender.to_string())
        .unwrap_or(Uint128::zero());

    if vote_power - locked_amount < config.proposal_creation_token_limit {
        return Err(StdError::generic_err(
            "Power is not enough to create proposal",
        ));
    }
    locked_amount += config.proposal_creation_token_limit;

    PROPOSERS_INFO.save(deps.storage, sender.to_string(), &locked_amount)?;

    let proposal = {
        // Limit mutability to this block.
        let mut proposal = SingleChoiceProposal {
            title,
            description,
            proposer: sender.clone(),
            expiration,
            threshold: config.threshold,
            total_power: Uint128::zero(),
            msgs,
            status: Status::Open,
            votes: Votes::zero(),
            allow_revoting: false,
            voting_start_time: env.block.time.seconds(),
        };
        // Update the proposal's status. Addresses case where proposal
        // expires on the same block as it is created.
        proposal.update_status(&env.block);
        proposal
    };
    let id = advance_proposal_id(deps.storage)?;

    // Limit the size of proposals.
    //
    // The Juno mainnet has a larger limit for data that can be
    // uploaded as part of an execute message than it does for data
    // that can be queried as part of a query. This means that without
    // this check it is possible to create a proposal that can not be
    // queried.
    //
    // The size selected was determined by uploading versions of this
    // contract to the Juno mainnet until queries worked within a
    // reasonable margin of error.
    //
    // `to_vec` is the method used by cosmwasm to convert a struct
    // into it's byte representation in storage.
    let proposal_size = cosmwasm_std::to_vec(&proposal)?.len() as u64;
    if proposal_size > MAX_PROPOSAL_SIZE {
        return Err(StdError::generic_err("Proposal size is too large"));
    }

    PROPOSALS.save(deps.storage, id, &proposal)?;

    Ok(Response::default()
        // .add_submessages(hooks)
        .add_attribute("action", "propose")
        .add_attribute("sender", sender)
        .add_attribute("proposal_id", id.to_string())
        .add_attribute("status", proposal.status.to_string()))
}

pub fn execute_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    // if config.only_members_execute {

    // }

    let mut prop: SingleChoiceProposal = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No Such Proposal"))?;
    let vote_power = get_voting_power(deps.as_ref(), info.sender.clone(), config.dao.to_string())?;
    // let power = Uint128::from(1000000u128);
    if vote_power.is_zero() {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // Check here that the proposal is passed. Allow it to be executed
    // even if it is expired so long as it passed during its voting
    // period.

    let mut locked_amount = PROPOSERS_INFO
        .load(deps.storage, prop.proposer.to_string())
        .unwrap_or(Uint128::zero());
    locked_amount -= config.proposal_creation_token_limit;

    PROPOSERS_INFO.save(deps.storage, prop.proposer.to_string(), &locked_amount)?;
    let description = prop.is_passed(&env.block).description;
    if description != "" {
        return Err(StdError::generic_err(description));
    }
    prop.update_status(&env.block);
    // if matches!(old_status , Status::Open {voting_status} ) && prop.is_passed(&env.block) {
    //     prop.status = Status::Passed;
    // }
    println!("{:?}", prop.status);
    if prop.status != Status::Passed {
        return Err(StdError::generic_err("Proposal is not in 'passed' state"));
    }
    prop.status = Status::Executed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    Ok(Response::new()
        .add_attribute("action", "execute")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("dao", config.dao)
        .add_messages(prop.msgs))
}

pub fn execute_vote(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote: Vote,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut prop: SingleChoiceProposal = PROPOSALS
        .may_load(deps.storage, proposal_id)?
        .ok_or(StdError::generic_err("No such proposal"))?;

    // let vote_power=Uint128::from(100000u128);
    let vote_power = get_voting_power(deps.as_ref(), info.sender.clone(), config.dao.to_string())?;

    if vote_power.is_zero() {
        let mut message = "Not Registered".to_string();
        message.push_str(&format!(
            "sender {} dao {}",
            info.sender.clone(),
            config.dao.to_string(),
        ));
        return Err(StdError::generic_err(&message));
    }

    if prop.expiration.is_expired(&env.block) {
        return Err(StdError::generic_err("Voting time expired"));
    } else if prop.status == Status::Executed || prop.status == Status::Closed {
        return Err(StdError::generic_err(format!(
            "Proposal is in {} state",
            prop.status
        )));
    }
    // prop.status = Status::Executed;

    BALLOTS.update(
        deps.storage,
        (proposal_id, info.sender.clone()),
        |bal| match bal {
            Some(current_ballot) => {
                if prop.allow_revoting {
                    if current_ballot.vote == vote {
                        // Don't allow casting the same vote more than
                        // once. This seems liable to be confusing
                        // behavior.
                        Err(StdError::generic_err("Already Casted"))
                    } else {
                        // Remove the old vote if this is a re-vote.
                        prop.votes
                            .remove_vote(current_ballot.vote, current_ballot.power);
                        Ok(Ballot {
                            power: vote_power,
                            vote: vote.clone(),
                        })
                    }
                } else {
                    prop.votes
                        .remove_vote(current_ballot.vote, current_ballot.power);
                    Ok(Ballot {
                        power: vote_power,
                        vote: vote.clone(),
                    })
                }
            }
            None => Ok(Ballot {
                power: vote_power,
                vote: vote.clone(),
            }),
        },
    )?;

    // let old_status = prop.clone().status;

    prop.votes.add_vote(vote.clone(), vote_power.clone());
    prop.total_power = get_total_power(deps.as_ref(), config.dao.to_string())?;
    prop.update_status(&env.block);

    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // let new_status = prop.clone().status;

    Ok(Response::default()
        .add_attribute("action", "vote")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("position", vote.to_string())
        .add_attribute("status", prop.status.to_string())
        .add_attribute("power added", vote_power))
}

pub fn execute_close(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> StdResult<Response> {
    let mut prop: SingleChoiceProposal = PROPOSALS.load(deps.storage, proposal_id)?;
    let config: Config = CONFIG.load(deps.storage)?;
    // Update status to ensure that proposals which were open and have
    // expired are moved to "rejected."

    // if info.sender != config.admin {
    //     return Err(StdError::generic_err("Unauthorized"));
    // }
    let mut locked_amount = PROPOSERS_INFO
        .load(deps.storage, prop.proposer.to_string())
        .unwrap_or(Uint128::zero());
    if prop.voting_start_time + config.token_hold_duration > env.block.time.seconds() {
        return Err(StdError::generic_err(
            "tokens are hold in the contract kindly wait for the specified time",
        ));
    }

    prop.update_status(&env.block);
    if prop.status != Status::Rejected {
        return Err(StdError::generic_err("Only rejected proposals closed."));
    }

    locked_amount -= config.proposal_creation_token_limit;

    PROPOSERS_INFO.save(deps.storage, prop.proposer.to_string(), &locked_amount)?;
    // let old_status = prop.clone().status;
    prop.status = Status::Closed;
    PROPOSALS.save(deps.storage, proposal_id, &prop)?;

    // Add prepropose / deposit module hook which will handle deposit refunds.

    Ok(Response::default()
        .add_attribute("action", "close")
        .add_attribute("sender", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string()))
}

pub fn get_voting_power(deps: Deps, sender: Addr, dao: String) -> StdResult<Uint128> {
    let power: BalanceResponse = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: dao.to_string(),
            msg: to_binary(&stakingMsg::Balance {
                address: sender.to_string(),
            })?,
        }))
        .unwrap_or(BalanceResponse {
            balance: Uint128::from(0u128),
        });
    Ok(power.balance)
    // Ok(Uint128::from(1024374967541u128))
}

pub fn get_total_power(deps: Deps, dao: String) -> StdResult<Uint128> {
    let mut total_power = Uint128::zero();

    let config: StakingConfig = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: dao.to_string(),
        msg: to_binary(&stakingMsg::QueryConfig {})?,
    }))?;

    let durations = config.duration_values_vector;
    for duration in durations {
        let power: BalanceResponse = deps
            .querier
            .query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: dao.to_string(),
                msg: to_binary(&stakingMsg::TotalBalance { duration })?,
            }))
            .unwrap_or(BalanceResponse {
                balance: Uint128::from(0u128),
            });
        total_power += power.balance;
    }
    Ok(total_power)

    // Ok(Uint128::from(383603867678926u128))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    threshold_: Option<Threshold>,
    max_voting_period_: Option<Duration>,
    min_voting_period_: Option<Duration>,
    dao_: Option<String>,
    token_hold_duration_: Option<u64>,
    proposal_creation_token_limit_: Option<Uint128>,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    // Only the DAO may call this method.
    if info.sender != config.admin {
        return Err(StdError::generic_err("Unauthorized"));
    }
    let mut threshold = config.threshold;
    if threshold_.is_some() {
        threshold = threshold_.unwrap();
        threshold.validate()?
    }
    let mut min_voting_period = config.min_voting_period;
    let mut max_voting_period = config.max_voting_period;
    if min_voting_period_.is_some() {
        min_voting_period = min_voting_period_.unwrap();
    }
    if max_voting_period_.is_some() {
        max_voting_period = max_voting_period_.unwrap();
    }
    let (min_voting_period, max_voting_period) =
        validate_voting_period(min_voting_period, max_voting_period)?;
    let mut dao = config.dao;
    if dao_.is_some() {
        dao = dao_.unwrap();
    }
    let mut token_hold_duration = config.token_hold_duration;
    if token_hold_duration_.is_some() {
        token_hold_duration = token_hold_duration_.unwrap();
    }

    let mut proposal_creation_token_limit = config.proposal_creation_token_limit;
    if proposal_creation_token_limit_.is_some() {
        proposal_creation_token_limit = proposal_creation_token_limit_.unwrap();
    }
    CONFIG.save(
        deps.storage,
        &Config {
            threshold,
            max_voting_period,
            min_voting_period,
            dao,
            admin: config.admin,
            token_hold_duration,
            proposal_creation_token_limit,
        },
    )?;

    Ok(Response::default()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn verifying_voting_period(
    voting_period: &Duration,
    min_vp: &Duration,
    max_vp: &Duration,
) -> bool {
    match (voting_period, min_vp, max_vp) {
        // compare if both height or both time
        (Duration::Time(vp), Duration::Time(mvp1), Duration::Time(mvp2)) => {
            vp >= mvp1 && vp <= mvp2
        }

        // if they are mis-matched finite ends, no compare possible
        _ => false,
    }
}
// pub fn execute_update_proposal_creation_policy(
//     deps: DepsMut,
//     info: MessageInfo,
//     new_info: PreProposeInfo,
// ) -> StdResult<Response,> {
//     let config = CONFIG.load(deps.storage)?;
//     if config.dao != info.sender {
//         return Err(ContractError::Unauthorized {});
//     }

//     let (initial_policy, messages) = new_info.into_initial_policy_and_messages(config.dao)?;
//     CREATION_POLICY.save(deps.storage, &initial_policy)?;

//     Ok(Response::default()
//         .add_submessages(messages)
//         .add_attribute("action", "update_proposal_creation_policy")
//         .add_attribute("sender", info.sender)
//         .add_attribute("new_policy", format!("{initial_policy:?}")))
// }

// pub fn add_hook(
//     hooks: Hooks,
//     storage: &mut dyn Storage,
//     validated_address: Addr,
// ) -> StdResult<()> {
//     hooks
//         .add_hook(storage, validated_address)
//         .map_err(ContractError::HookError)?;
//     Ok(())
// }

// pub fn remove_hook(
//     hooks: Hooks,
//     storage: &mut dyn Storage,
//     validate_address: Addr,
// ) -> StdResult<()> {
//     hooks
//         .remove_hook(storage, validate_address)
//         .map_err(ContractError::HookError)?;
//     Ok(())
// }

// pub fn execute_add_proposal_hook(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     address: String,
// ) -> StdResult<Response,> {
//     let config = CONFIG.load(deps.storage)?;
//     if config.dao != info.sender {
//         // Only DAO can add hooks
//         return Err(ContractError::Unauthorized {});
//     }

//     let validated_address = deps.api.addr_validate(&address)?;

//     add_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;

//     Ok(Response::default()
//         .add_attribute("action", "add_proposal_hook")
//         .add_attribute("address", address))
// }

// pub fn execute_remove_proposal_hook(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     address: String,
// ) -> Result<Response, ContractError> {
//     let config = CONFIG.load(deps.storage)?;
//     if config.dao != info.sender {
//         // Only DAO can remove hooks
//         return Err(ContractError::Unauthorized {});
//     }

//     let validated_address = deps.api.addr_validate(&address)?;

//     remove_hook(PROPOSAL_HOOKS, deps.storage, validated_address)?;

//     Ok(Response::default()
//         .add_attribute("action", "remove_proposal_hook")
//         .add_attribute("address", address))
// }

// pub fn execute_add_vote_hook(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     address: String,
// ) -> Result<Response, ContractError> {
//     let config = CONFIG.load(deps.storage)?;
//     if config.dao != info.sender {
//         // Only DAO can add hooks
//         return Err(ContractError::Unauthorized {});
//     }

//     let validated_address = deps.api.addr_validate(&address)?;

//     add_hook(VOTE_HOOKS, deps.storage, validated_address)?;

//     Ok(Response::default()
//         .add_attribute("action", "add_vote_hook")
//         .add_attribute("address", address))
// }

// pub fn execute_remove_vote_hook(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     address: String,
// ) -> Result<Response, ContractError> {
//     let config = CONFIG.load(deps.storage)?;
//     if config.dao != info.sender {
//         // Only DAO can remove hooks
//         return Err(ContractError::Unauthorized {});
//     }

//     let validated_address = deps.api.addr_validate(&address)?;

//     remove_hook(VOTE_HOOKS, deps.storage, validated_address)?;

//     Ok(Response::default()
//         .add_attribute("action", "remove_vote_hook")
//         .add_attribute("address", address))
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => query_config(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::Proposal { proposal_id } => query_proposal(deps, env, proposal_id),
        QueryMsg::ListProposals { start_after, limit } => {
            query_list_proposals(deps, env, start_after, limit)
        }
        QueryMsg::ProposalCount {} => query_proposal_count(deps),
        QueryMsg::GetVote { proposal_id, voter } => query_vote(deps, proposal_id, voter),
        QueryMsg::ListVotes {
            proposal_id,
            start_after,
            limit,
        } => query_list_votes(deps, proposal_id, start_after, limit),
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::HoldAmount { address } => query_hold_amount(deps, address),
        // QueryMsg::ReverseProposals {
        //     start_before,
        //     limit,
        // } => query_reverse_proposals(deps, env, start_before, limit),
        // QueryMsg::ProposalCreationPolicy {} => query_creation_policy(deps),
        // QueryMsg::ProposalHooks {} => to_binary(&PROPOSAL_HOOKS.query_hooks(deps)?),
        // QueryMsg::VoteHooks {} => to_binary(&VOTE_HOOKS.query_hooks(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_binary(&config)
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_binary(&config.dao)
}

pub fn query_proposal(deps: Deps, env: Env, id: u64) -> StdResult<Binary> {
    let proposal = PROPOSALS.load(deps.storage, id)?;
    let votes = BALLOTS
        .prefix(id)
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .count();
    to_binary(&proposal.into_response(&env.block, id, votes))
}

pub fn query_hold_amount(deps: Deps, address: String) -> StdResult<Binary> {
    to_binary(
        &PROPOSERS_INFO
            .load(deps.storage, address)
            .unwrap_or(Uint128::zero()),
    )
}

// pub fn query_creation_policy(deps: Deps) -> StdResult<Binary> {
//     let policy = CREATION_POLICY.load(deps.storage)?;
//     to_binary(&policy)
// }

pub fn query_list_proposals(
    deps: Deps,
    env: Env,
    start_after: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let min = start_after.map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let props: Vec<ProposalResponse> = PROPOSALS
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, SingleChoiceProposal)>, _>>()?
        .into_iter()
        .map(|(id, proposal)| {
            let votes = BALLOTS
                .prefix(id)
                .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                .count();
            proposal.into_response(&env.block, id, votes)
        })
        .collect();

    to_binary(&ProposalListResponse { proposals: props })
}

pub fn query_reverse_proposals(
    deps: Deps,
    env: Env,
    start_before: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let max = start_before.map(Bound::exclusive);
    let props: Vec<ProposalResponse> = PROPOSALS
        .range(deps.storage, None, max, cosmwasm_std::Order::Descending)
        .take(limit as usize)
        .collect::<Result<Vec<(u64, SingleChoiceProposal)>, _>>()?
        .into_iter()
        .map(|(id, proposal)| {
            let votes = BALLOTS
                .prefix(id)
                .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
                .count();
            proposal.into_response(&env.block, id, votes)
        })
        .collect();

    to_binary(&ProposalListResponse { proposals: props })
}

pub fn query_proposal_count(deps: Deps) -> StdResult<Binary> {
    let proposal_count = PROPOSAL_COUNT.load(deps.storage)?;
    to_binary(&proposal_count)
}

pub fn query_vote(deps: Deps, proposal_id: u64, voter: String) -> StdResult<Binary> {
    let voter = deps.api.addr_validate(&voter)?;
    let ballot = BALLOTS.may_load(deps.storage, (proposal_id, voter.clone()))?;
    let vote = ballot.map(|ballot| VoteInfo {
        voter,
        vote: ballot.vote,
        power: ballot.power,
    });
    to_binary(&VoteResponse { vote })
}

pub fn query_list_votes(
    deps: Deps,
    proposal_id: u64,
    start_after: Option<String>,
    limit: Option<u64>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let start_after = start_after
        .map(|addr| deps.api.addr_validate(&addr))
        .transpose()?;
    let min = start_after.map(Bound::<Addr>::exclusive);

    let votes = BALLOTS
        .prefix(proposal_id)
        .range(deps.storage, min, None, cosmwasm_std::Order::Ascending)
        .take(limit as usize)
        .map(|item| {
            let (voter, ballot) = item?;
            Ok(VoteInfo {
                voter,
                vote: ballot.vote,
                power: ballot.power,
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    to_binary(&VoteListResponse { votes })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&info)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
    //     // Set contract to version to latest
    //     set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    //     match msg {
    //         MigrateMsg::FromV1 {
    //             close_proposal_on_execution_failure,
    //             pre_propose_info,
    //         } => {
    //             // Update the stored config to have the new
    //             // `close_proposal_on_execution_falure` field.
    //             let current_config = v1::state::CONFIG.load(deps.storage)?;
    //             CONFIG.save(
    //                 deps.storage,
    //                 &Config {
    //                     threshold: v1_threshold_to_v2(current_config.threshold),
    //                     max_voting_period: v1_duration_to_v2(current_config.max_voting_period),
    //                     min_voting_period: current_config.min_voting_period.map(v1_duration_to_v2),
    //                     only_members_execute: current_config.only_members_execute,
    //                     allow_revoting: current_config.allow_revoting,
    //                     dao: current_config.dao.clone(),
    //                     close_proposal_on_execution_failure,
    //                 },
    //             )?;

    //             let (initial_policy, pre_propose_messages) =
    //                 pre_propose_info.into_initial_policy_and_messages(current_config.dao)?;
    //             CREATION_POLICY.save(deps.storage, &initial_policy)?;

    //             // Update the module's proposals to v2.

    //             let current_proposals = v1::state::PROPOSALS
    //                 .range(deps.storage, None, None, Order::Ascending)
    //                 .collect::<StdResult<Vec<(u64, v1::proposal::Proposal)>>>()?;

    //             // Based on gas usage testing, we estimate that we will be
    //             // able to migrate ~4200 proposals at a time before
    //             // reaching the block max_gas limit.
    //             current_proposals
    //                 .into_iter()
    //                 .try_for_each::<_, Result<_, ContractError>>(|(id, prop)| {
    //                     if prop
    //                         .deposit_info
    //                         .map(|info| !info.deposit.is_zero())
    //                         .unwrap_or(false)
    //                         && prop.status != voting_v1::Status::Closed
    //                         && prop.status != voting_v1::Status::Executed
    //                     {
    //                         // No migration path for outstanding
    //                         // deposits.
    //                         return Err(ContractError::PendingProposals {});
    //                     }

    //                     let migrated_proposal = SingleChoiceProposal {
    //                         title: prop.title,
    //                         description: prop.description,
    //                         proposer: prop.proposer,
    //                         start_height: prop.start_height,
    //                         min_voting_period: prop.min_voting_period.map(v1_expiration_to_v2),
    //                         expiration: v1_expiration_to_v2(prop.expiration),
    //                         threshold: v1_threshold_to_v2(prop.threshold),
    //                         total_power: prop.total_power,
    //                         msgs: prop.msgs,
    //                         status: v1_status_to_v2(prop.status),
    //                         votes: v1_votes_to_v2(prop.votes),
    //                         allow_revoting: prop.allow_revoting,
    //                     };

    //                     PROPOSALS
    //                         .save(deps.storage, id, &migrated_proposal)
    //                         .map_err(|e| e.into())
    //                 })?;

    //             Ok(Response::default()
    //                 .add_attribute("action", "migrate")
    //                 .add_attribute("from", "v1")
    //                 .add_submessages(pre_propose_messages))
    //         }

    //         MigrateMsg::FromCompatible {} => Ok(Response::default()
    //             .add_attribute("action", "migrate")
    //             .add_attribute("from", "compatible")),
    //     }
}

// #[cfg_attr(not(feature = "library"), entry_point)]
// pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
//     let repl = TaggedReplyId::new(msg.id)?;
//     match repl {
//         TaggedReplyId::FailedProposalExecution(proposal_id) => {
//             PROPOSALS.update(deps.storage, proposal_id, |prop| match prop {
//                 Some(mut prop) => {
//                     prop.status = Status::ExecutionFailed;

//                     Ok(prop)
//                 }
//                 None => Err(ContractError::NoSuchProposal { id: proposal_id }),
//             })?;

//             Ok(Response::new().add_attribute("proposal_execution_failed", proposal_id.to_string()))
//         }
//         TaggedReplyId::FailedProposalHook(idx) => {
//             let addr = PROPOSAL_HOOKS.remove_hook_by_index(deps.storage, idx)?;
//             Ok(Response::new().add_attribute("removed_proposal_hook", format!("{addr}:{idx}")))
//         }
//         TaggedReplyId::FailedVoteHook(idx) => {
//             let addr = VOTE_HOOKS.remove_hook_by_index(deps.storage, idx)?;
//             Ok(Response::new().add_attribute("removed_vote_hook", format!("{addr}:{idx}")))
//         }
//         TaggedReplyId::PreProposeModuleInstantiation => {
//             let res = parse_reply_instantiate_data(msg)?;
//             let module = deps.api.addr_validate(&res.contract_address)?;
//             CREATION_POLICY.save(
//                 deps.storage,
//                 &ProposalCreationPolicy::Module { addr: module },
//             )?;

//             Ok(Response::new().add_attribute("update_pre_propose_module", res.contract_address))
//         }
//         TaggedReplyId::FailedPreProposeModuleHook => {
//             let addr = match CREATION_POLICY.load(deps.storage)? {
//                 ProposalCreationPolicy::Anyone {} => {
//                     // Something is off if we're getting this
//                     // reply and we don't have a pre-propose
//                     // module installed. This should be
//                     // unreachable.
//                     return Err(ContractError::InvalidReplyID {
//                         id: failed_pre_propose_module_hook_id(),
//                     });
//                 }
//                 ProposalCreationPolicy::Module { addr } => {
//                     // If we are here, our pre-propose module has
//                     // errored while receiving a proposal
//                     // hook. Rest in peace pre-propose module.
//                     CREATION_POLICY.save(deps.storage, &ProposalCreationPolicy::Anyone {})?;
//                     addr
//                 }
//             };
//             Ok(Response::new().add_attribute("failed_prepropose_hook", format!("{addr}")))
//         }
//     }
// }
