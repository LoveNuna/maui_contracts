use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};

use crate::borrow::compute_interest;
use crate::math::reverse_decimal;
use crate::state::{read_config, read_state, store_state, Config, State};

use cw20::Cw20HandleMsg;
use moneymarket::deduct_tax;
use terraswap::{load_balance, load_supply};

pub fn deposit_stable<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;

    // Check base denom deposit
    let amount: Uint128 = env
        .message
        .sent_funds
        .iter()
        .find(|c| c.denom == config.base_denom)
        .map(|c| c.amount)
        .unwrap_or(Uint128::zero());

    // Cannot deposit zero amount
    if amount.is_zero() {
        return Err(StdError::generic_err("Cannot deposit zero coins"));
    }

    // Update interest related state
    let mut state: State = read_state(&deps.storage)?;
    compute_interest(deps, &env, &config, &mut state)?;
    store_state(&mut deps.storage, &state)?;

    // Load anchor token exchange rate with updated state
    let exchange_rate = compute_exchange_rate(deps, &env)?;
    let mint_amount = amount * reverse_decimal(exchange_rate);

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.anchor_token)?,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: env.message.sender,
                amount: mint_amount,
            })?,
        })],
        log: vec![],
        data: None,
    })
}

pub fn redeem_stable<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    burn_amount: Uint128,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;

    // Update interest related state
    let mut state: State = read_state(&deps.storage)?;
    compute_interest(deps, &env, &config, &mut state)?;
    store_state(&mut deps.storage, &state)?;

    // Load anchor token exchange rate with updated state
    let exchange_rate = compute_exchange_rate(deps, &env)?;
    let redeem_amount = burn_amount * exchange_rate;

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: deps.api.human_address(&config.anchor_token)?,
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Burn {
                    amount: burn_amount,
                })?,
            }),
            CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: sender,
                amount: vec![deduct_tax(
                    &deps,
                    Coin {
                        denom: config.base_denom,
                        amount: redeem_amount,
                    },
                )?],
            }),
        ],
        log: vec![
            log("action", "redeem_stable"),
            log("burn_amount", burn_amount),
            log("redeem_amount", redeem_amount),
        ],
        data: None,
    })
}

fn compute_exchange_rate<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    env: &Env,
) -> StdResult<Decimal> {
    let config: Config = read_config(&deps.storage)?;
    let state: State = read_state(&deps.storage)?;
    let anchor_token_supply = load_supply(&deps, &deps.api.human_address(&config.anchor_token)?)?;
    let balance = load_balance(&deps, &env.contract.address, config.base_denom.to_string())?;

    // (anchor_token / base_denom)
    // exchange_rate = (balance + total_liabilities - total_reserves) / anchor_token_supply
    Ok(Decimal::from_ratio(
        (balance + state.total_liabilities - state.total_reserves)?,
        anchor_token_supply,
    ))
}
