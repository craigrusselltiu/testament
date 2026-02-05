pub mod csharp;
mod trx;

pub use csharp::{build_test_name_map, TestMethodInfo};
pub use trx::{parse_trx, TestOutcome, TestResult};
