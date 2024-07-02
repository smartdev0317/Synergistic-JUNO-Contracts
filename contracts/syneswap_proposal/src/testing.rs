// // use crate::contract::{execute_mint, instantiate, query, query_balance};
// use crate::contract::{execute, instantiate, query};
// use crate::query::ProposalResponse;
// use crate::threshold::{PercentageThreshold, Threshold};
// use crate::voting::Vote;
// use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
// use cosmwasm_std::{from_binary, BankMsg, Coin, Decimal, Empty, Env, Timestamp, Uint128};
// use cosmwasm_std::{to_binary, CosmosMsg, StdError, SubMsg, WasmMsg};
// use cw20_base::msg::ExecuteMsg as Cw20ExecuteMsg;
// use cosmwasm_std::Api;

// use cw_utils::Duration;
// use syneswap::mock_querier::mock_dependencies;
// use std::str::FromStr;
// // use crate::msg::{InstantiateMsg};
// use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
// fn mock_env_time(time: Timestamp) -> Env {
//     let mut env = mock_env();
//     env.block.time = time;
//     env
// }

// mod tests {
//     use crate::proposal::SingleChoiceProposal;

//     use super::*;

//     #[test]
//     // #[test]
//     fn test_initialize() {
//         let mut deps = mock_dependencies(&[]);
//         let validated = deps.api.addr_validate("ujuno").unwrap();
//         let init_msg = InstantiateMsg {
//             threshold: Threshold::ThresholdQuorum { threshold: PercentageThreshold::Majority {  }, quorum: PercentageThreshold::Percent(Decimal::from_str("0.01").unwrap()) },
//             // threshold: Threshold::AbsolutePercentage {
//             //     percentage: PercentageThreshold::Majority {},
//             // },
//             max_voting_period: Duration::Time(300),
//             min_voting_period: Duration::Time(5),

//             only_members_execute: false,

//             allow_revoting: false,

//             close_proposal_on_execution_failure: false,
//             dao: "Staking".to_string(),
//             proposal_creation_token_limit: Uint128::from(1u128),
//             token_hold_duration: 1,
//             // marketing: None,
//         };

//         println!("{:?}", init_msg);
//         println!("{:?}", mock_env().block.time);
//         let info = mock_info("juno1jx22pxvxdhpadzzjk0lcwcydgywwpyvhuw44jk", &[]);
//         let _res = instantiate(deps.as_mut(), mock_env(), info, init_msg).unwrap();

//         let mut msgs: Vec<CosmosMsg<Empty>> = Vec::new();

//         msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: "Staking".to_string(),
//             msg: to_binary(&BankMsg::Send {
//                 to_address: "juno1jx22pxvxdhpadzzjk0lcwcydgywwpyvhuw44jk".to_string(),
//                 amount: vec![Coin {
//                     amount: Uint128::from(2u128),
//                     denom: "uusd".to_string(),
//                 }],
//             })
//             .unwrap(),
//             funds: vec![],
//         }));
//         let update_reward_msg = ExecuteMsg::Propose {
//             // title: "staking".to_string(),
//             // description: "staking".to_string(),
//             // msgs,
//             // voting_period: Duration::Time(5),
//             msgs,
//             title: "test".to_string(),
//             description: "new".to_string(),
//             voting_period: Duration::Time(120),
//         };

//         println!(
//             "propose {:?}",
//             execute(
//                 deps.as_mut(),
//                 mock_env().clone(),
//                 mock_info("syne_staker1", &[]),
//                 update_reward_msg
//             )
//             .unwrap()
//         );
//         let mut new_env = mock_env_time(mock_env().block.time.plus_seconds(1u64));

//         // let update_reward_msg = ExecuteMsg::Vote {
//         //     proposal_id: 1,
//         //     vote: Vote::Yes,
//         // };

//         // println!(
//         //     "Vote {:?}",
//         //     execute(
//         //         deps.as_mut(),
//         //         new_env.clone(),
//         //         mock_info("syne_staker1", &[]),
//         //         update_reward_msg
//         //     )
//         //     .unwrap()
//         // );

//         let update_reward_msg = ExecuteMsg::Vote {
//             proposal_id: 1,
//             vote: Vote::Yes,
//         };

//         println!(
//             "Vote {:?}",
//             execute(
//                 deps.as_mut(),
//                 new_env.clone(),
//                 mock_info("syne_staker2", &[]),
//                 update_reward_msg
//             )
//             .unwrap()
//         );
//         let reward_user1: ProposalResponse = from_binary(
//             &query(
//                 deps.as_ref(),
//                 new_env.clone(),
//                 QueryMsg::Proposal { proposal_id: 1 },
//             )
//             .unwrap(),
//         )
//         .unwrap();
//         println!(" day syne_staker1 {:?}", reward_user1);
//         let mut new_env = mock_env_time(mock_env().block.time.plus_seconds(101u64));
//         let update_reward_msg = ExecuteMsg::Execute { proposal_id: 1u64 };

//         println!(
//             "Execute {:?}",
//             execute(
//                 deps.as_mut(),
//                 new_env.clone(),
//                 mock_info("syne_staker1", &[]),
//                 update_reward_msg
//             )
//             .unwrap()
//         );
//         println!("{:?}", _res)
//     }

//     // #[test]
// }
