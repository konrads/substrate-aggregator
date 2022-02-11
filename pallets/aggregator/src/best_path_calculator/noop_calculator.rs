use sp_std::{collections::btree_map::BTreeMap, vec};
use crate::types::*;
use crate::BestPathCalculator;

pub struct NoBestPathCalculator {}
impl<C: Currency, P: Provider, A: Amount> BestPathCalculator<C, P, A> for NoBestPathCalculator {
	fn calc_best_paths(pairs_and_prices: &[(ProviderPair<C, P>, A)]) -> Result<BTreeMap<Pair<C>, PricePath<C, P, A>>, CalculatorError> {
		Ok(pairs_and_prices.iter().cloned().map(|(pp, price)| (Pair{source: pp.pair.source, target: pp.pair.target}, PricePath{total_cost: price, steps: vec![]})).collect())
	}
}
