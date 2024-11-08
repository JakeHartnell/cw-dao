# DAO Contracts

[![GitHub tag (with filter)](https://img.shields.io/github/v/tag/DA0-DA0/dao-contracts?label=Latest%20version&logo=github)](https://github.com/DA0-DA0/dao-contracts/releases/latest)
[![GitHub contributors](https://img.shields.io/github/contributors/DA0-DA0/dao-contracts?logo=github)](https://github.com/DA0-DA0/dao-contracts/graphs/contributors)

[![GitHub commit activity (branch)](https://img.shields.io/github/commit-activity/m/DA0-DA0/dao-contracts?logo=git)](https://github.com/DA0-DA0/dao-contracts/pulse/monthly)
[![codecov](https://codecov.io/gh/DA0-DA0/dao-contracts/branch/main/graph/badge.svg?token=SCKOIPYZPV)](https://codecov.io/gh/DA0-DA0/dao-contracts)

[![Discord](https://img.shields.io/discord/895922260047720449?logo=discord&label=Discord)](https://discord.gg/MUBxdbwJDD)
[![X (formerly Twitter) URL](https://img.shields.io/twitter/url?url=https%3A%2F%2Ftwitter.com%2FDA0_DA0&label=DA0_DA0)](https://x.com/DA0_DA0)

[![DAO DAO DAO](https://img.shields.io/badge/DAO%20DAO%20DAO-gray?logo=data%3Aimage%2Fpng%3Bbase64%2CiVBORw0KGgoAAAANSUhEUgAAAHAAAABwCAMAAADxPgR5AAAABGdBTUEAALGPC%2FxhBQAAAAFzUkdCAK7OHOkAAAAgY0hSTQAAeiYAAICEAAD6AAAAgOgAAHUwAADqYAAAOpgAABdwnLpRPAAAAR1QTFRF%2F%2F%2F%2F7%2FDw0NDRoaOjgoSFY2VmREdIJSgpFhgaBgkLNTc4sbKz0dHRREZINTc54ODgwMHCwMLCJSgpREZIoqOjNTc5VFZXoqOksbKz0dHRwcHCkpOUsbKyoqKjJikqc3V2g4SFoaGiwMDB7%2FDwoaKjFxkb39%2FfkZKTFhgaY2VnZGZnZGVn7u%2FvgoOEdHZ3BwoM%2Fv7%2BwcLCRUhJNjg5NTc4gYOENTg5cnR2c3V3sLGy7%2B%2Fv0NHR0NDRU1ZXoaOkgoSFY2VmREdIz9DQJSgqoaKiFhkaBgkL3%2BDgv8DBNDc5z8%2FQsLKzVVdY7u7uwMHBkpSUFxobZWZodHV3sbGyc3R2Njg6RUdJoKKiv8HBr7GykZOT3t%2Ff%2F%2F%2F%2FcnR1oaOjFOTQHAAAAA90Uk5T%2Fv7%2B%2Fv7%2B%2Fv7%2B%2Fv7%2B%2Fv7%2B6a2FXwAABrRJREFUaN7Vm%2BtjmzYQwNnWx7ZuUwq1jU0Dbp04LM0CTWu3pWnBdXHjuY9kXV8b5P%2F%2FM4aFAElIIAH5sPsWk%2BjnO%2B4hnS5KJCPe6Us%2FWCyX4SqRcLleBP7mVGoFRRGHvYzXK6asg0vQNXDH58ByaHy7O6DnhysBUWPQCXBHWwnLYtMaeKdHrLjsxwN9CDyoOBiNjFhTSTUvWwHv4qv1Yt1jWlyPe8LISiBuTM3wqtYBu9jvmqAR0LOKLz326t0B3CvMEQB54E7%2B59pINMh0rdauPGChnjhuK%2FdzpDWRAQK1EQ4i1co3yQbuoUAPjaiBZO9yXxcF%2Bpl6IGok0wAt4IsBrTbqpXJg84gM4O%2FIz0DUQqbIrIf1QMR74EWt5KjPJpaAiDeOWssfTCIN9Dvj5cTjKiDiGVHUIdHnA%2B92qB90Vrics8cDgrBbXqbjPmADPbVrXkY0J0xgGvD9LnmR%2B4ByHAx4J413r1NgdATN5jwsA9MCEYKoY5nahFELYNBlQJRd9ZgGgnRr0D0vcjXcqDlQbZ%2Bwq416QgJTj9mLrkSgUR2dAKYKXg0PGdXEgamCD68IGN2HKl5iQKjg4qp4SMVHBVC%2FWgUzFR%2FmQI2toLeJ%2B9vDbrhexBuvvYonGTCNQb10siBPhf1NaxUnCGgxXPSuyjhxXjZXUUWlGALVUgwCziH0cePMMIOFMQXuwLXwpQzuEdtummyPkNtsgRbtMn7Vqdpv4zY%2BBM4pi%2FrV53i%2FRX57tAUCyqJ%2BXefAb2HTSQKEae1x8f7qexWN3qM7h%2BktAcLK%2B4TauVWK3cRXXegqTxPgnIj6nlA7pomKz%2BBLTIBwiQmxk6oVvfFL%2FE55TqQZVQzYpHK6yy3we0XHbaSL9rgaVBb3xRb4g%2BLjPvNCuKnW1GuuKQHm6BPhLp49kSfubf%2FwuqJhFipbNByPomgYCLmN5ydrrS%2Br3fSGMseAVsk5ANW3yeUJ91jP7bS92j69qSyxxFYqSvnfApt60mdXPOqsRH6jFGhjYTinVg34Gb0UGK9rky0MxB8V%2BCvoI1oNrGE%2For2GlSqRnHDcdPvsJwJIWxT%2F9apn2WpInP8PEMsnz%2BlnE1ZFT%2BURRZoQQNxpbH6pDbgOjO%2BSyl0L6OKE0%2BBhseQWPlAb%2BUdFWNC2fEOExRJzx3kp8KcuEdRVZZ8f%2BGj1s%2B3Tn9PUpnPsloTin657NmNsAxjdjntJ9dYY%2FfgBBryRJu9LTmrrJn0b2L7tOlGeBhJA8ZKo60R5GmD2eS4DPBEuvKdF4Di3yC2GLQF0TgV7CvtYdk%2B2GBM8jDUZFd%2BKKRic4JsoapvoywAdobd45hjUNtHCvGYkA%2BQWPlzeqcj0qc8cJsABvtW3pYjHIjsnE9%2Fq7yVA4iVaUsD6c80s77LDxOaA%2FLhmNLFpLXFWOPPr%2FLiWekq20dS6JM6KWpWWr6cQOMJtakgCedd2W3%2BxsFo1xY7caVVCX3ViyxJNTgI4U%2FFa9Rr9AIE%2Bnmz8lbSwLmBfaUQxTjeRTxEw9dP3TVUsN428M40qxs9SH2W2voxVEwn7H26fJqXQe7P50A%2Fp3QbV%2BkJu876ho1a93wirvbBFi9qXc1zFUWc8B5Q6tAi4S6hodQUcEwru4S1oE1dxYnZrUFcttaDRyfAg2xTud8E7B3gMIv%2FJrxGgle0p%2Bmm3ixdoEPtHk7q3GJFH93F3L9ANcP8proIswqjRRVveX%2BRV0GHp7in1lNyobYkX%2BR4qJLbj2HVealT1qBPiRbHFIM83VReWxx3w3D5pXwKIwm%2BWfzB2WvpLdogzPc6l8z5FNBrF4%2FkuxTsHvGt1FH4FETTIOdhxLeU5H%2FmDA2OaKG%2FWvz36UDyuGo04LhGBKYPEJ4xYvPLwBwqGmYsZWhhJjODMKIflADNi%2FyiSRYb44fddn8ljDfBcZMd7wp0%2B1SHJSbvswE%2FzmCNKyHPsAxf%2FdHhoOlxoGJPTYQchlVArgblnBlPy86Hx%2BbwMDbUxNYs21VbcsRX2mJmOAl794tKPhrr%2F9nM2stsL4sGwfOkTlhJAHbAIePWrK9loLrakEoN0SV49zky3kELmODwBiAC3oZB38L%2B50jiTN2JYMe4JDnP%2FUL9M65mer65q1ItqBlrxrLZImBVQ74PGzm9SQCrFrONv71wG1dsQA%2BDmx6oVa4eSqaymLuJ%2Fvn79Nx1KfnO6eRn3yZ7numaARGDsWv%2FsNCoWjYHJu7SEkvdSZJJYdHR%2BZH1yKqG9sdisrcQ%2FB4DdwwTKovbigfCkhiIpv9y6dv3X37JcutQCfzCUGgtR%2FgO61zuwRnnviwAAAABJRU5ErkJggg%3D%3D)](https://daodao.zone/dao/juno10h0hc64jv006rr8qy0zhlu4jsxct8qwa0vtaleayh0ujz0zynf2s2r7v8q)

This is a collection of smart contracts for building composable, modular, and upgradable DAOs.

For a detailed look at how these contracts work, see [our wiki](https://github.com/DA0-DA0/dao-contracts/wiki/DAO-DAO-Contracts-Design).

## Overview

Every DAO is made up of three modules:

1. A voting power module, which manages the voting power of DAO members.
2. Any number of proposal modules, which manage proposals in the DAO.
3. A core module, which holds the DAO treasury.

![image](https://user-images.githubusercontent.com/30676292/220181882-737c4dd3-a85d-498c-a1f2-067b317418a9.png)

For example, voting power might be based on [staked governance tokens](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/voting/dao-voting-cw20-staked), [staked NFTs](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/voting/dao-voting-cw721-staked), or [membership](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/voting/dao-voting-cw4) and proposal modules might implement [yes/no](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/proposal/dao-proposal-single), [multiple-choice](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/proposal/dao-proposal-multiple), or [ranked-choice](https://github.com/DA0-DA0/dao-contracts/tree/main/contracts/proposal/dao-proposal-condorcet) voting.

Each module type has a [standard interface](https://github.com/DA0-DA0/dao-contracts/wiki/DAO-DAO-Contracts-Design). As a result, any voting module can be used with any proposal module, and any proposal module with any voting module.

The best way to get started is to create a DAO! We maintain an [open source](https://github.com/DA0-DA0/dao-dao-ui) frontend you can find at [daodao.zone](https://daodao.zone).

## Audits

If you believe you have found a problem, please [let us know](SECURITY.md).

DAO DAO has been audited by [Oak Security](https://www.oaksecurity.io/) on multiple occasions. You can find all the audit reports [here](https://github.com/oak-security/audit-reports/tree/master/DAO%20DAO).

## Why?

Our institutions grew rapidly after 1970, but as time passed their priorities shifted from growth, to protectionism. We're fighting this. We believe The Internet is where the organizations of tomorrow will be built.

DAO DAO is a global community working on Internet governance, and [a real DAO](https://daodao.zone/dao/juno10h0hc64jv006rr8qy0zhlu4jsxct8qwa0vtaleayh0ujz0zynf2s2r7v8q#proposals). We've never raised money, and all our work is open-source. We hope you'll [join us](https://discord.gg/sAaGuyW3D2).

## Links and Resources

- [DAO DAO DAO](https://daodao.zone/dao/juno10h0hc64jv006rr8qy0zhlu4jsxct8qwa0vtaleayh0ujz0zynf2s2r7v8q)
- [Discord](https://discord.gg/sAaGuyW3D2)
- [Docs](https://docs.daodao.zone)
- [Manually Instantiating a DAO](https://github.com/DA0-DA0/dao-contracts/wiki/Instantiating-a-DAO)
- [X (formerly Twitter)](https://x.com/DA0_DA0)
- [What is a DAO?](https://docs.daodao.zone/docs/introduction/what-is-dao)

## Developers

Information about our development workflow and how to contribute can be found in [CONTRIBUTING.md](./CONTRIBUTING.md).

## Testing

### Unit tests

Run `cargo test`, or `just test` from the project root to run the unit tests.

### Integration tests

Run `just bootstrap-dev` to spin up a local environment and `just integration-test-dev` to run tests against it.

See [ci/integration-tests/README.md](ci/integration-tests/README.md) for more information.

## Disclaimer

DAO DAO TOOLING IS PROVIDED “AS IS”, AT YOUR OWN RISK, AND WITHOUT
WARRANTIES OF ANY KIND. No developer or entity involved in creating
the DAO DAO UI or smart contracts will be liable for any claims or
damages whatsoever associated with your use, inability to use, or your
interaction with other users of DAO DAO tooling, including any direct,
indirect, incidental, special, exemplary, punitive or consequential
damages, or loss of profits, cryptocurrencies, tokens, or anything
else of value.
