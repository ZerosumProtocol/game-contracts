#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, from_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Decimal, Uint128, Addr,
    CosmosMsg, WasmMsg, Coin};
use cw2::set_contract_version;
use zerosum::asset::{Asset, AssetInfo};
use zerosum::house::{ExecuteMsg as HouseExecuteMsg};
use zerosum::querier::{query_random};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, Cw20HookMsg};
use crate::state::{State, STATE, Bet, BETS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:dice";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        gov_contract: info.sender.clone(),
        house_contract: msg.house_contract.unwrap_or(Addr::unchecked("")),
        random_contract: msg.random_contract.unwrap_or(Addr::unchecked("")),
        fee: msg.fee.unwrap_or_default(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("fee", msg.fee.unwrap_or_default().to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateState {
            gov_contract,
            house_contract,
            random_contract,
            fee,
        } => execute_update_state(deps, info, gov_contract, house_contract, random_contract, fee),
        ExecuteMsg::Bet { prediction, over } => {
            let coin = info.funds[0].clone();
            let asset = Asset {
                info: AssetInfo::NativeToken { denom: coin.denom },
                amount: coin.amount,
            };
            execute_bet(deps, env, asset, info.sender, prediction, over)
        },
        ExecuteMsg::Settle {} => execute_settle(deps, env, info),
    }
}

pub fn receive_cw20(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bet { prediction, over }) => {
            let asset = Asset {
                info: AssetInfo::Token { contract_addr: info.sender },
                amount: cw20_msg.amount,
            };
            let player = deps.api.addr_validate(cw20_msg.sender.as_str())?;
            execute_bet(deps, env, asset, player, prediction, over)
        },
        Err(err) => Err(ContractError::Std(err))
    }
}

pub fn execute_update_state(
    deps: DepsMut,
    info: MessageInfo,
    gov_contract: Option<Addr>, 
    house_contract: Option<Addr>, 
    random_contract: Option<Addr>, 
    fee: Option<Decimal>
) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<State, ContractError> {
        if state.gov_contract != info.sender {
            return Err(ContractError::Unauthorized {});
        }
        if gov_contract.is_some() {
            state.gov_contract = gov_contract.unwrap();
        }
        if house_contract.is_some() {
            state.house_contract = house_contract.unwrap();
        }
        if random_contract.is_some() {
            state.random_contract = random_contract.unwrap();
        }
        if fee.is_some() {
            state.fee = fee.unwrap();
        }
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "update_state"))
}

pub fn execute_bet(deps: DepsMut, env: Env, bet_asset: Asset, player: Addr, prediction: u8, over: bool) -> Result<Response, ContractError> {
    let prev_bet: Option<Bet> = BETS.may_load(deps.storage, player.clone())?;
    if prev_bet.is_some() {
        if prev_bet.unwrap().result.is_none() {
            return Err(ContractError::AreadyExist {});
        };
    }
    let bet = Bet {
        player: player.clone(),
        bet_asset: bet_asset,
        prediction: prediction,
        over: over,
        block_height: env.block.height,
        lucky_number: None,
        result: None,
        prize_amount: None,
    };
    BETS.save(deps.storage, player, &bet)?;
    Ok(Response::new().add_attribute("method", "bet"))
}

pub fn execute_settle(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut msgs = vec![];
    let prev_bet = BETS.may_load(deps.storage, info.sender.clone())?;
    if prev_bet.is_none() {
        return Err(ContractError::NotExist {});
    }
    let mut bet: Bet = prev_bet.unwrap();
    if bet.block_height >= env.block.height {
        return Err(ContractError::SoFast {});
    }
    if bet.result.is_some() {
        return Err(ContractError::AlreadySettle {})
    }
    // random contract() 호출해서 해야함
    let state = STATE.load(deps.storage)?;
    let lucky_number = query_random(
        &deps.querier, 
        state.random_contract, 
        bet.block_height, 
        Some(bet.player.as_bytes().to_vec()),
        99,
    )?.expect("Random Contract Die.. T-T") as u8;

    let result: bool = if bet.over == true {
        if bet.prediction < lucky_number { true } else { false }
    } else {
        if bet.prediction > lucky_number { true } else { false }
    };

    let prize_amount: Uint128 = if result {
        if bet.over == true {
            let prize_rate =  Decimal::from_ratio(Uint128::from(100u8), Uint128::from(99u8 - bet.prediction));
            prize_rate * bet.bet_asset.amount
        } else {
            let prize_rate =  Decimal::from_ratio(Uint128::from(100u8), Uint128::from(bet.prediction));
            prize_rate * bet.bet_asset.amount
        }
    } else {
        Uint128::zero()
    };

    match &bet.bet_asset.info {
        AssetInfo::NativeToken { denom } => {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.house_contract.to_string(),
                funds: vec![Coin {
                    amount: bet.bet_asset.amount,
                    denom: denom.clone()
                }],
                msg: to_binary(&HouseExecuteMsg::Settle {
                    player: bet.player.clone(),
                    output: prize_amount,
                })?,
            }));
        },
        AssetInfo::Token { contract_addr } => {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: bet.player.to_string(),
                    amount: bet.bet_asset.amount,
                    msg: to_binary(&HouseExecuteMsg::Settle {
                            player: bet.player.clone(),
                            output: prize_amount,
                        })?,
                })?,
            }));
        }
    }

    bet.lucky_number = Some(lucky_number);
    bet.result = Some(result);
    bet.prize_amount = Some(prize_amount);

    BETS.save(deps.storage, info.sender, &bet)?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("method", "settle")
        .add_attribute("result", if result { "win" } else { "lose" })
        .add_attribute("prize", prize_amount)
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Bet { address } => to_binary(&query_bet(deps, address)?),
    }
}

