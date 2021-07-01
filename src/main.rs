#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]
mod config;
use chrono::prelude::DateTime;
use chrono::{Duration, Utc};
use dialoguer::{theme::ColorfulTheme, Select};
use sol_wap::candles;
use sol_wap::pyth;
use sol_wap::PythClient;
use std::process;
fn main() {
    // let c = config::Config::new().unwrap_or_else(|err| {
    //     panic!("Config Err: {:?}", err);
    //     // process::exit(1);
    // });

    let twap_options = ["Pyth", "Serum"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("TWAP Option")
        .default(0)
        .items(&twap_options)
        .interact()
        .unwrap();

    if twap_options[selection] == "Serum" {
        println!("Not implemented yet");
        process::exit(1);
    }

    // setup pyth client
    let url = "http://api.devnet.solana.com".to_string();
    let pyth_map_key = "BmA9Z6FjioHJPpjT39QazZyhDRUdZy2ezwx4GiDdE2u2".to_string();
    let pyth = PythClient::new(url, pyth_map_key);

    let products = match pyth.get_product_accounts() {
        Err(e) => panic!("Pyth Err: {}", e),
        Ok(r) => r,
    };

    // let symbol_options = ["Pyth", "Serum"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Symbol Option")
        .default(0)
        .items(&products)
        .paged(true)
        .interact()
        .unwrap();
    // println!("Selected Symbol {}", products[selection]);

    let px_acct = match pyth::find_product(&products, products[selection].to_string()) {
        Some(i) => i,
        None => {
            pyth::print_products(&products);
            panic!("Couldnt find symbol in list of pyth products");
        }
    };

    let px_data = match pyth.get_price_account_data(px_acct) {
        Ok(i) => i,
        Err(e) => panic!("Pyth Err: {}", e),
    };

    // let end_time = Utc::now() - Duration::minutes(3); // for debugging the interval is set low
    let start_time = Utc::now();
    let duration = Duration::minutes(5);
    let historic_prices = match pyth.get_historical_data(px_data.key, start_time, duration) {
        Ok(i) => i,
        Err(e) => panic!("Pyth Err: {}", e),
    };

    let candles = candles::get_pyth_candles(&historic_prices, &start_time, px_data.expo);
    let candles = candles.get_candles(candles::Interval::MIN1);

    println!("len: {}", candles.len());
    for (i, c) in candles.iter().enumerate() {
        if c.is_valid() {
            println!("{:4} - {}", i, c.to_string());
        }
    }
}
