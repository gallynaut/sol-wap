use clap::{App, Arg};
pub struct Config {
    pub symbol: String,
    pub debug: bool,
}

impl Config {
    pub fn new() -> Result<Config, &'static str> {
        // validate command line arguements
        let matches = App::new("Serum-TWAP")
            .version("0.1.0")
            .author("Conner <ConnerNGallagher@gmail.com>")
            .about("using serum to calculate twap")
            .arg(
                Arg::with_name("symbol")
                    .help("the symbol to calculate the TWAP for (BTC/USD)")
                    .index(1)
                    .required(true),
            )
            .arg(
                Arg::with_name("debug")
                    .help("print debug information verbosely")
                    .short("d"),
            )
            .get_matches();

        let symbol = matches
            .value_of("symbol")
            .unwrap()
            .to_string()
            .to_ascii_uppercase();
        println!("{:.<20} {}", "symbol", symbol);

        let debug = matches.is_present("debug");

        Ok(Config { symbol, debug })
    }
}
