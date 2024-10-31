#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail
command -v shellcheck >/dev/null && shellcheck "$0"

function print_usage() {
  echo "Usage: $0 [-h|--help]"
  echo "Publishes crates to crates.io."
}

if [ $# = 1 ] && { [ "$1" = "-h" ] || [ "$1" = "--help" ] ; }
then
    print_usage
    exit 1
fi

START_DIR=$(pwd)

# Publishing cargo workspaces with cyclic dependencies is very tricky.
# Dev-dependencies are often cyclic, so we need to publish without them.
# There is ongoing discussion about this issue: https://github.com/rust-lang/cargo/issues/4242
#
# In the meantime, we are using cargo-hack to publish our crates.
# This temporarily removes dev dependencies from Cargo.toml and publishes the crate.
# https://github.com/taiki-e/cargo-hack
#
# Install cargo-hack to run this script. `cargo install cargo-hack`

# Crates must be published in the correct order, as some depend on others.
# We start with publish packages, aside from dao-testing which must be published last.

# Packages

cd packages/cw-denom
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd packages/cw-hooks
cargo publish
cd "$START_DIR"

cd packages/cw-wormhole
cargo publish
cd "$START_DIR"

cd packages/cw-stake-tracker
cargo publish
cd "$START_DIR"

cd packages/cw-paginate-storage
cargo publish
cd "$START_DIR"

cd packages/cw721-controllers
cargo publish
cd "$START_DIR"

cd packages/dao-cw721-extensions
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

sleep 120

cd packages/dao-interface
cargo publish
cd "$START_DIR"

cd packages/dao-dao-macros
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd packages/dao-pre-propose-base
cargo publish
cd "$START_DIR"

cd packages/dao-voting
cargo publish
cd "$START_DIR"

cd packages/dao-hooks
cargo publish
cd "$START_DIR"

cd packages/cw-tokenfactory-types
cargo publish
cd "$START_DIR"

# Test contracts

cd contracts/test/dao-proposal-sudo
cargo publish
cd "$START_DIR"

sleep 120

cd contracts/test/dao-voting-cw20-balance
cargo publish
cd "$START_DIR"

cd contracts/test/dao-proposal-hook-counter
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

# Contracts

cd contracts/external/cw-tokenfactory-issuer
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/test/dao-test-custom-factory
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/external/cw-token-swap
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/external/cw-vesting
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/external/cw-payroll-factory
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

sleep 120

cd contracts/external/cw721-roles
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/pre-propose/dao-pre-propose-single
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/pre-propose/dao-pre-propose-multiple
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/pre-propose/dao-pre-propose-approval-single
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/pre-propose/dao-pre-propose-approver
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/proposal/dao-proposal-single
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/proposal/dao-proposal-multiple
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

sleep 120

cd contracts/proposal/dao-proposal-condorcet
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/staking/cw20-stake
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/staking/cw20-stake-external-rewards
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/staking/cw20-stake-reward-distributor
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/voting/dao-voting-cw4
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/voting/dao-voting-cw20-staked
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/voting/dao-voting-cw721-roles
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

sleep 120

cd contracts/voting/dao-voting-cw721-staked
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/voting/dao-voting-token-staked
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/voting/dao-voting-onft-staked
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/dao-dao-core
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/external/cw-admin-factory
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

# TODO: uncomment once cleaned up and audited
# cd contracts/distribution/cw-fund-distributor
# cargo hack publish --no-dev-deps --allow-dirty
# cd "$START_DIR"

cd contracts/distribution/dao-rewards-distributor
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

sleep 120

cd contracts/external/btsg-ft-factory
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd contracts/external/dao-migrator
cargo hack publish --no-dev-deps --allow-dirty
cd "$START_DIR"

cd packages/dao-testing
cargo publish
cd "$START_DIR"

echo "Everything is published!"
