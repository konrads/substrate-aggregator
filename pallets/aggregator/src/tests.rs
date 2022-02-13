#![cfg(test)]

use crate as aggregator;
use crate::*;

use codec::Decode;
use frame_support::{assert_ok, assert_noop, parameter_types};
use parking_lot::RwLock;
use sp_core::{
	offchain::{testing, testing::PoolState, OffchainWorkerExt, OffchainDbExt, TransactionPoolExt},
	sr25519::Signature,
	H256,
};
use std::sync::Arc;
use sp_std::{collections::btree_set::BTreeSet, vec::Vec};
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BadOrigin, BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
	RuntimeAppPublic,
};


const MOCK_PROVIDER:  &[u8] = b"mock_provider";
const BTC_CURRENCY:   &[u8] = b"BTC";
const ETH_CURRENCY:   &[u8] = b"ETH";
const USDT_CURRENCY:  &[u8] = b"USDT";
const BOGUS_CURRENCY: &[u8] = b"__BOGUS_CURRENCY__";
const BOGUS_PROVIDER: &[u8] = b"__BOGUS_PROVIDER__";
pub struct MockProvider {}
impl TradeProvider<Vec<u8>, Vec<u8>, u64> for MockProvider {
    fn is_valid_provider(provider: &Vec<u8>) -> bool {
        provider == MOCK_PROVIDER
    }