fn query_state(deps: Deps) -> StdResult<State> {
    let state = STATE.load(deps.storage)?;
    Ok(state)
}

fn query_bet(deps: Deps, address: Addr) -> StdResult<Option<Bet>> {
    let bet = BETS.may_load(deps.storage, address)?;
    Ok(bet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, BlockInfo, ContractInfo, Timestamp};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fee: None, house_contract: None, random_contract: None };
        let info = mock_info("creator", &vec![]);

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap();
        let state: State = from_binary(&res).unwrap();
        assert_eq!(Decimal::zero(), state.fee);
        assert_eq!(Addr::unchecked("creator"), state.gov_contract);
    }

    #[test]
    fn update_state() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { fee: None, house_contract: None, random_contract: None };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::UpdateState {
            fee: Some(Decimal::one()),
            gov_contract: None,
            house_contract: Some(Addr::unchecked("house")),
            random_contract: Some(Addr::unchecked("random")),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::State {}).unwrap();
        let state: State = from_binary(&res).unwrap();
        assert_eq!(Decimal::one(), state.fee);
        assert_eq!(Addr::unchecked("house"), state.house_contract);
        assert_eq!(Addr::unchecked("random"), state.random_contract);
    }

    #[test]
    fn bet() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { fee: Some(Decimal::percent(5u64)), house_contract: None, random_contract: None };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("player", &coins(20000, "UST"));
        let msg = ExecuteMsg::Bet {
            prediction: 37,
            over: true,
        };
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg.clone()).unwrap();
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        match res {
            Err(_) => {},
            _ => { panic!("Must return error") }
        }
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Bet { address: Addr::unchecked("player")}).unwrap();
        let bet: Bet = from_binary(&res).unwrap();
        assert_eq!(bet.bet_asset.amount, Uint128::from(20000u128));
        assert_eq!(bet.over, true);
        assert_eq!(bet.block_height, 12345);
        assert_eq!(bet.result.is_none(), true);
        assert_eq!(bet.player, Addr::unchecked("player"));
        assert_eq!(bet.lucky_number.is_none(), true);

        let info = mock_info("player", &vec![]);
        let msg = ExecuteMsg::Settle {};
        let _res = execute(deps.as_mut(), mock_env_height(12346), info.clone(), msg.clone()).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Bet { address: Addr::unchecked("player")}).unwrap();
        let bet: Bet = from_binary(&res).unwrap();
        assert_eq!(bet.bet_asset.amount, Uint128::from(20000u128));
        assert_eq!(bet.over, true);
        assert_eq!(bet.block_height, 12345);
        assert_eq!(bet.result.is_some(), true);
        assert_eq!(bet.result.unwrap(), true);
        assert_eq!(bet.player, Addr::unchecked("player"));
        assert_eq!(bet.lucky_number.is_some(), true);

        let info = mock_info("player", &vec![]);
        let msg = ExecuteMsg::Settle {};
        let res = execute(deps.as_mut(), mock_env_height(12346), info.clone(), msg.clone());
        match res {
            Err(_) => {},
            _ => { panic!("Must return error") }
        }
    }

    pub fn mock_env_height(height: u64) -> Env {
        Env {
            block: BlockInfo {
                height: height,
                time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                chain_id: "cosmos-testnet-14002".to_string(),
            },
            contract: ContractInfo {
                address: Addr::unchecked("contract"),
            },
            transaction: None,
        }
    }

}
