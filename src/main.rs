use chrono::{Duration, Utc};
use dialoguer::{theme::ColorfulTheme, Input, Select};
use sol_wap::candles;
use sol_wap::pyth;
use sol_wap::{PythClient, SerumClient};
use std::error::Error;
use std::process;

fn main() -> Result<(), Box<dyn Error>> {
    let twap_options = ["Pyth", "Serum"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("TWAP Option")
        .default(0)
        .items(&twap_options)
        .interact()
        .unwrap();

    match twap_options[selection] {
        "Pyth" => pyth_twap()?,
        "Serum" => serum_twap()?,
        _ => panic!("Not a valid option"),
    };
    Ok(())
}

fn pyth_twap() -> Result<(), Box<dyn Error>> {
    let networks = ["Mainnet Beta", "Devnet", "Localnet"];
    let network = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Network Selection")
        .default(1)
        .items(&networks)
        .interact()
        .unwrap();
    let pyth = match networks[network] {
        "Mainnet Beta" => {
            println!("Pyth is currently only on devnet");
            process::exit(1);
        }
        "Devnet" => {
            let url = "http://api.devnet.solana.com".to_string();
            let pyth_map_key = "BmA9Z6FjioHJPpjT39QazZyhDRUdZy2ezwx4GiDdE2u2".to_string();
            PythClient::new(url, pyth_map_key)
        }
        "Localnet" => {
            // println!("We need a pyth mapping key");
            let pyth_map_key = match Input::new()
                .with_prompt("Enter the pyth mapping key")
                .interact()
            {
                Ok(i) => i,
                _ => {
                    panic!("Error reading pyth mapping key");
                }
            };
            let url = "http://localhost".to_string();
            PythClient::new(url, pyth_map_key)
        }
        _ => panic!("Not a valid network option"),
    };

    let products = pyth.get_product_accounts()?;

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Symbol Option")
        .default(0)
        .items(&products)
        .paged(true)
        .interact()
        .unwrap();

    let px_acct = match pyth::find_product(&products, products[selection].to_string()) {
        Some(i) => i,
        None => {
            for p in products.iter() {
                println!("{:10} - {}", p.name, p.key)
            }
            panic!("Couldnt find symbol in list of pyth products");
        }
    };

    let px_data = match pyth.get_price_account_data(px_acct) {
        Ok(i) => i,
        Err(e) => panic!("Pyth Err: {}", e),
    };

    let pyth_intervals = [
        // "1 minute", // not enough data to make it worthwhile
        "5 minutes",
        "15 minutes",
        "1 hour",
        "4 hour",
        "1 day (slow)",
    ];
    let interval_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Interval to search pyth over")
        .default(0)
        .items(&pyth_intervals)
        .paged(true)
        .interact()
        .unwrap();

    let (duration, pyth_candle) = match pyth_intervals[interval_selection] {
        "1 minute" => (Duration::minutes(1), candles::Interval::MIN1),
        "5 minutes" => (Duration::minutes(5), candles::Interval::MIN1),
        "15 minutes" => (Duration::minutes(15), candles::Interval::MIN1),
        "1 hour" => (Duration::minutes(60), candles::Interval::MIN15),
        "4 hour" => (Duration::minutes(240), candles::Interval::HR1),
        "1 day (slow)" => (Duration::minutes(1440), candles::Interval::HR1),
        _ => (Duration::minutes(1), candles::Interval::MIN1),
    };

    let start_time = Utc::now();
    let historic_prices = match pyth.get_historical_data(px_data.key, start_time, duration) {
        Ok(i) => i,
        Err(e) => panic!("Pyth Err: {}", e),
    };

    let candles = historic_prices.get_pyth_candles(&start_time, px_data.expo);
    let candles_1min = candles.get_candles(&candles::Interval::MIN1);

    candles::print_candles(&candles_1min);

    let twap = candles.twap(&pyth_candle).unwrap();
    println!("TWAP: ${:.2} using {} candles", twap, &pyth_candle);
    println!("N: {} pyth transactions", historic_prices.data.len());
    let pyth_duration = Utc::now() - start_time;
    let (hrs, mins, secs) = (
        pyth_duration.num_hours(),
        pyth_duration.num_minutes() % 60,
        pyth_duration.num_seconds() % 60,
    );
    println!("Executed in {}:{}:{}", hrs, mins, secs);
    Ok(())
}

fn serum_twap() -> Result<(), Box<dyn Error>> {
    let s = SerumClient::new();

    let markets = s.get_markets()?;

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Symbol Option")
        .default(0)
        .items(&markets)
        .paged(true)
        .interact()
        .unwrap();
    let symbol = markets[selection]
        .to_ascii_uppercase()
        .replace(&['/'][..], ""); // remove backslash if provided

    let trades = match s.get_trades(&symbol) {
        Some(i) => i,
        None => panic!("failed to get trades"),
    };
    let start_time = Utc::now();
    let candles = trades.get_candle_list(&start_time);
    let candle_interval = candles::Interval::HR1;
    let candles_1hr = candles.get_candles(&candle_interval);

    println!("1 HR");
    candles::print_candles(&candles_1hr);

    let twap = candles.twap(&candle_interval).unwrap();
    println!("TWAP: ${:.2} using {} candles", twap, &candle_interval);
    println!("N: {} serum trades", trades.data.len());
    Ok(())
}
