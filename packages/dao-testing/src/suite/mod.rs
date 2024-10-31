mod base;
mod cw20_suite;
mod cw4_suite;
mod cw721_suite;
mod token_suite;

pub const OWNER: &str = "owner";

pub const MEMBER1: &str = "member1";
pub const MEMBER2: &str = "member2";
pub const MEMBER3: &str = "member3";
pub const MEMBER4: &str = "member4";
pub const MEMBER5: &str = "member5";

pub const GOV_DENOM: &str = "ugovtoken";

pub use cw_multi_test::Executor;

pub use base::*;
pub use cw20_suite::*;
pub use cw4_suite::*;
pub use cw721_suite::*;
pub use token_suite::*;
