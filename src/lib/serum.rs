use super::candles::{print_candles, CandleList, OHLC};
use arr_macro::arr;
use chrono::prelude::DateTime;
use chrono::Utc;
use core::f64;
use serde::Deserialize;
use std::process;
use std::time::Duration;
use std::time::Duration as StdDuration;
use std::time::{SystemTime, UNIX_EPOCH};
use ureq::{Agent, AgentBuilder};

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketData {
    market: String,
    price: f64,
    size: f64,
    side: String,
    time: f64,
    order_id: String,
    fee_cost: f64,
    market_address: String,
}
#[derive(Deserialize, Debug)]
pub struct MarketResponse {
    pub success: bool,
    pub data: Vec<MarketData>,
}

impl MarketResponse {
    fn is_valid(&self) -> bool {
        self.success
    }
    pub fn get_hourly_candles(&self) -> Option<[OHLC; 24]> {
        if !self.is_valid() {
            return None;
        }
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let start_us: f64 = since_the_epoch.as_millis() as f64;
        let interval_us = 3600000.0; //

        // sort responses into hourly array
        let mut candle_data: [Vec<MarketData>; 24] = Default::default();
        for o in self.data.iter() {
            let i = (start_us - o.time) / interval_us;
            let i = i as usize;
            candle_data[i].push(o.clone());
        }

        // process each hour of data and compute OHLC
        let mut candles = [OHLC::new(); 24];
        for (i, x) in candle_data.iter().enumerate() {
            if x.len() == 0 {
                continue;
            }
            // reverse order
            candles[i].open = match x.iter().last() {
                Some(i) => Some(i.price),
                None => None,
            };
            candles[i].close = match x.iter().next() {
                Some(i) => Some(i.price),
                None => None,
            };

            let mut high: Option<f64> = None;
            let mut low: Option<f64> = None;

            for y in x.iter() {
                if high == None || y.price > high.unwrap() {
                    high = Some(y.price)
                }
                if low == None || y.price < low.unwrap() {
                    low = Some(y.price)
                }
            }
            candles[i].low = low;
            candles[i].high = high;
        }
        Some(candles)
    }
}
#[derive(Deserialize, Debug)]
pub struct GetMarketsResponse {
    pub success: bool,
    pub data: Vec<String>,
}
impl GetMarketsResponse {
    fn print_markets(&self) {
        for i in self.data.iter() {
            println!(" > {}", i);
        }
    }
}

pub struct SerumData {
    pub data: Vec<MarketData>,
}
impl SerumData {
    pub fn get_candle_list(&self, start: &DateTime<Utc>) -> CandleList {
        let mut candle_data: [Vec<MarketData>; 1440] = arr![Vec::new(); 1440];
        let interval = chrono::Duration::seconds(60).num_milliseconds() as f64;
        let start = (start.timestamp() * 1000) as f64;

        for t in self.data.iter() {
            //
            // builds index and stores serum trades in vectors representing 1 min of data
            // reverses order of candle so earliest price is 0 index
            //
            let i = (start - t.time) / interval;
            let i = candle_data.len() - 1 - i as usize; // reverses the order of the candles
            candle_data[i].push(t.clone());
        }
        let mut candles = [OHLC::new(); 1440];
        //
        // ------ TBD --------
        // Need to implement logic in case prev candles close is the low/high
        //
        for (i, c) in candle_data.iter().enumerate() {
            let mut candle = make_serum_candle(c);
            if !candle.is_valid() {
                if i != 0 && candles[i - 1].is_valid() {
                    //
                    //  if no data set fields to prev candles close price
                    //
                    candle = OHLC {
                        open_time: None,
                        open: candles[i - 1].close,
                        high: candles[i - 1].close,
                        low: candles[i - 1].close,
                        close: candles[i - 1].close,
                        close_time: None,
                    }
                }
            }
            //
            // next candles open price should equal prev candles close price
            //
            if i != 0 {
                candle.open = candles[i - 1].close;
            }
            candles[i] = candle
        }
        println!("SERUM");
        // print_candles(&candles.to_vec());
        return CandleList::new(candles);
    }
}
fn make_serum_candle(trades: &Vec<MarketData>) -> OHLC {
    if trades.len() == 0 {
        return OHLC::new();
    }

    let mut open_time: Option<f64> = None;
    let mut open: Option<f64> = None;
    let mut high: Option<f64> = None;
    let mut low: Option<f64> = None;
    let mut close_time: Option<f64> = None;
    let mut close: Option<f64> = None;

    for t in trades.iter() {
        if low == None || low.unwrap() > t.price {
            low = Some(t.price)
        }
        if high == None || high.unwrap() < t.price {
            high = Some(t.price)
        }
        if open_time == None || open_time.unwrap() > t.price {
            open_time = Some(t.time);
            open = Some(t.price);
        }
        if close_time == None || close_time.unwrap() < t.price {
            close_time = Some(t.time);
            close = Some(t.price);
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
