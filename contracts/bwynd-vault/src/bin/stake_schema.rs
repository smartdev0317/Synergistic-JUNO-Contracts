use cosmwasm_schema::write_api;
use synedex::stake::InstantiateMsg;
use bwynd_vault::msg::{ExecuteMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
    }
}
