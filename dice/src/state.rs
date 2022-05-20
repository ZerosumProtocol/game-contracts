use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

use zerosum::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub gov_contract: Addr,
    pub house_contract: Addr,
    pub random_contract: Addr,
    pub fee: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Bet {
    pub player: Addr,
    pub bet_asset: Asset,
    pub prediction: u8,
    pub over: bool,
    pub block_height: u64,
    pub lucky_number: Option<u8>,
    pub result: Option<bool>,
    pub prize_amount: Option<Uint128>, 
}

pub const BETS: Map<Addr, Bet> = Map::new("bets");
pub const STATE: Item<State> = Item::new("state");
