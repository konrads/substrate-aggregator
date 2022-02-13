pub mod crypto_compare;
mod crypto_compare_tests;
use crate::{TradeProvider, TradeProviderErr};
use sp_std::vec::Vec;


const CRYPTOCOMPARE: &[u8] = b"cryptocompare";
const SCALE: u32 = 12;

/// Dynamic implementation of the best path calculator.
/// 
/// Re-implement as new providers are added!
pub struct TradeProviderImpl {}
impl TradeProvider<Vec<u8>, Vec<u8>, u128> for TradeProviderImpl {
    fn is_valid_provider(provider: &Vec<u8>) -> bool {
        provider == CRYPTOCOMPARE
    }

	fn get_price(provider: &Vec<u8>, source: &Vec<u8>, target: &Vec<u8>) -> Result<u128, TradeProviderErr<Vec<u8>>> {
		match provider {
			provider if provider == CRYPTOCOMPARE => crypto_compare::get_price(source, target, SCALE),
			unknown => Err(TradeProviderErr::UnknownProviderErr(unknown.clone()))
		}
	}
}
