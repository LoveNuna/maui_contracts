use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, InitResult, Querier, StdError, Storage, Uint128,
    WasmMsg,
};

use crate::collateral::{
    handle_borrow, handle_liquidiate_collateral, handle_lock_collateral, handle_unlock_collateral,
};
use crate::math::{decimal_division, decimal_subtraction};
use crate::msg::{HandleMsg, InitMsg, WhitelistResponseItem};
use crate::state::{
    read_config, read_epoch_state, read_whitelist, read_whitelist_item, store_config,
    store_epoch_state, store_whitelist_item, Config, EpochState, WhitelistItem,
};

use moneymarket::{
    deduct_tax, load_balance, load_epoch_state, CustodyHandleMsg, EpochStateResponse,
    MarketHandleMsg,
};

/// # of blocks per epoch period
const EPOCH_PERIOD: u64 = 86400u64;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> InitResult {
    store_config(
        &mut deps.storage,
        &Config {
            owner_addr: deps.api.canonical_address(&msg.owner_addr)?,
            distribution_threshold: msg.distribution_threshold,
            target_deposit_rate: msg.target_deposit_rate,
            buffer_distribution_rate: msg.buffer_distribution_rate,
            oracle_contract: deps.api.canonical_address(&msg.oracle_contract)?,
            market_contract: deps.api.canonical_address(&msg.market_contract)?,
            base_denom: msg.base_denom,
        },
    )?;

    store_epoch_state(
        &mut deps.storage,
        &EpochState {
            deposit_rate: Decimal::zero(),
            prev_a_token_supply: Uint128::zero(),
            prev_exchange_rate: Decimal::one(),
            last_executed_height: env.block.height,
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::UpdateConfig {
            owner_addr,
            distribution_threshold,
            target_deposit_rate,
            buffer_distribution_rate,
        } => update_config(
            deps,
            env,
            owner_addr,
            distribution_threshold,
            target_deposit_rate,
            buffer_distribution_rate,
        ),
        HandleMsg::Whitelist {
            collateral_token,
            custody_contract,
            ltv,
        } => register_whitelist(deps, env, collateral_token, custody_contract, ltv),
        HandleMsg::ExecuteEpochOperations {} => handle_execute_epoch_operations(deps, env),
        HandleMsg::LockCollateral { collaterals } => handle_lock_collateral(deps, env, collaterals),
        HandleMsg::UnlockCollateral { collaterals } => {
            handle_unlock_collateral(deps, env, collaterals)
        }
        HandleMsg::LiquidiateCollateral { borrower } => {
            handle_liquidiate_collateral(deps, env, borrower)
        }
        HandleMsg::Borrow { amount } => handle_borrow(deps, env, amount),
    }
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner_addr: Option<HumanAddr>,
    distribution_threshold: Option<Decimal>,
    target_deposit_rate: Option<Decimal>,
    buffer_distribution_rate: Option<Decimal>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;

    if deps.api.canonical_address(&env.message.sender)? != config.owner_addr {
        return Err(StdError::unauthorized());
    }

    if let Some(owner_addr) = owner_addr {
        config.owner_addr = deps.api.canonical_address(&owner_addr)?;
    }

    if let Some(distribution_threshold) = distribution_threshold {
        config.distribution_threshold = distribution_threshold;
    }

    if let Some(buffer_distribution_rate) = buffer_distribution_rate {
        config.buffer_distribution_rate = buffer_distribution_rate;
    }

    if let Some(target_deposit_rate) = target_deposit_rate {
        config.target_deposit_rate = target_deposit_rate;
    }

    store_config(&mut deps.storage, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "update_config")],
        data: None,
    })
}