	fn get_price(provider: &Vec<u8>, source: &Vec<u8>, target: &Vec<u8>) -> Result<u64, TradeProviderErr<Vec<u8>>> {
		match provider {
			p if p == MOCK_PROVIDER && source == BTC_CURRENCY && target == USDT_CURRENCY => Ok(50_000),
			unknown => Err(TradeProviderErr::UnknownProviderErr(unknown.clone()))
		}
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// For testing the module, we construct a mock runtime.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Fixture: aggregator::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = sp_core::sr25519::Public;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

type Extrinsic = TestXt<Call, ()>;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(Call, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

parameter_types! {
	pub const OffchainTriggerDelay: u64 = 1;
	pub const MaxTxPoolStayTime: u64 = 1;
	pub const UnsignedPriority: u64 = 1 << 20;
    pub const PriceChangeTolerance: u32 = 1;
}

impl Config for Test {
	type Event = Event;
	type AuthorityId = crypto::TestAuthId;
	type Call = Call;
    // aggregator specific
	type OffchainTriggerDelay = OffchainTriggerDelay;
	type MaxTxPoolStayTime = MaxTxPoolStayTime;
	type UnsignedPriority = UnsignedPriority;
    type PriceChangeTolerance = PriceChangeTolerance;
    type BestPathCalculator = best_path_calculator::noop_calculator::NoBestPathCalculator;
    type TradeProvider = MockProvider;
    type Currency = Vec<u8>;
    type Provider = Vec<u8>;
	type Amount = u64;
	type WeightInfo = ();
}

/// Return text externalities after first block, as events only get issued after the first block
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = sp_io::TestExternalities::default();
	t.execute_with(|| System::set_block_number(1));
	t
}

pub fn new_test_ext_with_keystore() -> (sp_io::TestExternalities, testing::TestOffchainExt, Arc<RwLock<PoolState>>, sp_core::sr25519::Public) {
	const PHRASE: &str = "news slush supreme milk chapter athlete soap sausage put clutch what kitten";
	let (offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let keystore = KeyStore::new();
	SyncCryptoStore::sr25519_generate_new(
		&keystore,
		crate::crypto::Public::ID,
		Some(&format!("{}/hunter1", PHRASE)),
	)
	.unwrap();
	let public_key = SyncCryptoStore::sr25519_public_keys(&keystore, crate::crypto::Public::ID)
		.get(0)
		.unwrap()
		.clone();
	let mut t = sp_io::TestExternalities::default();
	t.register_extension(OffchainWorkerExt::new(offchain.clone()));
	t.register_extension(OffchainDbExt::new(offchain.clone()));
	t.register_extension(TransactionPoolExt::new(pool));
	t.register_extension(KeystoreExt(Arc::new(keystore)));
	(t, offchain, pool_state, public_key)
}

fn last_event() -> Option<Event> {
    System::events().last().map(|e| e.event.clone())
}

#[test]
fn test_submit_monitored_pairs_ok() {
	new_test_ext().execute_with(|| {
		// validate deletion of non existent entry - should succeed, but expect no storage change/event
		assert_ok!(Fixture::submit_monitored_pairs(Origin::root(), vec![
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: BOGUS_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},
		]));
		assert_eq!(last_event(), None);
		assert_eq!(0, MonitoredPairs::<Test>::iter_keys().count());

		// validate additions
		assert_ok!(Fixture::submit_monitored_pairs(Origin::root(), vec![
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add},  // deduped, replaces the Del above
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add},
		]));
		assert_eq!(vec![
				ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()},
				ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()},
				ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()},
			].into_iter().collect::<BTreeSet<ProviderPair<Vec<u8>, Vec<u8>>>>(),
			MonitoredPairs::<Test>::iter_keys().collect::<BTreeSet<ProviderPair<Vec<u8>, Vec<u8>>>>()
		);
		assert_eq!(last_event(), Some(Event::Fixture(crate::Event::<Test>::MonitoredPairsSubmitted(vec![
			(BTC_CURRENCY.to_vec(), USDT_CURRENCY.to_vec(), MOCK_PROVIDER.to_vec(), Operation::Add),
			(BTC_CURRENCY.to_vec(), ETH_CURRENCY.to_vec(),  MOCK_PROVIDER.to_vec(), Operation::Add),
			(ETH_CURRENCY.to_vec(), USDT_CURRENCY.to_vec(), MOCK_PROVIDER.to_vec(), Operation::Add),
		]))));

		// validate deletions
		assert_ok!(Fixture::submit_monitored_pairs(Origin::root(), vec![
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},   provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},   provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},  // deduped, replaces the Add above
			// Note: ETH - USDT still remains
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(), target: BOGUS_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},  // expect it skipped in the event
		]));
		assert_eq!(vec![
				ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()},
			],
			MonitoredPairs::<Test>::iter_keys().collect::<Vec<ProviderPair<Vec<u8>, Vec<u8>>>>()
		);
		assert_eq!(last_event(), Some(Event::Fixture(crate::Event::<Test>::MonitoredPairsSubmitted(vec![
			(BTC_CURRENCY.to_vec(), USDT_CURRENCY.to_vec(), MOCK_PROVIDER.to_vec(), Operation::Del),
			(BTC_CURRENCY.to_vec(), ETH_CURRENCY.to_vec(),  MOCK_PROVIDER.to_vec(), Operation::Del),
		]))));

		// validate mixture of additions and deletions
		assert_ok!(Fixture::submit_monitored_pairs(Origin::root(), vec![
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(),   target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BOGUS_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Del}, // expect it skipped in the event
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: USDT_CURRENCY.to_vec(),  target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add},
		]));
		assert_eq!(vec![
				ProviderPair{pair: Pair{source: USDT_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()},
			],
			MonitoredPairs::<Test>::iter_keys().collect::<Vec<ProviderPair<Vec<u8>, Vec<u8>>>>()
		);
		assert_eq!(last_event(), Some(Event::Fixture(crate::Event::<Test>::MonitoredPairsSubmitted(vec![
			(ETH_CURRENCY.to_vec(),  USDT_CURRENCY.to_vec(), MOCK_PROVIDER.to_vec(), Operation::Del),
			(USDT_CURRENCY.to_vec(), ETH_CURRENCY.to_vec(),  MOCK_PROVIDER.to_vec(), Operation::Add),
		]))));
	});
}

#[test]
fn test_submit_monitored_pairs_errors() {
	new_test_ext().execute_with(|| {
		// validate incorrect origins
		assert_noop!(
			Fixture::submit_monitored_pairs(Origin::signed(Default::default()), vec![ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add}]),
			BadOrigin);
		assert_eq!(0, MonitoredPairs::<Test>::iter_keys().count());

		assert_noop!(
			Fixture::submit_monitored_pairs(Origin::none(), vec![ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, operation: Operation::Add}]),
			BadOrigin);
		assert_eq!(0, MonitoredPairs::<Test>::iter_keys().count());

		// validate invalid providers
		assert_noop!(Fixture::submit_monitored_pairs(Origin::root(), vec![
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: BOGUS_PROVIDER.to_vec()}, operation: Operation::Add},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: ETH_CURRENCY.to_vec()},  provider: MOCK_PROVIDER.to_vec()},  operation: Operation::Add},
			ProviderPairOperation{provider_pair: ProviderPair{pair: Pair{source: ETH_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()},  operation: Operation::Del}]),
			Error::<Test>::UnknownTradeProviderError
		);
		assert_eq!(0, MonitoredPairs::<Test>::iter_keys().count());
	});
}

