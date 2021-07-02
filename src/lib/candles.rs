use super::pyth;
use arr_macro::arr;
use chrono::prelude::DateTime;
use chrono::Duration;
use chrono::Utc;
use core::f64;
use core::fmt;
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
    pub fn new() -> Self {
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
    pub fn new(candles: [OHLC; 1440]) -> Self {
        Self { candles }
    }
    pub fn get_candles(&self, interval: &Interval) -> Vec<OHLC> {
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
            candles[i] = self.candle_smasher(rng);
        }

        // trim vector and only keep valid candles
        let mut final_candles: Vec<OHLC> = Vec::new();
        for (i, c) in candles.iter().enumerate() {
            if c.is_valid() {
                final_candles.push(*c);
                continue;
            }
            // should be setting all values to previous candles close
            if i != 0 && candles[i - 1].is_valid() {
                let prev = &candles[i - 1];
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
        candles
    }
    fn candle_smasher(&self, candles: Vec<OHLC>) -> OHLC {
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
            if !c.is_valid() {
                continue;
            }
            if low == None || low.unwrap() > c.low.unwrap() {
                low = c.low
            }
            if high == None || high.unwrap() < c.high.unwrap() {
                high = c.high
            }
            if c.open_time != None
                && (open_time == None || open_time.unwrap() > c.open_time.unwrap())
            {
                open_time = c.open_time;
                open = c.open;
            }
            if c.close_time != None
                && (close_time == None || close_time.unwrap() < c.close_time.unwrap())
            {
                close_time = c.close_time;
                close = c.close;
            }
        }

        OHLC {
            open_time,
            open,
            high,
            low,
            close,
            close_time,
        }
    }
    pub fn twap(&self, interval: &Interval) -> Option<f64> {
        let candles = self.get_candles(&interval);
        let mut twap = 0.0;
        let mut counter = 0.0;
        for c in candles.iter() {
            if c.is_valid() {
                twap += c.twap().unwrap();
                counter += 1.0;
            }
        }
        let twap = twap / counter;
        Some(twap)
    }
}
pub enum Interval {
    MIN1,
    MIN5,
    MIN15,
    HR1,
    HR4,
}
impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Interval::MIN1 => write!(f, "1 minute"),
            Interval::MIN5 => write!(f, "5 minute"),
            Interval::MIN15 => write!(f, "15 minute"),
            Interval::HR1 => write!(f, "1 hour"),
            Interval::HR4 => write!(f, "4 hour"),
        }
    }
}

pub fn print_candles(candles: &Vec<OHLC>) {
    for (i, c) in candles.iter().enumerate() {
        if c.is_valid() {
            println!("{:4} - {}", i, c.to_string());
        }
    }
}
