use super::*;

#[allow(unused)]
use crate::Pallet as Aggregator;
use frame_benchmarking::benchmarks; // , whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
	add_price_pair_new {
		let i in 0 .. 9;
		let sources = vec![b"BTC".to_vec(), b"ETH".to_vec(), b"DOT".to_vec(), b"ADA".to_vec(), b"USDT".to_vec(), b"CRO".to_vec(), b"BTC".to_vec(), b"BNB".to_vec(), b"ACA".to_vec(), b"KAR".to_vec()];
		let mut targets = sources.clone();
		targets.rotate_right(3);
		let provider = b"cryptocompare".to_vec();  // using existing price provider only, for most expensive scenario
		let source = sources[i as usize].clone();
		let target = targets[i as usize].clone();
	}: add_price_pair(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider))

	add_price_pair_existing {
		let i in 0 .. 9;
		let sources = vec![b"BTC".to_vec(), b"ETH".to_vec(), b"DOT".to_vec(), b"ADA".to_vec(), b"USDT".to_vec(), b"CRO".to_vec(), b"BTC".to_vec(), b"BNB".to_vec(), b"ACA".to_vec(), b"KAR".to_vec()];
		let mut targets = sources.clone();
		targets.rotate_right(3);
		let provider = b"cryptocompare".to_vec();  // using existing price provider only, for most expensive scenario
		let source = sources[i as usize].clone();
		let target = targets[i as usize].clone();
	}: add_price_pair(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider))

	// TDB

	// delete_price_pair__new {
	// }: delete_price_pair(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider))

	// delete_price_pair__existing {
	// }: delete_price_pair(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider))

	// submit_price_pairs__new {
	// }: submit_price_pairs(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider))

	// submit_price_pairs__resubmit {
	// }: submit_price_pairs(RawOrigin::Root, vec![(T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Provider::from_vecu8(provider), Operations::random())])

	// trade__existing {
	// }: trade(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Amount::from(amount))

	// trade__nonexisting {
	// }: trade(RawOrigin::Root, T::Currency::from_vecu8(source), T::Currency::from_vecu8(target), T::Amount::from(amount))

	// ocw_submit_best_paths_changes {
	// 			origin: OriginFor<T>,
	//		best_path_change_payload: BestPathChangesPayload<T::Public, T::BlockNumber, T::Currency, T::Provider, T::Amount>,  // FIXME: need Box<dyn PairChange>
	//		_signature: T::Signature,      // FIXME: KS: what is the signature, why needed?
	// ensure_none origin 
	// }: _()

	impl_benchmark_test_suite!(Aggregator, crate::mock::new_test_ext(), crate::mock::Test);
}
