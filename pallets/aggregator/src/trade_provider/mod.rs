pub mod crypto_compare;
mod crypto_compare_tests;
use crate::{types::*, TradeProvider, TradeProviderErr};
use sp_std::convert::AsRef;


const SCALE: u32 = 12;

/// Dynamic implementation of the best path calculator.
/// 
/// Re-implement as new providers are added!
pub struct TradeProviderImpl {}
impl TradeProvider<u128> for TradeProviderImpl {
	fn get_price<C: AsRef<[u8]>>(provider: &Provider, source: C, target: C) -> Result<u128, TradeProviderErr> {
		match provider {
			Provider::CRYPTOCOMPARE => crypto_compare::get_price(source.as_ref(), target.as_ref(), SCALE),
		}
	}
}
