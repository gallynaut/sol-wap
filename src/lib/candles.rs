extern crate reqwest;
use super::pyth;
use arr_macro::arr;
use chrono::prelude::DateTime;
use chrono::Duration;
use chrono::Utc;
use core::f64;
use reqwest::Client;
use serde::Deserialize;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct OHLC {
    pub open_time: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
    pub close_time: Option<f64>,
}

impl OHLC {
    fn new() -> Self {
        Self {
            open_time: None,
            open: None,
            high: None,
            low: None,
            close: None,
            close_time: None,
        }
    }
    pub fn to_string(&self) -> String {
        if !self.is_valid() {
            return "Invalid Candle".to_string();
        }
        format!(
            "O: {:.2}, H: {:.2}, L: {:.2}, C: {:.2}",
            self.open.unwrap(),
            self.high.unwrap(),
            self.low.unwrap(),
            self.close.unwrap()
        )
    }
    pub fn is_valid(&self) -> bool {
        if self.open == None || self.high == None || self.low == None || self.close == None {
            return false;
        }
        true
    }
    fn twap(&self) -> Option<f64> {
        if !self.is_valid() {
            return None;
        }
        let twap =
            (self.open.unwrap() + self.high.unwrap() + self.low.unwrap() + self.close.unwrap())
                / 4.0;
        Some(twap)
    }
}

// 1440 to store 1min candles for 1 full day
pub struct CandleList {
    candles: [OHLC; 1440],
}
impl CandleList {
    pub fn get_1m_candles(&self) -> [OHLC; 1440] {
        self.candles
    }
    pub fn get_1hr_candles(&self) -> [OHLC; 24] {
        let count = 60;
        let mut candles_1hr: [OHLC; 24] = [OHLC::new(); 24];

        for i in 0..candles_1hr.len() {
            let rng = self.candles[i * count..(i + 1) * count].to_vec();
            candles_1hr[i] = candle_smasher(rng);
        }
        candles_1hr
    }
    pub fn get_15min_candles(&self) -> [OHLC; 96] {
        let count = 15;
        let mut candles_15min: [OHLC; 96] = [OHLC::new(); 96];

        for i in 0..candles_15min.len() {
            let rng = self.candles[i * count..(i + 1) * count].to_vec();
            candles_15min[i] = candle_smasher(rng);
        }
        candles_15min
    }
    pub fn get_candles(&self, interval: Interval) -> Vec<OHLC> {
        let (candle_count, size) = match interval {
            Interval::MIN1 => (1, 1440),
            Interval::MIN5 => (5, 288),
            Interval::MIN15 => (15, 96),
            Interval::HR1 => (60, 24),
            Interval::HR4 => (240, 6),
        };
        let mut candles = vec![OHLC::new(); size];

        for i in 0..candles.len() {
            let rng = self.candles[i * candle_count..(i + 1) * candle_count].to_vec();
            candles[i] = candle_smasher(rng);
        }

        // trim vector and only keep valid candles
        let mut final_candles: Vec<OHLC> = Vec::new();
        for (i, c) in candles.iter().enumerate() {
            if c.is_valid() {
                final_candles.push(*c)
            } else {
                // should be setting all values to previous candles close
                if i != candles.len()
                    && i != 0
                    && candles[i - 1].is_valid()
                    && candles[i + 1].is_valid()
                {
                    let prev = &candles[i + 1];
                    final_candles.push(OHLC {
                        open_time: None,
                        open: prev.close,
                        high: prev.high,
                        low: prev.low,
                        close: prev.close,
                        close_time: None,
                    })
                }
            }
        }
        final_candles
    }
}
pub enum Interval {
    MIN1,
    MIN5,
    MIN15,
    HR1,
    HR4,
}

pub fn get_pyth_candles(
    transactions: &Vec<pyth::PriceResult>,
    start: &DateTime<Utc>,
    expo: i32,
) -> CandleList {
    let mut candle_data: [Vec<pyth::PriceResult>; 1440] = arr![Vec::new(); 1440];

    // let interval_us: i64 = 60000000; // 1min in microseconds
    let interval = chrono::Duration::seconds(60).num_seconds();
    let start = start.timestamp();

    for tx in transactions.iter() {
        let i = (start - tx.block_time) / interval;
        let i = candle_data.len() - 1 - i as usize; // reverses the order of the candles
        candle_data[i].push(tx.clone()); // can change to reference
    }

    let mut candles = [OHLC::new(); 1440];
    for (i, c) in candle_data.iter().enumerate() {
        let mut candle = make_pyth_candle(c, expo);
        if !candle.is_valid() {
            if i != 0 {
                candle = candles[i - 1] // set to previous candle if no data
            }
        }
        candles[i] = candle
    }
    return CandleList { candles: candles };
}

pub fn candle_smasher(candles: Vec<OHLC>) -> OHLC {
    if candles.len() == 0 {
        return OHLC::new();
    }

    let mut open_time: Option<f64> = None;
    let mut open: Option<f64> = None;
    let mut high: Option<f64> = None;
    let mut low: Option<f64> = None;
    let mut close_time: Option<f64> = None;
    let mut close: Option<f64> = None;

    for c in candles.iter() {
        if low == None || low.unwrap() > c.low.unwrap() {
            low = c.low
        }
        if high == None || high.unwrap() < c.high.unwrap() {
            high = c.high
        }
        if open_time == None || open_time.unwrap() > c.open_time.unwrap() {
            open_time = c.open_time;
            open = c.open;
        }
        if close_time == None || close_time.unwrap() < c.close_time.unwrap() {
            close_time = c.close_time;
            close = c.close;
        }
    }

    OHLC {
        open_time: open_time,
        open: open,
        high: high,
        low: low,
        close: close,
        close_time: close_time,
    }
}

fn make_pyth_candle(transactions: &Vec<pyth::PriceResult>, expo: i32) -> OHLC {
    if transactions.len() == 0 {
        return OHLC::new();
    }

    let mut open_time: Option<i64> = None;
    let mut open: Option<i64> = None;
    let mut high: Option<i64> = None;
    let mut low: Option<i64> = None;
    let mut close_time: Option<i64> = None;
    let mut close: Option<i64> = None;

    for txn in transactions.iter() {
        if low == None || low.unwrap() > txn.price {
            low = Some(txn.price)
        }
        if high == None || high.unwrap() < txn.price {
            high = Some(txn.price)
        }
        if open_time == None || open_time.unwrap() > txn.block_time {
            open_time = Some(txn.block_time);
            open = Some(txn.price);
        }
        if close_time == None || close_time.unwrap() < txn.block_time {
            close_time = Some(txn.block_time);
            close = Some(txn.price);
        }
    }

    let base: f64 = 10.0;
    let scale_factor: f64 = base.powi(expo);

    let open_time = open_time.unwrap() as f64;
    let open_price = (open.unwrap() as f64) * scale_factor;
    let high_price = (high.unwrap() as f64) * scale_factor;
    let low_price = (low.unwrap() as f64) * scale_factor;
    let close_price = (close.unwrap() as f64) * scale_factor;
    let close_time = close_time.unwrap() as f64;

    OHLC {
        open_time: Some(open_time),
        open: Some(open_price),
        high: Some(high_price),
        low: Some(low_price),
        close: Some(close_price),
        close_time: Some(close_time),
    }
}
