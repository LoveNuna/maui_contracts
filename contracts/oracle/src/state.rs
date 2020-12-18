use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::PricesResponseElem;
use cosmwasm_bignumber::Decimal256;
use cosmwasm_std::{CanonicalAddr, Order, StdError, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};

static PREFIX_PRICE: &[u8] = b"price";
static KEY_CONFIG: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: CanonicalAddr,
    pub base_asset: String,
}

pub fn store_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PriceInfo {
    pub price: Decimal256,
    pub last_updated_time: u64,
}

pub fn store_price<S: Storage>(storage: &mut S, asset: &str, price: &PriceInfo) -> StdResult<()> {
    let mut price_bucket: Bucket<S, PriceInfo> = Bucket::new(PREFIX_PRICE, storage);
    price_bucket.save(asset.as_bytes(), &price)
}

pub fn read_price<S: Storage>(storage: &S, asset: &str) -> StdResult<PriceInfo> {
    let price_bucket: ReadonlyBucket<S, PriceInfo> = ReadonlyBucket::new(PREFIX_PRICE, storage);
    let res = price_bucket.load(asset.as_bytes());
    match res {
        Ok(data) => Ok(data),
        Err(_err) => Err(StdError::generic_err("no price data stored")),
    }
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;
pub fn read_prices<S: Storage>(
    storage: &S,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<PricesResponseElem>> {
    let price_bucket: ReadonlyBucket<S, PriceInfo> = ReadonlyBucket::new(PREFIX_PRICE, storage);

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = calc_range_start(start_after);

    price_bucket
        .range(start.as_deref(), None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, v) = item?;

            let asset = std::str::from_utf8(&k).unwrap().to_string();
            Ok(PricesResponseElem {
                asset,
                price: v.price,
                last_updated_time: v.last_updated_time,
            })
        })
        .collect()
}

// this will set the first key after the provided key, by appending a 1 byte
fn calc_range_start(start_after: Option<String>) -> Option<Vec<u8>> {
    start_after.map(|idx| {
        let mut v = idx.as_bytes().to_vec();
        v.push(1);
        v
    })
}
