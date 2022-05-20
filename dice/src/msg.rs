use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Decimal, Addr};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub house_contract: Option<Addr>,
    pub random_contract: Option<Addr>,
    pub fee: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Bet {
        prediction: u8,
        over: bool,
    },
    Settle {},
    Receive(Cw20ReceiveMsg),
    UpdateState {
        gov_contract: Option<Addr>,
        house_contract: Option<Addr>,
        random_contract: Option<Addr>,
        fee: Option<Decimal>,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bet {
        prediction: u8,
        over: bool,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    State {},
    Bet { address: Addr },
}