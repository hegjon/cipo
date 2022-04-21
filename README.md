# Cipo

## Crypto in, power out

Cipo makes it easy to let users pay for electricity for instance for their campervan, electric cars, boat, caravan and other high load cases.

Cipo currently supports the Shelly 4PM realay and probably other Shelly relays supporting the same API.

Monero is the only platform for payment at the moment, other crypto currencies might be supported in the future.

The only user interface for end users are a QR-code per socket/outlet. This makes it possbible to pay with the wallet they already have, but makes it hard to provide feedback to the end user. In the future it might be poissble to add a screen or provide a QR-code with a webpage for status and notifications.

## Config

The devices and price per watt hour is declared in the config file config.toml.
You need to restart the program for it to read in changes.

As long as Cipo is versioned as 0.X, then the config format can change without an
upgrade path.

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

* 1 XMR = 230 USD
* 1 USD = 8,83 NOK
* 1 kWh = 1000 Wh = 2.63 NOK

TODO: Make formula

### When is the exchange rate calculated?

For long term usage, the rate is calculated/bought at the time of payment.

When the program receive the payment it will register that the *txid* have been credited amount of Wh.

If you change the rate after the payment have been accepted by the program, then the program will use the old change rate.

## State dir

As long as Cipo is versioned as 0.X, then the state format can change without an
upgrade path.

The changes are written to files append-only log-file style, each transaction have their own file.

These files can be used for debugging and are used by the program on startup.


### Folder structure
<journal-dir>/<receiving-address>/<txid>.log

### File/log structure
`<timestamp> <remaining-watt-hours>`

### Example

```
2022-04-18T18:23:15Z +3.04
2022-04-18T18:23:25Z +0.31
2022-04-18T18:23:35Z -0.04
```
