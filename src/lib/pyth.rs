use super::candles::{CandleList, OHLC};
use arr_macro::arr;
use chrono::prelude::DateTime;
use chrono::Utc;
use pyth_client::{
    AccountType, Mapping, Price, PriceStatus, PriceType, Product, MAGIC, PROD_HDR_SIZE, VERSION_2,
};
use solana_program::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt;
#[repr(C)]
pub struct UpdatePriceInstruction {
    pub version: u32,
    pub cmd: i32,
    pub status: PriceStatus,
    pub unused: u32,
    pub price: i64,
    pub conf: u64,
    pub pub_slot: u64,
}
impl UpdatePriceInstruction {
    pub fn to_price_result(&self, t: i64) -> PriceResult {
        PriceResult {
            price: self.price,
            conf: self.conf,
            pub_slot: self.pub_slot,
            block_time: t,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PriceResult {
    pub price: i64,
    pub conf: u64,
    pub pub_slot: u64,
    pub block_time: i64,
}

#[derive(Default)]
pub struct ProductResult {
    pub name: String,
    pub key: Pubkey,
    pub price_accounts: [u8; 32],
}
impl fmt::Display for ProductResult {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub struct PriceAccountResult {
    pub key: Pubkey,
    pub expo: i32,
    pub twap: i64,
}

pub trait PythAccount {
    fn is_valid(&self) -> bool;
    // cast byte string into structs
    fn new<T>(d: &[u8]) -> Option<&T> {
        let (_, pxa, _) = unsafe { d.align_to::<T>() };
        if pxa.len() > 0 {
            return Some(&pxa[0]);
        } else {
            return None;
        }
    }
}
impl PythAccount for Mapping {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Mapping as u32 || self.ver != VERSION_2
        {
            return false;
        }
        true
    }
}
impl PythAccount for Product {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Product as u32 || self.ver != VERSION_2
        {
            return false;
        }
        true
    }
}
pub trait PythProduct {
    fn get_symbol(&self) -> Option<String>;
    fn decode_attributes(&self) -> Option<HashMap<String, String>>;
}

impl PythProduct for Product {
    fn get_symbol(&self) -> Option<String> {
        let attr_map = match self.decode_attributes() {
            None => return None,
            Some(i) => i,
        };
        let k = String::from("symbol");
        match attr_map.get(&k) {
            Some(i) => return Some(i.clone()),
            None => return None,
        };
    }
    fn decode_attributes(&self) -> Option<HashMap<String, String>> {
        let mut attributes = HashMap::new();
        let mut pr_attr_sz = self.size as usize - PROD_HDR_SIZE;
        let mut pr_attr_it = (&self.attr[..]).iter();
        while pr_attr_sz > 0 {
            let key = get_attr_str(&mut pr_attr_it);
            let val = get_attr_str(&mut pr_attr_it);
            pr_attr_sz -= 2 + key.len() + val.len();
            // println!("{:.<16} {}", key, val);
            attributes.insert(key, val);
        }
        Some(attributes)
    }
}

impl PythAccount for Price {
    fn is_valid(&self) -> bool {
        if self.magic != MAGIC || self.atype != AccountType::Price as u32 || self.ver != VERSION_2 {
            return false;
        }
        let _ = match &self.ptype {
            PriceType::Price => "price",
            _ => return false,
        };
        true
    }
}
impl PythAccount for UpdatePriceInstruction {
    fn is_valid(&self) -> bool {
        let _ = match &self.status {
            PriceStatus::Trading => "trading",
            _ => return false,
        };
        if self.price == 0 {
            return false;
        }
        true
    }
}

pub fn get_attr_str<'a, T>(ite: &mut T) -> String
where
    T: Iterator<Item = &'a u8>,
{
    let mut len = *ite.next().unwrap() as usize;
    let mut val = String::with_capacity(len);
    while len > 0 {
        val.push(*ite.next().unwrap() as char);
        len -= 1;
    }
    return val;
}

pub fn find_product(products: &Vec<ProductResult>, s: String) -> Option<[u8; 32]> {
    for p in products.iter() {
        if p.name == s {
            return Some(p.price_accounts);
        }
    }
    println!(
        "See {} for a list of symbols",
        "https://pyth.network/markets/"
    );
    None
}
pub struct PythData {
    pub data: Vec<PriceResult>,
}
impl PythData {
    pub fn get_pyth_candles(&self, start: &DateTime<Utc>, expo: i32) -> CandleList {
        let mut candle_data: [Vec<PriceResult>; 1440] = arr![Vec::new(); 1440];
        let interval = chrono::Duration::seconds(60).num_seconds();
        let start = start.timestamp();
        for tx in self.data.iter() {
            //
            // builds index and stores pyth transactions in vectors representing 1 min of data
            // reverses order of candle so earliest price is 0 index
            //
            let i = (start - tx.block_time) / interval;
            let i = candle_data.len() - 1 - i as usize;
            candle_data[i].push(tx.clone());
        }

        let mut candles = [OHLC::new(); 1440];
        for (i, c) in candle_data.iter().enumerate() {
            let mut candle = make_pyth_candle(c, expo);
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
            if i != 0 && candles[i - 1].close != None {
                candle.open = candles[i - 1].close;
            }
            candles[i] = candle
        }
        return CandleList::new(candles);
    }
}
fn make_pyth_candle(transactions: &Vec<PriceResult>, expo: i32) -> OHLC {
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
