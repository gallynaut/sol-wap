#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
pub mod candles;
pub mod pyth;
pub mod serum;
use crate::pyth::{PriceResult, PythAccount, PythData, PythProduct};
use chrono::prelude::DateTime;
use chrono::Duration;
use chrono::Utc;
use pyth::PriceAccountResult;
use pyth_client::{
    AccountType, Mapping, Price, PriceStatus, PriceType, Product, MAGIC, PROD_HDR_SIZE, VERSION_2,
};
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_client::rpc_client::RpcClient;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::UiTransactionEncoding;
use std::str::FromStr;
use std::time::{Duration as StdDuration, UNIX_EPOCH};

use progress_bar::color::{Color, Style};
use progress_bar::progress_bar::ProgressBar;

use ureq::{Agent, AgentBuilder};

pub struct PythClient {
    client: RpcClient,
    mapping_key: String,
}
impl PythClient {
    pub fn new(url: String, map_key: String) -> Self {
        Self {
            client: RpcClient::new(url),
            mapping_key: map_key,
        }
    }

    // reads pyth mapping key and iterates over the products and returns a vector
    // with symbol name, public key, and associated price accounts
    pub fn get_product_accounts(&self) -> Result<Vec<pyth::ProductResult>, &'static str> {
        // mapping accounts stored as linked list so we iterate until empty
        let mut akey = Pubkey::from_str(&self.mapping_key).unwrap();

        let mut products: Vec<pyth::ProductResult> = Vec::new();

