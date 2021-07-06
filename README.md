# Sol-WAP

Sol-WAP is a rust application to calculate the Time Weighted Average Price (TWAP) using Solana's Pyth oracle or Bonfida's API to fetch historical serum trades. 
## Pyth
Fetching historical data using Pyth will quickly hit the rate limits on most RPC servers. Future versions will implement an API to cache transactions and provide longer timeframes for data.
Calculating the TWAP using Pyth lets you specify the duration to fetch Pyth transactions for. Setting this to anything under 1 hour will use 1 min candles. Anything over 1 hour will use 1 hour candles.
## Serum
This application uses Bonfida's API to fetch historical Serum trades. The default is 24 hour interval with 1 hour candles.

## To-Do
Build function that can take in list of trades/oracle price feed & interval then output a list of candles. Decide whether to use another library to capture candle type (amv-dev/yata)