#[test]
fn test_ocw_submit_best_paths_changes() {
	let (t, _, _, public_key) = &mut new_test_ext_with_keystore();
	let payload = BestPathChangesPayload {
		nonce: 0,
		block_number: 1,
		changes: vec![(BTC_CURRENCY.to_vec(), USDT_CURRENCY.to_vec(), Some(PricePath{total_cost: 50000, steps: vec![]}))],
		public: <Test as SigningTypes>::Public::from(*public_key),
	};
	let payload2 = payload.clone();

	t.execute_with(|| {
		let signature = 
			<BestPathChangesPayload<
				<Test as SigningTypes>::Public,
				<Test as frame_system::Config>::BlockNumber,
				<Test as Config>::Currency,
				<Test as Config>::Provider,
				<Test as Config>::Amount,
			> as SignedPayload<Test>>::sign::<crypto::TestAuthId>(&payload).unwrap();
		assert_ok!(Fixture::ocw_submit_best_paths_changes(Origin::none(), payload.clone(), signature.clone()));
		assert_noop!(Fixture::ocw_submit_best_paths_changes(Origin::none(), payload, signature), Error::<Test>::StaleUnsignedTxError);
	});

	// verify with bogus
	let (t, _, _, _) = &mut new_test_ext_with_keystore();
	t.execute_with(|| {
		let signature = 
			<BestPathChangesPayload<
				<Test as SigningTypes>::Public,
				<Test as frame_system::Config>::BlockNumber,
				<Test as Config>::Currency,
				<Test as Config>::Provider,
				<Test as Config>::Amount,
			> as SignedPayload<Test>>::sign::<crypto::TestAuthId>(&payload2).unwrap();
		assert_noop!(Fixture::ocw_submit_best_paths_changes(Origin::root(), payload2, signature), BadOrigin);
	});
}

#[test]
fn test_fetch_prices_and_update_best_paths() {
	let (t, _, pool_state, public_key) = &mut new_test_ext_with_keystore();
	let payload = BestPathChangesPayload {
		nonce: 0,
		block_number: 1,
		changes: vec![(BTC_CURRENCY.to_vec(), USDT_CURRENCY.to_vec(), Some(PricePath{total_cost: 50000, steps: vec![]}))],
		public: <Test as SigningTypes>::Public::from(*public_key),
	};

	// verify extrinsic was called
	t.execute_with(|| {
		MonitoredPairs::<Test>::insert(
			ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: MOCK_PROVIDER.to_vec()}, 
			());

		assert!(Fixture::fetch_prices_and_update_best_paths(1).is_ok());
		let tx = pool_state.write().transactions.pop().unwrap();
        assert!(pool_state.read().transactions.is_empty());
        let decoded_tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(decoded_tx.signature, None);
		if let Call::Fixture(crate::Call::ocw_submit_best_paths_changes {
			best_path_change_payload: body,
			signature,
		}) = decoded_tx.call
		{
			assert_eq!(body, payload);

			let signature_valid =
				<BestPathChangesPayload<
					<Test as SigningTypes>::Public,
					<Test as frame_system::Config>::BlockNumber,
					<Test as Config>::Currency,
					<Test as Config>::Provider,
					<Test as Config>::Amount,
				> as SignedPayload<Test>>::verify::<crypto::TestAuthId>(&payload, signature);

			assert!(signature_valid);
		}
	});

	// verify no extrinsics on bogus provider in MonitoredPairs
	let (t, _, pool_state, _) = &mut new_test_ext_with_keystore();
	t.execute_with(|| {
		MonitoredPairs::<Test>::insert(
			ProviderPair{pair: Pair{source: BTC_CURRENCY.to_vec(), target: USDT_CURRENCY.to_vec()}, provider: BOGUS_PROVIDER.to_vec()}, 
			());

		assert!(Fixture::fetch_prices_and_update_best_paths(1).is_ok());
		assert!(pool_state.write().transactions.is_empty());
	});
}

#[test]
fn test_should_trigger_offchain() {
	let (t, _, _, _) = &mut new_test_ext_with_keystore();
	t.execute_with(|| {
		StorageValueRef::persistent(NEXT_OFFCHAIN_TRIGGER_BLOCK).set(&10_u64);
		assert!(! Fixture::should_trigger_offchain(9));
		assert!(Fixture::should_trigger_offchain(10));
		assert!(Fixture::should_trigger_offchain(11));
	});
}

#[test]
fn test_trade() {
	// FIXME: add once trade is populated!!!
}