        loop {
            let map_data = match self.client.get_account_data(&akey) {
                Err(_) => return Err("not a valid pyth mapping account"),
                Ok(i) => i,
            };
            let map_acct = Mapping::new::<Mapping>(&map_data).unwrap();
            if !map_acct.is_valid() {
                return Err("not a valid pyth mapping account");
            }
            // loop over products until we find one that matches our symbol
            let mut i = 0;
            for prod_akey in &map_acct.products {
                let prod_pkey = Pubkey::new(&prod_akey.val);
                let prod_data = self.client.get_account_data(&prod_pkey).unwrap();
                let prod_acct = match Product::new::<Product>(&prod_data) {
                    Some(prod_acct) => prod_acct,
                    None => continue, // go to next loop if no product account
                };
                if !prod_acct.is_valid() {
                    continue;
                }
                // loop through reference attributes and find symbol
                let prod_attr_sym = match prod_acct.get_symbol() {
                    Some(s) => s,
                    None => continue,
                };
                // add to vector if price accounts are valid
                if prod_acct.px_acc.is_valid() {
                    products.push(pyth::ProductResult {
                        name: prod_attr_sym,
                        key: prod_pkey,
                        price_accounts: prod_acct.px_acc.val,
                    });
                }
                // go to next account if valid
                i += 1;
                if i == map_acct.num {
                    break;
                }
            }
            // go to next Mapping account in list
            if !map_acct.next.is_valid() {
                break;
            }
            akey = Pubkey::new(&map_acct.next.val);
        }
        return Ok(products);
    }

    pub fn get_price_account_data(
        &self,
        px_acct: [u8; 32],
    ) -> Result<PriceAccountResult, &'static str> {
        // check if price account is valid
        let mut price_pkey = Pubkey::new(&px_acct);
        let mut p: &Price;
        loop {
            let price_data = match self.client.get_account_data(&price_pkey) {
                Ok(price_acct) => price_acct,
                Err(_) => return Err("error getting price data"), // go to next loop if no product account
            };
            p = Price::new::<Price>(&price_data).unwrap();
            if p.is_valid() {
                return Ok(PriceAccountResult {
                    key: price_pkey,
                    expo: p.expo,
                    twap: p.twap,
                });
            }
            // go to next Mapping account in list
            if !p.next.is_valid() {
                return Err("price account not found");
            }
            price_pkey = Pubkey::new(&p.next.val);
            continue;
        }
    }
    pub fn get_historical_data(
        &self,
        px_acct: Pubkey,
        start_time: DateTime<Utc>,
        duration: Duration,
    ) -> Result<PythData, &'static str> {
        // we can request 1000 sig per req
        let mut last_sig: Option<Signature> = None;
        let mut signature_list: Vec<pyth::PriceResult> = Vec::new();

        let end_time = start_time - duration; // for debugging the interval is set low
        let duration_us = duration.num_microseconds().unwrap();

        let mut progress_bar = ProgressBar::new(100);
        progress_bar.set_action(" Progress", Color::Blue, Style::Bold);

        'process_px_acct: loop {
            let rqt_config = GetConfirmedSignaturesForAddress2Config {
                before: last_sig,
                until: None,
                limit: None,
                commitment: None,
            };

            let px_sigs = self
                .client
                .get_signatures_for_address_with_config(&px_acct, rqt_config);
            let price_account_signatures = match px_sigs {
                Ok(result) => result,
                Err(error) => {
                    println!("Rpc Err: {}", error);
                    continue;
                }
            };
            for sig in price_account_signatures {
                // check for signature error
                if let Some(_) = sig.err {
                    continue;
                };
                // check time duration
                let time = sig.block_time.unwrap() as i64;
                let block_time = utc_to_datetime(time);
                if block_time < end_time {
                    progress_bar.set_progression(100);
                    progress_bar.finalize();
                    break 'process_px_acct;
                }
                // request transaction from signature
                let s = Signature::from_str(&sig.signature).unwrap();
                last_sig = Some(s);
                let txn = match self
                    .client
                    .get_transaction(&s, UiTransactionEncoding::Base64)
                {
                    Ok(i) => i,
                    Err(e) => continue,
                };
                let txn = txn.transaction.transaction.decode().unwrap(); // transaction
                let instrs = txn.message.instructions;
                let i = &instrs.first().unwrap(); // first instruction
                let d = &i.data;

                let data =
                    match pyth::UpdatePriceInstruction::new::<pyth::UpdatePriceInstruction>(&d) {
                        None => continue, // skip value
                        Some(i) => i,     // unwrap
                    };
                // check if empty price or invalid status
                if !data.is_valid() {
                    continue;
                }
                signature_list.push(data.to_price_result(time));

                // update progress bar
                let progress_microseconds = (start_time - block_time).num_microseconds().unwrap();
                let time_progress = (100.0 * progress_microseconds as f32) / (duration_us as f32);
                progress_bar.set_progression(time_progress as usize);
            }
        }
        if signature_list.len() == 0 {
            return Err("No signatures found");
        }
        println!(""); // progress bar gets in the way
        Ok(PythData {
            data: signature_list,
        })
    }
}
pub struct SerumClient {
    pub client: Agent,
}
impl SerumClient {
    pub fn new() -> Self {
        let agent: Agent = ureq::AgentBuilder::new()
            .timeout_read(StdDuration::from_secs(5))
            .timeout_write(StdDuration::from_secs(5))
            .build();
        Self { client: agent }
    }
    pub fn get_markets(&self) -> Vec<String> {
        let response = match self
            .client
            .get("https://serum-api.bonfida.com/pairs")
            .call()
        {
            Ok(i) => i,
            Err(e) => panic!("Bonfida Err: {}", e),
        };
        let markets: serum::GetMarketsResponse = match response.into_json() {
            Ok(i) => i,
            Err(e) => panic!("Bonfida Err: {}", e),
        };
        markets.data
    }
    pub fn get_trades(&self, symbol: &String) -> Option<serum::SerumData> {
        let q = format!("https://serum-api.bonfida.com/trades/{}", symbol);
        let response = match self.client.get(&q).call() {
            Ok(i) => i,
            Err(e) => panic!("Bonfida Err: {}", e),
        };
        let trades: serum::MarketResponse = match response.into_json() {
            Ok(i) => i,
            Err(e) => panic!("Bonfida Err: {}", e),
        };
        if !trades.success {
            return None;
        }
        Some(serum::SerumData { data: trades.data })
    }
}

pub fn utc_to_datetime(t: i64) -> DateTime<Utc> {
    let t = UNIX_EPOCH + StdDuration::from_secs(t as u64);
    let t = DateTime::<Utc>::from(t);
    t
}
