#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]

use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_system::{
	self,
	offchain::{AppCrypto, CreateSignedTransaction, SendUnsignedTransaction, SignedPayload, Signer, SigningTypes},
};
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
	offchain::{storage::{MutateStorageError, StorageRetrievalError, StorageValueRef}},
	transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
	offchain::http,
	RuntimeDebug,
};
use sp_std::{collections::btree_map::BTreeMap, vec::Vec, str, prelude::*};
mod types;
use types::*;
mod utils;
use utils::*;
pub mod heap;
pub mod trade_provider;
pub mod best_path_calculator;
use sp_std::convert::TryInto;
use scale_info::prelude::{string::String, format};

#[cfg(test)]
mod tests;

/// Key for the next offchain trigger.
pub const NEXT_OFFCHAIN_TRIGGER_STORAGE: &[u8] = b"aggregator::next_offchain_trigger";

/// Defines application identifier for crypto keys of this module.
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"aggr");

pub trait BestPathCalculator<C: Currency, P: Provider, A: Amount> {
	fn calc_best_paths(pairs_and_prices: Vec<(ProviderPair<C, P>, A)>) -> Result<BTreeMap<Pair<C>, PricePath<C, P, A>>, CalculatorError>;
}

#[derive(Debug)]
pub enum TradeProviderErr<P: Provider> {
	TransportErr(http::Error),
	UnknownProviderErr(P),
}

impl <P: Provider> From<http::Error> for TradeProviderErr<P> {
    fn from(err: http::Error) -> Self {
        TradeProviderErr::TransportErr(err)
    }
}

