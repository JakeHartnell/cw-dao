# Multi choice proposal approval contract

[![dao-pre-propose-approval-multiple on crates.io](https://img.shields.io/crates/v/dao-pre-propose-approval-multiple.svg?logo=rust)](https://crates.io/crates/dao-pre-propose-approval-multiple)
[![docs.rs](https://img.shields.io/docsrs/dao-pre-propose-approval-multiple?logo=docsdotrs)](https://docs.rs/dao-pre-propose-approval-multiple/latest/dao_pre_propose_approval_multiple/)

This contract implements an approval flow for proposals, it also handles deposit logic. It works with the `dao-proposal-multiple` proposal module.

## Approval Logic

This contract is instantatied with an `approver` address. This address is
allowed to approve or reject the proposal. An approved proposal opens for voting
immediately, whereas a rejected proposal is simply discarded.

```text
      ┌──────────┐
      │          │
      │  Account │
      │          │
      └─────┬────┘
            │
            │ Makes prop
            ▼
┌────────────────────────┐               ┌────────────────────────┐
│                        │               │                        │
│  Pre-propose Approval  │ ◄─────────────┤    Approver Address    │
│                        │    Approves   │                        │
└───────────┬────────────┘    or rejects └────────────────────────┘
            │
            │ Creates prop
            │ on approval
            ▼
┌──────────────────────────┐
│                          │
│     Proposal Multiple    │
│                          │
└───────────┬──────────────┘
            │
            │ Normal voting
            │
            ▼
┌────────────────────────┐
│                        │
│       Main DAO         │
│                        │
└────────────────────────┘
```

The `approver` may also register a `ProposalSubmitHook`, which fires every time a proposal is submitted to the `dao-pre-propose-approval-multiple` contract.

## Deposit Logic

It may accept either native ([bank
module](https://docs.cosmos.network/main/modules/bank/)),
[cw20](https://github.com/CosmWasm/cw-plus/tree/bc339368b1ee33c97c55a19d4cff983c7708ce36/packages/cw20)
tokens, or no tokens as a deposit. If a proposal deposit is enabled
the following refund strategies are avaliable:

1. Never refund deposits. All deposits are sent to the DAO on proposal
   completion.
2. Always refund deposits. Deposits are returned to the proposer on
   proposal completion and even rejection by the `approver`.
3. Only refund passed proposals. Deposits are only returned to the
   proposer if the proposal is approved and passes. Otherwise, they
   are sent to the DAO.

This module may also be configured to only accept proposals from
members (addresses with voting power) of the DAO.

Here is a flowchart showing the proposal creation process using this
module:

![](https://bafkreig42cxswefi2ks7vhrwyvkcnumbnwdk7ov643yaafm7loi6vh2gja.ipfs.nftstorage.link)

### Resources

More about the [pre-propose design](https://github.com/DA0-DA0/dao-contracts/wiki/Pre-propose-module-design).

More about [pre-propose modules](https://github.com/DA0-DA0/dao-contracts/wiki/DAO-DAO-Contracts-Design#pre-propose-modules).
