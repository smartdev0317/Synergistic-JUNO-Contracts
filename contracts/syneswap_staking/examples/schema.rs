use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{remove_schemas};

// use syneswap::factory::MigrateMsg;
// use crate::msg::{Cw20HookMsg, ExecuteMsg, InstantiateMsg, QueryMsg};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    //     export_schema(&schema_for!(InstantiateMsg), &out_dir);
    //     export_schema(&schema_for!(ExecuteMsg), &out_dir);
    //     export_schema(&schema_for!(MigrateMsg), &out_dir);
    //     export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    //     export_schema(&schema_for!(QueryMsg), &out_dir);
    //
}
