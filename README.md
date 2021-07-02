# Sol-WAP

Sol-WAP is a rust application to calculate the Time Weighted Average Price (TWAP) using Solana's Pyth oracle or Bonfida's API to fetch historical serum trades. 
## Pyth
Fetching historical data using Pyth will quickly hit the rate limits on most RPC servers. Future versions will implement an API to cache transactions and provide longer timeframes for data.