pub fn register_whitelist<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    collateral_token: HumanAddr,
    custody_contract: HumanAddr,
    ltv: Decimal,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.owner_addr {
        return Err(StdError::unauthorized());
    }

    let collateral_token_raw = deps.api.canonical_address(&collateral_token)?;
    if read_whitelist_item(&deps.storage, &collateral_token_raw).is_ok() {
        return Err(StdError::generic_err(
            "The collateral token was already registered",
        ));
    }

    store_whitelist_item(
        &mut deps.storage,
        &collateral_token_raw,
        &WhitelistItem {
            custody_contract: deps.api.canonical_address(&custody_contract)?,
            ltv,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "register_whitelist"),
            log("collateral_token", collateral_token),
            log("custody_contract", custody_contract),
            log("LTV", ltv),
        ],
        data: None,
    })
}

pub fn handle_execute_epoch_operations<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let config: Config = read_config(&deps.storage)?;
    let state: EpochState = read_epoch_state(&deps.storage)?;
    if env.block.height < state.last_executed_height + EPOCH_PERIOD {
        return Err(StdError::generic_err("Epoch period is not passed"));
    }

    // # of blocks from the last executed height
    let blocks = Uint128::from(env.block.height - state.last_executed_height);

    // Compute next epoch state
    let market_contract: HumanAddr = deps.api.human_address(&config.market_contract)?;
    let epoch_state: EpochStateResponse = load_epoch_state(&deps, &market_contract)?;

    // effective_deposit_rate = cur_exchange_rate / prev_exchange_rate
    // deposit_rate = (effective_deposit_rate - 1) / blocks
    let effective_deposit_rate =
        decimal_division(epoch_state.exchange_rate, state.prev_exchange_rate);
    let deposit_rate = decimal_division(
        decimal_subtraction(effective_deposit_rate, Decimal::one()),
        Decimal::from_ratio(1u128, blocks),
    );

    let mut messages: Vec<CosmosMsg> = vec![];

    // Distribute Interest Buffer to depositor
    // Only executed when deposit rate < distribution_threshold
    let mut distributed_interest: Uint128 = Uint128::zero();
    if deposit_rate < config.distribution_threshold {
        // missing_deposit_rate(_per_block)
        let missing_deposit_rate = decimal_subtraction(config.distribution_threshold, deposit_rate);
        let prev_deposits = state.prev_a_token_supply * state.prev_exchange_rate;

        // missing_deposits = prev_deposits * missing_deposit_rate(_per_block) * blocks
        let missing_deposits = Uint128(prev_deposits.u128() * blocks.u128()) * missing_deposit_rate;
        let interest_buffer =
            load_balance(&deps, &env.contract.address, config.base_denom.to_string())?;
        let distribution_buffer = interest_buffer * config.buffer_distribution_rate;

        // When there was not enough deposits happens,
        // distribute interest to market contract
        distributed_interest = std::cmp::min(missing_deposits, distribution_buffer);

        // Send some portion of interest buffer to Market contract
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address,
            to_address: deps.api.human_address(&config.market_contract)?,
            amount: vec![deduct_tax(
                &deps,
                Coin {
                    denom: config.base_denom,
                    amount: distributed_interest,
                },
            )?],
        }));
    }

    // Execute market send keeper premium
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market_contract.clone(),
        send: vec![],
        msg: to_binary(&MarketHandleMsg::SendKeeperPremium {})?,
    }));

    // Execute DistributeRewards
    let whitelist: Vec<WhitelistResponseItem> = read_whitelist(&deps, None, None)?;
    for item in whitelist.iter() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: item.custody_contract.clone(),
            send: vec![],
            msg: to_binary(&CustodyHandleMsg::DistributeRewards {})?,
        }));
    }

    // update last_executed_height
    store_epoch_state(
        &mut deps.storage,
        &EpochState {
            last_executed_height: env.block.height,
            prev_exchange_rate: epoch_state.exchange_rate,
            prev_a_token_supply: epoch_state.a_token_supply,
            deposit_rate,
        },
    )?;

    return Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "epoch_operations"),
            log("distributed_interest", distributed_interest),
            log("deposit_rate", deposit_rate),
            log("exchange_rate", epoch_state.exchange_rate),
            log("a_token_supply", epoch_state.a_token_supply),
        ],
        data: None,
    });
}
