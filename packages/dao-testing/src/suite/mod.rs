mod base;
mod cw20_suite;
mod cw4_suite;
mod cw721_suite;
mod token_suite;

pub const OWNER: &str = "owner";

pub const ADDR0: &str = "addr0";
pub const ADDR1: &str = "addr1";
pub const ADDR2: &str = "addr2";
pub const ADDR3: &str = "addr3";
pub const ADDR4: &str = "addr4";

pub const GOV_DENOM: &str = "ugovtoken";

pub use cw_multi_test::Executor;

pub use base::*;
pub use cw20_suite::*;
pub use cw4_suite::*;
pub use cw721_suite::*;
pub use token_suite::*;