pub trait TradeProvider<C: Currency, P: Provider, A: Amount> {
	fn is_valid_provider(provider: P) -> bool;
	fn get_price(provider: P, source: C, target: C) -> Result<A, TradeProviderErr<P>>;
	// fn trade(source: provider: Provider, Cu, target: Cu, amount: u128, cost: Cost) -> Option<Cost>;
}

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Defines the block when next unsigned transaction will be accepted.
	///
	/// To prevent spam of unsigned (and unpayed!) transactions on the network,
	/// we only allow one transaction every `T::UnsignedInterval` blocks.
	/// This storage entry defines when new transaction is going to be accepted.
	#[pallet::storage]
	//#[pallet::getter(fn next_unsigned_at)]
	pub(super) type AcceptNextOcwTxAt<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	/// DoubleMap of source/target currencies => cost by provider
	#[pallet::storage]
	// #[pallet::getter(fn best_path)]  // not using getter only
	pub(super) type BestPaths<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Currency,  // source currency
		Blake2_128Concat,
		T::Currency,  // target currency
		PricePath<T::Currency, T::Provider, T::Amount>, // best path
	>;

	#[pallet::storage]
	// #[pallet::getter(fn best_path)]  // not using getter only
	pub(super) type MonitoredPairs<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ProviderPair<T::Currency, T::Provider>,
		Option<()>,  // membership in the map indicates price is to be fetched, Some(()) - existance of the latest price
	>;

	/// Events for the pallet.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Addition/change of a price pair.
		/// \[source_currency, target_currency, new_cost\]
		PricePairChanged(T::Currency, T::Currency, T::Amount),

		/// Multiple price changes
		/// \[{source_currency, target_currency, new_cost, operation}\]
		MultiplePricePairsChanged(Vec<(T::Currency, T::Currency, T::Amount, Operation)>),

		/// Removal of a price pair.
		/// \[source_currency, target_currency\]
		PricePairRemoved(T::Currency, T::Currency),

		/// Addition/removal of a monitored pair.
		/// \[source_currency, target_currency, provider\]
		MonitoredPairAdded(T::Currency, T::Currency, T::Provider),

		/// Addition/removal of a monitored pair.
		/// \[source_currency, target_currency, provider\]
		MonitoredPairRemoved(T::Currency, T::Currency, T::Provider),

		/// Confirmation of a trade.
		/// \[owner_account_id, source_currency, target_currency, cost, source_amount, target_amount\]
		TradePerformed(T::AccountId, T::Currency, T::Currency, T::Amount, T::Amount, T::Amount),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		PricePairNotFoundError,
		UnknownTradeProviderError,
	}
	
	/// This pallet's configuration trait
	#[pallet::config]
	pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching dispatch call type.
		type Call: From<Call<Self>>;

		type Currency: Currency;

		type Provider: Provider;

		type Amount: Amount;

		/// Dynamic implementation of the best path calculator
		type BestPathCalculator: BestPathCalculator<Self::Currency, Self::Provider, Self::Amount>;

		/// Dynamic implementation of the trade provider
		type TradeProvider: TradeProvider<Self::Currency, Self::Provider, Self::Amount>;

		// Configuration parameters

		/// Frequency of offchain worker trigger.
		///
		/// To avoid sending too many offchain worker instantiations, we only attempt to trigger one
		/// every `OffchainTriggerDelay` blocks. We use Local Storage to coordinate
		/// sending between distinct runs.
		#[pallet::constant]
		type OffchainTriggerFreq: Get<Self::BlockNumber>;

		/// Frequency of accepted unsigned (OCW) transactions.
		///
		/// This ensures that we only accept unsigned transactions once every `UnsignedTxAcceptFreq` blocks.
		#[pallet::constant]
		type UnsignedTxAcceptFreq: Get<Self::BlockNumber>;

		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple pallets send unsigned transactions.
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		/// Tolerance of price change in best paths, expressed in 1/1,000,000, to prevent triggering extrinsics for minor price changes
		#[pallet::constant]
		type PriceChangeTolerance: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Offchain Worker entry point, keeping functionality to minimum and delegating to `impl` block.
		fn offchain_worker(block_number: T::BlockNumber) {
			if Self::should_trigger_offchain(block_number) {
				if let Err(e) = Self::fetch_prices_and_update_best_paths(block_number) {
					log::error!("OCW price fetching error: {}", e);
				}
			}
		}
	}

	/// A public part of the pallet.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn ocw_submit_best_paths_changes(
			origin: OriginFor<T>,
			best_path_change_payload: BestPathChangesPayload<T::Public, T::BlockNumber, T::Currency, T::Provider, T::Amount>,  // FIXME: need Box<dyn PairChange>
			_signature: T::Signature,      // FIXME: KS: what is the signature, why needed?
		) -> DispatchResultWithPostInfo {  // FIXME: KS: DispatchResultWithPostInfo to reduce the fee? how?
			ensure_none(origin)?;

			let mut event_payload = vec![];
			for (ref source, ref target, ref new_path) in best_path_change_payload.changes {
				BestPaths::<T>::mutate_exists(
					source,
					target,
					|old_path| {
						match new_path {
							Some(path) => {
								*old_path = Some(path.clone());  // insert/replace
								log::info!("Onchain: adding/changing price onchain for {} -> {}: {:?}", source.as_str(), target.as_str(), path.total_cost);
								event_payload.push((source.clone(), target.clone(), path.total_cost, Operation::Add));
							}
							None => if old_path.take().is_some() {
								log::info!("Onchain: removing price onchain: {} -> {}", source.as_str(), target.as_str());
								event_payload.push((source.clone(), target.clone(), T::Amount::default(), Operation::Del));
							}
						}
					}
				);
			}

			Self::deposit_event(Event::MultiplePricePairsChanged(event_payload));

			// now increment the block number at which we expect next unsigned transaction.
			let current_block = <frame_system::Pallet<T>>::block_number();
			<AcceptNextOcwTxAt<T>>::put(current_block + T::UnsignedTxAcceptFreq::get());

			Ok(Pays::No.into())
		}

		/// Add price pair
		#[pallet::weight(100)]
		pub fn add_price_pair(
			origin: OriginFor<T>,
			source: T::Currency, target: T::Currency, provider: T::Provider) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(T::TradeProvider::is_valid_provider(provider.clone()), Error::<T>::UnknownTradeProviderError);

			let pair = ProviderPair{ pair: Pair{source: source.clone(), target: target.clone()}, provider: provider.clone() };
			<MonitoredPairs<T>>::try_mutate_exists(pair, |prices_opt| -> DispatchResult {
				if prices_opt.is_none() {
					*prices_opt = Some(None);
					Self::deposit_event(Event::MonitoredPairAdded(source, target, provider));
				}
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn delete_price_pair(
			origin: OriginFor<T>,
			source: T::Currency, target: T::Currency, provider: T::Provider) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(T::TradeProvider::is_valid_provider(provider.clone()), Error::<T>::UnknownTradeProviderError);

			let pair = ProviderPair{ pair: Pair{source: source.clone(), target: target.clone()}, provider: provider.clone() };
			<MonitoredPairs<T>>::try_mutate_exists(pair, |prices_opt| -> DispatchResult {
				prices_opt.take().ok_or(Error::<T>::PricePairNotFoundError)?;
				Self::deposit_event(Event::MonitoredPairRemoved(source, target, provider));
				Ok(())
			})
		}

		/// Provided externally, by eg. root user, to inform ocw of additional pairs
		#[pallet::weight(100)]
		#[frame_support::transactional]
		pub fn submit_price_pairs(
			origin: OriginFor<T>,
			pairs: Vec<(T::Currency, T::Currency, T::Provider, Operation)>) -> DispatchResult {
			ensure_root(origin)?;

			// first check ALL the pairs exist
			for (source, target, provider, op) in &pairs {
				let pair = ProviderPair{ pair: Pair{source: source.clone(), target: target.clone()}, provider: provider.clone() };
				ensure!(T::TradeProvider::is_valid_provider(provider.clone()), Error::<T>::UnknownTradeProviderError);
				if *op == Operation::Del {
					ensure!(<MonitoredPairs<T>>::contains_key(pair), Error::<T>::PricePairNotFoundError);
				}
			}

			// as per above, except with multiples and no error checking (done above)
			for (source, target, provider, op) in pairs {
				let pair = ProviderPair{ pair: Pair{source: source.clone(), target: target.clone()}, provider: provider.clone() };
				<MonitoredPairs<T>>::mutate_exists(pair, |prices_opt| {
					match op {
						Operation::Add => if prices_opt.is_none() {
							*prices_opt = Some(None);
							Self::deposit_event(Event::MonitoredPairAdded(source, target, provider));
						},
						Operation::Del => {
							prices_opt.take();
							Self::deposit_event(Event::MonitoredPairRemoved(source, target, provider));
						},  // no error checking required, should have been dealt with prior
					}
				});
			}
			Ok(())
		}

		/// Trade as per available prices.
		#[pallet::weight(100)]
		pub fn trade(
			origin: OriginFor<T>,
			source: T::Currency, target: T::Currency, amount: T::Amount) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			// ensure!(<BestPaths<T>>::contains_key(source, target), Error::<T>::PricePairNotFoundError);

			match <BestPaths<T>>::get(&source, &target) {
				Some(price_path) => {
					// FIXME: perform actual transfers, following path, mimicking:
					// <pallet_balances::Module<T> as Currency<T::AccountId>>::transfer(&new_owner, &sender, price, ExistenceRequirement::KeepAlive)?;  // KeepAlive = ensure enough funds in account to keep account alive
					let cost = price_path.total_cost;
					let target_amount = amount * cost;
					Self::deposit_event(Event::TradePerformed(owner, source, target, cost, amount, target_amount));
					Ok(())
				}
				None => Err(Error::<T>::PricePairNotFoundError.into()),
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned call to this module.
		///
		/// By default unsigned transactions are disallowed, but implementing the validator
		/// here we make sure that some particular calls (the ones produced by offchain worker)
		/// are being whitelisted and marked as valid.
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			// Firstly let's check that we call the right function.
			if let Call::ocw_submit_best_paths_changes {
				best_path_change_payload: ref payload,
				ref signature,
			} = call
			{
				let signature_valid = SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone());
				if !signature_valid {
					return InvalidTransaction::BadProof.into()
				}

				// reject transactions from the future
				let current_block = <frame_system::Pallet<T>>::block_number();
				if current_block < payload.block_number {
					return InvalidTransaction::Future.into()
				}

				// Now let's check if the transaction has any chance to succeed.
				let next_ocw_tx_at = <AcceptNextOcwTxAt<T>>::get();
				if next_ocw_tx_at > payload.block_number {
					return InvalidTransaction::Stale.into()
				}

				ValidTransaction::with_tag_prefix("AggregatorWorker")
					.priority(T::UnsignedPriority::get())  // prioritise on changeset size... FIXME: good strategy?
					// This transaction does not require anything else to go before into the pool.
					// In theory we could require `previous_unsigned_at` transaction to go first,
					// but it's not necessary in our case.
					//.and_requires()
					// We set the `provides` tag to be the same as `next_unsigned_at`. This makes
					// sure only one transaction produced after `next_unsigned_at` will ever
					// get to the transaction pool and will end up in the block.
					// We can still have multiple transactions compete for the same "spot",
					// and the one with higher priority will replace other one in the pool.
					.and_provides(next_ocw_tx_at)
					.longevity(5)  // transaction is only valid for next 5 blocks. After that it's revalidated by the pool.
					.propagate(true)
					.build()
			} else {
				InvalidTransaction::Call.into()
			}
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct BestPathChangesPayload<Public, BlockNumber, C: Currency, P: Provider, A: Amount> {
	block_number: BlockNumber,
	changes: Vec<(C, C, Option<PricePath<C, P, A>>)>,
	public: Public,
}

impl<T: SigningTypes, C: Currency, P: Provider, A: Amount> SignedPayload<T> for BestPathChangesPayload<T::Public, T::BlockNumber, C, P, A> {
	fn public(&self) -> T::Public {
		self.public.clone()
	}
}

impl<T: Config> Pallet<T> {
	fn should_trigger_offchain(block_number: T::BlockNumber) -> bool {
		const RECENTLY_SENT: () = ();
		let next_offchain_trigger = StorageValueRef::persistent(NEXT_OFFCHAIN_TRIGGER_STORAGE);
		// FIXME: using StorageValueRef as a lock, should use StorageLock instead?
		let res = next_offchain_trigger.mutate(|val: Result<Option<T::BlockNumber>, StorageRetrievalError>| {
			match val {
				// Have we triggered recent enough?
				Ok(Some(next_trigger)) if block_number < next_trigger => Err(RECENTLY_SENT),
				// In every other case we attempt to acquire the lock and trigger offchain worker
				_ => Ok(block_number + T::OffchainTriggerFreq::get()),
			}
		});
		match res {
			// The value has been set correctly, which means we can safely send a transaction now.
			Ok(_) =>{
				log::debug!("Offchain lock acquired!");
				true
			}
			Err(MutateStorageError::ValueFunctionFailed(RECENTLY_SENT)) => {
				log::debug!("Offchain lock acquire attempted too soon");
				false
			}
			Err(MutateStorageError::ConcurrentModification(_)) => {
				log::debug!("Offchain lock unavailable, will not trigger");
				false
			}
		}
	}

	/// A helper function to fetch the price, sign payload and send an unsigned transaction
	fn fetch_prices_and_update_best_paths(
		block_number: T::BlockNumber,
	) -> Result<(), String> {
		let fetched_pairs = <MonitoredPairs<T>>::iter_keys()
			.map(|pp| (pp.clone(), T::TradeProvider::get_price(pp.provider, pp.pair.source, pp.pair.target)))
			.filter(|(_, provider_opt)| provider_opt.is_ok())
			.filter(|(_, res)| res.is_ok())
			.map(|(p, res)| (p, res.unwrap()))
			.collect::<Vec<(ProviderPair<_, _>, _)>>();

		if fetched_pairs.is_empty() {
			log::debug!("Offchain: no price pairs to update!");
			return Ok(())
		}

		let new_best_paths: BTreeMap<Pair<_>, PricePath<_, _, _>> = T::BestPathCalculator::calc_best_paths(fetched_pairs).map_err(|e| format!("Failed to calculate best prices due to {:?}", e))?;  // FIXME: provide reason

		// select the best path differences
		// - elements changed at all and outside of acceptable tolerance
		// - no longer existing elements
		// - newly added elements
		let mut changes = vec![];
		let tolerance = T::PriceChangeTolerance::get();
		for (source, target, old_price_path) in BestPaths::<T>::iter() {  // FIXME: iterating ovr *all* of BestPaths...
			let pair = Pair{ source: source.clone(), target: target.clone() };
			match new_best_paths.get(&pair) {
				Some(new_price_path) => {
					let old_total_cost = TryInto::<u128>::try_into(old_price_path.total_cost).ok().unwrap();  // FIXME: better way to convert?
					let new_total_cost = TryInto::<u128>::try_into(new_price_path.total_cost).ok().unwrap();
					if breaches_tolerance(old_total_cost, new_total_cost, tolerance) {
						log::info!("Offchain: adding price change for {:?} -> {:?} in excess of tolerance: {:?}: {:?} -> {:?}", pair.source.as_str(), pair.target.as_str(), tolerance, old_total_cost, new_total_cost);
						changes.push((source, target, Some(new_price_path.clone())));
					} else {
						log::info!("Offchain: skipping price change for {:?} -> {:?} within tolerance of {:?}: {:?} -> {:?}", pair.source.as_str(), pair.target.as_str(), tolerance, old_total_cost, new_total_cost);
					}
				}
				None => log::info!("Offchain: no price fetched for {:?} -> {:?}", pair.source.as_str(), pair.target.as_str()),
			}
		}
		for (Pair{source, target}, new_price_path) in new_best_paths.into_iter() {
			if ! BestPaths::<T>::contains_key(&source, &target) {
				log::info!("Offchain: adding new price: for {:?} -> {:?}: {:?}", source.as_str(), target.as_str(), &new_price_path.total_cost);
				changes.push((source, target, Some(new_price_path.clone())))
			// } else {
			// 	log::info!("Skipping existing price for {:?} -> {:?}: {:?}", source.as_str(), target.as_str(), &new_price_path);
			}
		}

		// -- Sign using any account
		let (_, result) = Signer::<T, T::AuthorityId>::any_account()  // vs all_accounts()?
			.send_unsigned_transaction(
				|account| BestPathChangesPayload { block_number, changes: changes.clone(), public: account.public.clone() },
				|payload, signature| Call::ocw_submit_best_paths_changes {
					best_path_change_payload: payload,
					signature,
				},
			)
			.ok_or("No local accounts accounts available.")?;
		result.map_err(|()| "Unable to submit transaction")?;

		log::info!("Offchain: updated best paths!");

		Ok(())
	}
}
