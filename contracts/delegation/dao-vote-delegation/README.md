# DAO Vote Delegation

[![dao-vote-delegation on
crates.io](https://img.shields.io/crates/v/dao-vote-delegation.svg?logo=rust)](https://crates.io/crates/dao-vote-delegation)
[![docs.rs](https://img.shields.io/docsrs/dao-vote-delegation?logo=docsdotrs)](https://docs.rs/dao-vote-delegation/latest/dao_vote_delegation/)

The `dao-vote-delegation` contract allows members of a DAO to delegate their
voting power to other members of the DAO who have registered as delegates. It
works in conjunction with voting and proposal modules to offer a comprehensive
delegation system for DAOs that supports the following features:

- Fractional delegation of voting power on a per-proposal-module basis.
- Delegate votes that can be overridden on a per-proposal basis by each
  delegator.

## Instantiation and Setup

This contract must be instantiated by the DAO.

### Hooks

After instantiating the contract, it is VITAL to set up the required hooks for
it to work. To compute delegate voting power correctly, this contract needs to
know about both voting power changes and votes cast on proposals as soon as they
happen.

This can be achieved using the `add_hook` method on voting/staking contracts
that support voting power changes, such as:

- `cw4-group`
- `dao-voting-cw721-staked`
- `dao-voting-token-staked`
- `cw20-stake`

For proposal modules, the corresponding hook is `add_vote_hook`:

- `dao-proposal-single`
- `dao-proposal-multiple`
- `dao-proposal-condorcet`

## Design Decisions

### Fractional Delegation via Percentages

In order to support fractional delegation, users assign a percentage of voting
power to each delegate. Percentages are used instead of choosing an absolute
amount of voting power (e.g. staked tokens) since voting power can change
independently of delegation. If an absolute amount were used, and a user who had
delegated all of their voting power to a few different delegates then unstaked
half of their tokens, there is no clear way to resolve what their new
delegations are. Using percentages instead allows voting power and delegation to
be decided independently.

## Implementation Notes

The trickiest piece of this implementation is navigating the snapshot maps,
which are the data structures used to store historical state.

Essentially, snapshot maps (and the other historical data structures based on
snapshot maps) take 1 block to reflect updates made, but only when querying
state at a specific height (typically in the past). When using the query
functions that do not accept a height, they read the updates immediately,
including those from the same block. For example, `snapshot_map.may_load`
returns the latest map values, including those changed in the same block by an
earlier transaction; on the other hand, `snapshot_map.may_load_at_height`
returns the map values as they were at the end of the previous block (due to an
implementation detail of snapshot maps that I'm not sure was intentional).

Ideally, we would just fix this discrepancy and move on. However, many other
modules have been built using SnapshotMaps, and it is important that all modules
behave consistently with respect to this issue. For example, voting power
queries in voting modules operate in this way, with updates delayed 1
block—because of this, it is crucial that we compute and store delegated voting
power in the same way. Otherwise we risk introducing off-by-one inconsistencies
in voting power calculations. Thus, for now, we will accept this behavior and
continue.

What this means for the implementation is that we must be very careful whenever
we do pretty much anything. When performing updates at the latest block, such as
when delegating or undelegating voting power, or when handling a change in
someone's voting power (in order to propagate that change to their delegates),
we need to be sure to interact with the latest delegation and voting power
state. However, when querying information from the past, we need to match the
delayed update behavior of voting power queries.

More concretely:
- when registering/unregistering a delegate, delegating/undelegating, or
  handling voting power change hooks, we need to access the account's latest
  voting power (by querying `latest_height + 1`), even if it was updated in the
  same block. this ensures that changes to voting power right before a
  registration/delegation occurs, or voting power changes right after a
  delegation occurs, are taken into account. e.g. an account should not be able
  to get rid of all their voting power (i.e. stop being a member) and then
  become a delegate within the same block.
- when delegating/undelegating or handling voting power change hooks, in order
  to update a delegate's total delegated VP, we need to query the latest
  delegated VP, even if it was updated earlier in the same block, and then
  effectively "re-prepare" the total that will be reflected in historical
  queries starting from the next block. `snapshot_map.update` takes care of this
  automatically by loading the latest value from the same block.
- when querying information from the past, such as when querying a delegate's
  total unvoted delegated VP when they cast a vote, or when a vote cast hook is
  triggered for a delegator, we need to use historical queries that match the
  behavior of the voting module's voting power queries, i.e. delayed by 1 block.
