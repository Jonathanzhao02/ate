mod ownership;
mod commodity_category;
mod commodity_kind;
mod national_currency;
mod digital_asset;
mod digital_service;
mod automation_time;
mod bag_of_coins;
mod carved_coin;
mod decimal;
mod invoice;
mod country;
mod contract;
mod contract_status;
mod contract_metrics;
mod rate_card;
mod charge;
mod charge_frequency;
mod advertised_service;
mod wallet;
mod denomination;
mod historic_month;
mod historic_day;
mod historic_activity;

pub use ownership::*;
pub use commodity_category::*;
pub use commodity_kind::*;
pub use national_currency::*;
pub use digital_asset::*;
pub use digital_service::*;
pub use automation_time::*;
pub use bag_of_coins::*;
pub use carved_coin::*;
pub use decimal::*;
pub use invoice::*;
pub use country::*;
pub use contract::*;
pub use contract_status::*;
pub use contract_metrics::*;
pub use rate_card::*;
pub use charge::*;
pub use charge_frequency::*;
pub use advertised_service::*;
pub use wallet::*;
pub use denomination::*;
pub use historic_month::*;
pub use historic_day::*;
pub use historic_activity::*;

pub const WALLET_COLLECTION_ID: u64 = 2259995437953076879u64;
pub const CONTRACT_COLLECTION_ID: u64 = 8278931753731734656u64;
pub const INVOICE_COLLECTION_ID: u64 = 1234960345778345782u64;

pub const COINS_PER_STACK_TO_BE_COMBINED: usize = 10usize;