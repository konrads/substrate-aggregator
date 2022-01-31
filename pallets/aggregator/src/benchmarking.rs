use super::*;

#[allow(unused)]
use crate::Pallet as Aggregator;
use frame_benchmarking::benchmarks; // , whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
	add_price_pair_nonexisting {
		let i in 0 .. 9;
		let sources = vec![b"BTC".to_vec(), b"ETH".to_vec(), b"DOT".to_vec(), b"ADA".to_vec(), b"USDT".to_vec(), b"CRO".to_vec(), b"BTC".to_vec(), b"BNB".to_vec(), b"ACA".to_vec(), b"KAR".to_vec()];
		let mut targets = sources.clone();
		targets.rotate_right(3);
		let source = T::Currency::from_vecu8(sources[i as usize].clone());
		let target = T::Currency::from_vecu8(targets[i as usize].clone());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());  // using existing price provider only, for most expensive scenario
	}: add_price_pair(RawOrigin::Root, source.clone(), target.clone(), provider.clone()) 
	verify {
		assert!(MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	add_price_pair_existing {
		let source = T::Currency::from_vecu8(b"ACA".to_vec());
		let target = T::Currency::from_vecu8(b"KAR".to_vec());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());
		MonitoredPairs::<T>::insert(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone() }, None::<()>);
	}: add_price_pair(RawOrigin::Root, source.clone(), target.clone(), provider.clone())
	verify {
		assert!(MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	delete_price_pair {
		let source = T::Currency::from_vecu8(b"ACA".to_vec());
		let target = T::Currency::from_vecu8(b"KAR".to_vec());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());
		MonitoredPairs::<T>::insert(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone() }, None::<()>);
	}: _(RawOrigin::Root, source.clone(), target.clone(), provider.clone())
	verify {
		assert!(! MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	// FIXME: implement below!

	submit_price_pairs {
		let source = T::Currency::from_vecu8(b"ACA".to_vec());
		let target = T::Currency::from_vecu8(b"KAR".to_vec());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());
		MonitoredPairs::<T>::insert(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone() }, None::<()>);
	}: add_price_pair(RawOrigin::Root, source.clone(), target.clone(), provider.clone())
	verify {
		assert!(MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	trade {
		let source = T::Currency::from_vecu8(b"ACA".to_vec());
		let target = T::Currency::from_vecu8(b"KAR".to_vec());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());
		MonitoredPairs::<T>::insert(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone() }, None::<()>);
	}: add_price_pair(RawOrigin::Root, source.clone(), target.clone(), provider.clone())
	verify {
		assert!(MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	ocw_submit_best_paths_changes {
		let source = T::Currency::from_vecu8(b"ACA".to_vec());
		let target = T::Currency::from_vecu8(b"KAR".to_vec());
		let provider = T::Provider::from_vecu8(b"cryptocompare".to_vec());
		MonitoredPairs::<T>::insert(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone() }, None::<()>);
	}: add_price_pair(RawOrigin::Root, source.clone(), target.clone(), provider.clone())
	verify {
		assert!(MonitoredPairs::<T>::contains_key(ProviderPair{ pair: Pair{ source: source.clone(), target: target.clone() }, provider: provider.clone()}));
	}

	impl_benchmark_test_suite!(Aggregator, crate::mock::new_test_ext(), crate::mock::Test);
}
