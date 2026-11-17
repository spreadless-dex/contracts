//! Oracle-driven depeg guard.
//!
//! Swaps never read an oracle. Instead, anyone may call the router's
//! `check_depeg(pool_id)`: it reads a SEP-40 oracle (Reflector) for each of the
//! pool's tokens, rejects stale or missing data, and sets the pool's protocol
//! pause when every one of the last `confirmations` readings of some token is
//! further from the peg than `max_deviation_bps`. Lifting the pause stays with
//! router governance (`unpause_pool`).

use soroban_sdk::{contractclient, panic_with_error, Env, Vec};

use crate::error::Error;
use crate::interface::{DepegGuardConfig, OracleAsset, PriceData, PriceFeed, TokenPriceSource};

/// Every oracle price is normalized to this many decimals before comparison.
pub const PRICE_DECIMALS: u32 = 14;
pub const PRICE_SCALE: i128 = 10i128.pow(PRICE_DECIMALS);
pub const BPS: i128 = 10_000;

pub const MAX_DEVIATION_BPS: u32 = 10_000;
/// Reflector keeps 24 hours of history.
pub const MAX_PRICE_AGE: u64 = 86_400;
/// Reflector returns `None` for long `prices` ranges; stay well inside them.
pub const MAX_CONFIRMATIONS: u32 = 10;
pub const MAX_QUOTES: u32 = 2;
const MAX_ORACLE_DECIMALS: u32 = 38;

/// The SEP-40 subset the guard reads.
#[contractclient(name = "PriceOracleClient")]
#[allow(dead_code)]
pub trait PriceOracle {
    fn decimals(e: Env) -> u32;
    fn lastprice(e: Env, asset: OracleAsset) -> Option<PriceData>;
    /// Most recent records first. Reflector returns one record more than
    /// requested; only the first `records` are used.
    fn prices(e: Env, asset: OracleAsset, records: u32) -> Option<Vec<PriceData>>;
}

/// Reject malformed configurations and feeds that don't answer right now.
pub fn validate_config(e: &Env, config: &DepegGuardConfig, token_count: u32) {
    let valid = config.sources.len() == token_count
        && config.peg_price > 0
        && config.max_deviation_bps > 0
        && config.max_deviation_bps <= MAX_DEVIATION_BPS
        && config.max_price_age > 0
        && config.max_price_age <= MAX_PRICE_AGE
        && config.confirmations > 0
        && config.confirmations <= MAX_CONFIRMATIONS;
    if !valid {
        panic_with_error!(e, Error::InvalidDepegGuard);
    }

    for source in config.sources.iter() {
        if source.quotes.len() > MAX_QUOTES {
            panic_with_error!(e, Error::InvalidDepegGuard);
        }
        require_feed_answers(e, &source.feed);
        for quote in source.quotes.iter() {
            require_feed_answers(e, &quote);
        }
    }
}

fn require_feed_answers(e: &Env, feed: &PriceFeed) {
    let oracle = PriceOracleClient::new(e, &feed.oracle);
    oracle_decimals(e, &oracle);
    if oracle.lastprice(&feed.asset).is_none() {
        panic_with_error!(e, Error::InvalidDepegGuard);
    }
}

/// The deviation of one token, sustained across every confirmation reading.
pub struct TokenDeviation {
    /// Most recent normalized price.
    pub price: i128,
    /// Smallest deviation among the readings, in basis points (rounded down).
    pub deviation_bps: u32,
    /// Whether every reading is strictly beyond `max_deviation_bps`.
    pub breached: bool,
}

pub fn token_deviation(
    e: &Env,
    config: &DepegGuardConfig,
    source: &TokenPriceSource,
) -> TokenDeviation {
    let n = config.confirmations;
    let readings = read_feed(e, &source.feed, n);

    let latest_age = e
        .ledger()
        .timestamp()
        .saturating_sub(readings.get_unchecked(0).timestamp);
    if latest_age > config.max_price_age {
        panic_with_error!(e, Error::StalePrice);
    }

    let mut quote_readings: Vec<Vec<PriceData>> = Vec::new(e);
    for quote in source.quotes.iter() {
        quote_readings.push_back(read_feed(e, &quote, n));
    }

    let mut latest_price = 0;
    let mut min_deviation_bps = u32::MAX;
    let mut breached = true;
    for (i, reading) in readings.iter().enumerate() {
        let mut price = reading.price;
        for quotes in quote_readings.iter() {
            let quote = quote_at(e, &quotes, reading.timestamp);
            price = price
                .checked_mul(quote)
                .map(|p| p / PRICE_SCALE)
                .unwrap_or_else(|| panic_with_error!(e, Error::PriceOverflow));
        }
        if i == 0 {
            latest_price = price;
        }

        let distance = (price - config.peg_price).abs();
        let scaled = distance
            .checked_mul(BPS)
            .unwrap_or_else(|| panic_with_error!(e, Error::PriceOverflow));
        let limit = (config.max_deviation_bps as i128) * config.peg_price;
        breached &= scaled > limit;
        let bps = u32::try_from(scaled / config.peg_price).unwrap_or(u32::MAX);
        min_deviation_bps = min_deviation_bps.min(bps);
    }

    TokenDeviation {
        price: latest_price,
        deviation_bps: min_deviation_bps,
        breached,
    }
}

/// The `n` most recent readings of a feed, normalized to `PRICE_DECIMALS`,
/// newest first. Reverts `PriceUnavailable` when the oracle has fewer than `n`
/// records, returns them out of order, or reports a non-positive price.
fn read_feed(e: &Env, feed: &PriceFeed, n: u32) -> Vec<PriceData> {
    let oracle = PriceOracleClient::new(e, &feed.oracle);
    let decimals = oracle_decimals(e, &oracle);
    let records = oracle
        .prices(&feed.asset, &n)
        .unwrap_or_else(|| panic_with_error!(e, Error::PriceUnavailable));
    if records.len() < n {
        panic_with_error!(e, Error::PriceUnavailable);
    }

    let mut readings = Vec::new(e);
    let mut previous_timestamp = u64::MAX;
    for record in records.iter().take(n as usize) {
        if record.price <= 0 || record.timestamp >= previous_timestamp {
            panic_with_error!(e, Error::PriceUnavailable);
        }
        previous_timestamp = record.timestamp;
        readings.push_back(PriceData {
            price: normalize(e, record.price, decimals),
            timestamp: record.timestamp,
        });
    }
    readings
}

/// The quote reading with exactly `timestamp`; quote and base feeds must share
/// their update slots, otherwise the conversion would mix moments in time.
fn quote_at(e: &Env, quotes: &Vec<PriceData>, timestamp: u64) -> i128 {
    quotes
        .iter()
        .find(|q| q.timestamp == timestamp)
        .map(|q| q.price)
        .unwrap_or_else(|| panic_with_error!(e, Error::PriceUnavailable))
}

fn oracle_decimals(e: &Env, oracle: &PriceOracleClient) -> u32 {
    let decimals = oracle.decimals();
    if decimals > MAX_ORACLE_DECIMALS {
        panic_with_error!(e, Error::InvalidDepegGuard);
    }
    decimals
}

fn normalize(e: &Env, price: i128, decimals: u32) -> i128 {
    if decimals >= PRICE_DECIMALS {
        price / 10i128.pow(decimals - PRICE_DECIMALS)
    } else {
        price
            .checked_mul(10i128.pow(PRICE_DECIMALS - decimals))
            .unwrap_or_else(|| panic_with_error!(e, Error::PriceOverflow))
    }
}
