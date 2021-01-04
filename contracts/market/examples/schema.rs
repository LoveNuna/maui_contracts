use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use moneymarket::market::{
    ConfigResponse, Cw20HookMsg, EpochStateResponse, HandleMsg, InitMsg, LiabilitiesResponse,
    LiabilityResponse, LoanAmountResponse, QueryMsg,
};
use moneymarket_market::state::State;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(State), &out_dir);
    export_schema(&schema_for!(EpochStateResponse), &out_dir);
    export_schema(&schema_for!(LiabilityResponse), &out_dir);
    export_schema(&schema_for!(LiabilitiesResponse), &out_dir);
    export_schema(&schema_for!(LoanAmountResponse), &out_dir);
}
