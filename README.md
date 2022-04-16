# Juice me

## Config

The devices and price per watt hour is declared in the config file config.toml.
You need to restart the program for it to read in changes.

### Example file
```
[price]
xmr-per-kwh = 0.00129556

[monero-rpc]
host = 'localhost'
port = 18083

[[device]]
location = 'Camping#1'
host = '10.40.4.96'
switch = 3
monero = '46vp22XJf4CWcAdhXrWTW3AbgWbjairqd2pHE3Z5tMzrfq8szv1Dt7g1Pw7qj4gE87gJDJopNno6tDRcGDn8zUNg72h7eQt'

[[device]]
location = 'Camping#2'
host = '10.40.4.96'
switch = 2
monero = '84aGHMyaHbRg1rcZ9mCByuEMkAMorEqe4UCK3GFgcgTkHxQ1kJEJq6pBbHgdX1wRsRhJaZ2vbrxdoFTR7JNw7m7kMj6C1sm'
```

### How to calculate the exchange rate?

This program does not support downloading the price of Monero nor the cost of electrcity.

It is reqcommended to calculate a new price when the price of Monero or electricity changes *enough*.

For Norway this could be done like this:

1 XMR = 230 USD
1 USD = 8,83 NOK
1 kWh = 1000 Wh = 2.63 NOK

TODO: Make formula

### When is the exchange rate calculated?

For long term usage, the rate is calculated/bought at the time of payment.

When the program receive the payment it will register that the *txid* have been credited amount of Wh.

If you change the rate after the payment have been accepted by the program, then the program will use the old change rate.

## State dir

The changes are written to files append-only log-file style, each transaction have their own file.

These files can be used for debugging and are used by the program on startup.

### Structure
<timestamp> <remaining-watt-hours>

### Example 2

```
2022-04-15T20:38:32.417830635Z +0.039
2022-04-15T20:38:37.526962839Z +0.019
2022-04-15T20:38:42.607861054Z +0.000
2022-04-15T20:38:47.674914008Z -0.020
```