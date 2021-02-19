use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::HumanAddr;
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InitMsg {
    /// overseer contract can be registered after
    /// we register market contract to overseer contract
    // pub overseer_contract: HumanAddr,

    /// Owner address for config update
    pub owner_addr: HumanAddr,
    /// The contract has the logics for
    /// Anchor borrow interest rate
    pub interest_model: HumanAddr,
    /// The contract has the logics for
    /// ANC distribution speed
    pub distribution_model: HumanAddr,
    /// Collector contract to send all the reserve
    pub collector_contract: HumanAddr,
    /// Faucet contract to drip ANC token to users
    pub faucet_contract: HumanAddr,
    /// stable coin denom used to borrow & repay
    pub stable_denom: String,
    /// reserve ratio applied to interest
    pub reserve_factor: Decimal256,
    /// Anchor token code ID used to instantiate
    pub aterra_code_id: u64,
    /// Anchor token distribution speed
    pub anc_emission_rate: Decimal256,
    /// Maximum borrow TODO
    pub max_borrow_factor: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),

    ////////////////////
    /// Owner operations
    ////////////////////
    /// Register Overseer contract address
    RegisterOverseer {
        overseer_contract: HumanAddr,
    },

    /// (internal) Register A-token contract address
    /// A-Token will invoke this after init
    RegisterATerra {},

    /// Update config values
    UpdateConfig {
        owner_addr: Option<HumanAddr>,
        reserve_factor: Option<Decimal256>,
        interest_model: Option<HumanAddr>,
        distribution_model: Option<HumanAddr>,
    },

    ////////////////////
    /// Overseer operations
    ////////////////////
    /// Repay stable with liquidated collaterals
    RepayStableFromLiquidation {
        borrower: HumanAddr,
        prev_balance: Uint256,
    },

    /// Execute epoch operations
    /// 1. send reserve to collector contract
    /// 2. update anc_emission_rate state
    ExecuteEpochOperations {
        target_deposit_rate: Decimal256,
        deposit_rate: Decimal256,
    },

    ////////////////////
    /// User operations
    ////////////////////
    /// Deposit stable asset to get interest
    DepositStable {},

    /// Borrow stable asset with collaterals in overseer contract
    BorrowStable {
        borrow_amount: Uint256,
        to: Option<HumanAddr>,
    },

    /// Repay stable asset to decrease liability
    RepayStable {},

    /// Claim distributed ANC rewards
    ClaimRewards {
        to: Option<HumanAddr>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// Return stable coins to a user
    /// according to exchange rate
    RedeemStable {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    EpochState {
        block_height: Option<u64>,
    },
    BorrowerInfo {
        borrower: HumanAddr,
        block_height: Option<u64>,
    },
    Liabilities {
        start_after: Option<HumanAddr>,
        limit: Option<u32>,
    },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner_addr: HumanAddr,
    pub aterra_contract: HumanAddr,
    pub interest_model: HumanAddr,
    pub distribution_model: HumanAddr,
    pub overseer_contract: HumanAddr,
    pub collector_contract: HumanAddr,
    pub faucet_contract: HumanAddr,
    pub stable_denom: String,
    pub reserve_factor: Decimal256,
    pub max_borrow_factor: Decimal256,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_liabilities: Decimal256,
    pub total_reserves: Decimal256,
    pub last_interest_updated: u64,
    pub last_reward_updated: u64,
    pub global_interest_index: Decimal256,
    pub global_reward_index: Decimal256,
    pub anc_emission_rate: Decimal256,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct EpochStateResponse {
    pub exchange_rate: Decimal256,
    pub aterra_supply: Uint256,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BorrowerInfoResponse {
    pub borrower: HumanAddr,
    pub interest_index: Decimal256,
    pub reward_index: Decimal256,
    pub loan_amount: Uint256,
    pub pending_rewards: Decimal256,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BorrowerInfosResponse {
    pub borrower_infos: Vec<BorrowerInfoResponse>,
}
