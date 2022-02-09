#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]

use codec::{Decode, Encode};
use frame_support::{traits::Get, transactional};
use frame_system::{
	self,
	offchain::{AppCrypto, CreateSignedTransaction, SendUnsignedTransaction, SignedPayload, Signer, SigningTypes},
};
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
	offchain::{http, Duration, storage::StorageValueRef, storage_lock::{StorageLock, Time}},
	traits::IdentifyAccount,
	transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

/// Duration for getting the OCW lock
pub const OCW_LOCK_DURATION: u64 = 100;

/// Defines application identifier for crypto keys of this module.
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"aggr");

/// Transaction tag to deduplicate OCW transactions
pub const TX_TAG: &[u8] = b"aggregator";

/// OCW off-chain lookup
pub const OCW_WORKER_LOCK: &[u8] = b"aggregator::ocw_lock";

/// Key for the next offchain trigger.
pub const NEXT_OFFCHAIN_TRIGGER: &[u8] = b"aggregator::next_offchain_trigger";

pub trait BestPathCalculator<C: Currency, P: Provider, A: Amount> {
	fn calc_best_paths(pairs_and_prices: &[(ProviderPair<C, P>, A)]) -> Result<BTreeMap<Pair<C>, PricePath<C, P, A>>, CalculatorError>;
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

	/// DoubleMap of source/target currencies => cost by provider
	#[pallet::storage]
	pub(super) type BestPaths<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::Currency,  // source currency
		Blake2_128Concat,
		T::Currency,  // target currency
		PricePath<T::Currency, T::Provider, T::Amount>, // best path
	>;

	#[pallet::storage]
	pub(super) type MonitoredPairs<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ProviderPair<T::Currency, T::Provider>,
		Option<()>,  // membership in the map indicates price is to be fetched, Some(()) - existence of the latest price
	>;

	#[pallet::storage]
	pub(super) type WhitelistedOffchainAuthorities<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		(),
	>;

	#[pallet::storage]
	pub(super) type UnsignedTxNonce<T: Config> = StorageValue<
		_,
		u64,
		ValueQuery,
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

		/// Added offchain authority account, for validation of offchain signed payloads.
		/// \[account_id\]
		WhitelistedOffchainAuthorityAdded(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		PricePairNotFoundError,
		UnknownTradeProviderError,
		StaleOffchainNonceError,
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

		type WeightInfo: WeightInfo;

		// Configuration parameters

		/// Frequency of offchain worker trigger.
		///
		/// To avoid sending too many offchain worker instantiations, we only attempt to trigger one
		/// every `OffchainTriggerDelay` blocks. We use Local Storage to coordinate
		/// sending between distinct runs.
		#[pallet::constant]
		type OffchainTriggerDelay: Get<Self::BlockNumber>;

		/// Priority of unsigned transactions
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
		/// Off-chain Worker entry point, keeping functionality to minimum and delegating to `impl` block.
		fn offchain_worker(block_number: T::BlockNumber) {
			if Self::should_trigger_offchain(block_number) {
				// obtain the OCW lock
				let lock_expiration = Duration::from_millis(OCW_LOCK_DURATION);
				let mut lock = StorageLock::<'static, Time>::with_deadline(OCW_WORKER_LOCK, lock_expiration);

				match lock.try_lock() {
					Ok(_guard) => {
						if let Err(e) = Self::fetch_prices_and_update_best_paths() {
							log::error!("OCW price fetching error: {}", e);
						}
						// bump the offchain trigger
						let next_trigger = block_number + T::OffchainTriggerDelay::get();
						StorageValueRef::persistent(NEXT_OFFCHAIN_TRIGGER).set(&next_trigger);
					},
					Err(e) => log::warn!("OCW failed to obtain OCW lock due to {:?}", e)
				};
			}
		}
	}

	/// A public part of the pallet.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::submit_price_pairs(best_path_change_payload.changes.len()))]
		#[transactional]
		pub fn ocw_submit_best_paths_changes(
			origin: OriginFor<T>,
			best_path_change_payload: BestPathChangesPayload<T::Public, T::Currency, T::Provider, T::Amount>,
			_signature: T::Signature,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			let current_nonce = <UnsignedTxNonce<T>>::get();
			ensure!(current_nonce == best_path_change_payload.nonce, Error::<T>::StaleOffchainNonceError);

			let mut event_payload = vec![];
			for (ref source, ref target, ref mut new_path) in best_path_change_payload.changes {
				BestPaths::<T>::mutate_exists(
					source,
					target,
					|old_path| {
						match new_path.take() {
							Some(path) => {
								let total_cost = path.total_cost;
								*old_path = Some(path);
								log::info!("Onchain: adding/changing price onchain for {} -> {}: {:?}", source.to_str(), target.to_str(), total_cost);
								event_payload.push((source.clone(), target.clone(), total_cost, Operation::Add));
							}
							None => if old_path.take().is_some() {
								log::info!("Onchain: removing price onchain: {} -> {}", source.to_str(), target.to_str());
								event_payload.push((source.clone(), target.clone(), T::Amount::default(), Operation::Del));
							}
						}
					}
				);
			}

			<UnsignedTxNonce<T>>::set(current_nonce + 1);
			Self::deposit_event(Event::MultiplePricePairsChanged(event_payload));
			Ok(Pays::No.into())
	}

		/// Add price pair
		#[pallet::weight(
			T::WeightInfo::add_price_pair_nonexisting().max(T::WeightInfo::add_price_pair_existing())
        )]
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

		#[pallet::weight(T::WeightInfo::delete_price_pair())]
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

		#[pallet::weight(T::WeightInfo::add_whitelisted_offchain_authority())]
		pub fn add_whitelisted_offchain_authority(
			origin: OriginFor<T>,
			offchain_authority: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;

			Self::deposit_event(Event::WhitelistedOffchainAuthorityAdded(offchain_authority.clone()));
			<WhitelistedOffchainAuthorities<T>>::insert(&offchain_authority, ());
			Ok(())
		}

		/// Provided externally, by eg. root user, to inform ocw of additional pairs
		#[pallet::weight(T::WeightInfo::submit_price_pairs(pairs.len()))]
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
		#[pallet::weight(T::WeightInfo::trade())]
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
					log::error!("OCW rejected transaction due to invalid signature");
					return InvalidTransaction::BadProof.into()
				}

				let account_id = payload.public.clone().into_account();
				if !WhitelistedOffchainAuthorities::<T>::contains_key(&account_id) {
					log::error!("OCW rejected transaction due to signer not on the offchain authority whitelist: {:?}", account_id);
					return InvalidTransaction::BadProof.into();
				}

				ValidTransaction::with_tag_prefix("AggregatorWorker")
					.priority(T::UnsignedPriority::get())
					// This transaction does not require anything else to go before into the pool.
					// In theory we could require `previous_unsigned_at` transaction to go first,
					// but it's not necessary in our case.
					//.and_requires()
					// We set the `provides` tag to be the same as `next_unsigned_at`. This makes
					// sure only one transaction produced after `next_unsigned_at` will ever
					// get to the transaction pool and will end up in the block.
					// We can still have multiple transactions compete for the same "spot",
					// and the one with higher priority will replace other one in the pool.
					.and_provides((<frame_system::Pallet<T>>::block_number(), TX_TAG))
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
pub struct BestPathChangesPayload<Public, C: Currency, P: Provider, A: Amount> {
	changes: Vec<(C, C, Option<PricePath<C, P, A>>)>,
	nonce: u64,
	public: Public,
}

impl<T: SigningTypes, C: Currency, P: Provider, A: Amount> SignedPayload<T> for BestPathChangesPayload<T::Public, C, P, A> {
	fn public(&self) -> T::Public {
		self.public.clone()
	}
}

impl<T: Config> Pallet<T> {
	fn should_trigger_offchain(block_number: T::BlockNumber) -> bool {
		match StorageValueRef::persistent(NEXT_OFFCHAIN_TRIGGER).get::<T::BlockNumber>() {
			Ok(Some(next_offchain_trigger)) if block_number >= next_offchain_trigger => {
				log::info!("Offchain trigger block encountered!");
				true
			}
			Ok(Some(_)) => {
				log::info!("Offchain trigger attempted too soon!");
				false
			}
			Ok(None) => {
				log::info!("Offchain trigger block initiated!");
				true
			}
			_ => {
				log::info!("Offchain trigger not readable!");
				false
			}
		}
	}

	/// A helper function to fetch the price, sign payload and send an unsigned transaction
	fn fetch_prices_and_update_best_paths() -> Result<(), String> {
		let fetched_pairs = <MonitoredPairs<T>>::iter_keys()
			.filter_map(|pp| {
				let pp2 = pp.clone();
				T::TradeProvider::get_price(pp2.provider, pp2.pair.source, pp2.pair.target).ok().map(move |res| (pp, res))
			})
			.collect::<Vec<(_, _)>>();

		if fetched_pairs.is_empty() {
			log::debug!("Offchain: no price pairs to update!");
			return Ok(())
		}

		let new_best_paths: BTreeMap<Pair<_>, PricePath<_, _, _>> = T::BestPathCalculator::calc_best_paths(&fetched_pairs).map_err(|e| format!("Failed to calculate best prices due to {:?}", e))?;

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
					let old_total_cost: u128 = old_price_path.total_cost.try_into().map_err(|_| "failed to convert old_price_path.total_cost")?;
					let new_total_cost: u128 = new_price_path.total_cost.try_into().map_err(|_| "failed to convert new_price_path.total_cost")?;
					if breaches_tolerance(old_total_cost, new_total_cost, tolerance) {
						log::debug!("Offchain: adding price change for {:?} -> {:?} in excess of tolerance: {:?}: {:?} -> {:?}", pair.source.to_str(), pair.target.to_str(), tolerance, old_total_cost, new_total_cost);
						changes.push((source, target, Some(new_price_path.clone())));
					} else {
						log::debug!("Offchain: skipping price change for {:?} -> {:?} within tolerance of {:?}: {:?} -> {:?}", pair.source.to_str(), pair.target.to_str(), tolerance, old_total_cost, new_total_cost);
					}
				}
				None => log::debug!("Offchain: no price fetched for {:?} -> {:?}", pair.source.to_str(), pair.target.to_str()),
			}
		}
		for (Pair{source, target}, new_price_path) in new_best_paths.into_iter() {
			if ! BestPaths::<T>::contains_key(&source, &target) {
				log::debug!("Offchain: adding new price: for {:?} -> {:?}: {:?}", source.to_str(), target.to_str(), &new_price_path.total_cost);
				changes.push((source, target, Some(new_price_path.clone())))
			}
		}

		if changes.is_empty() {
			log::info!("Offchain: detected no price changes that breached tolerance level")
		} else {
			let (_, result) = Signer::<T, T::AuthorityId>::any_account()
				.send_unsigned_transaction(
					|account| BestPathChangesPayload {changes: changes.clone(), nonce: <UnsignedTxNonce<T>>::get(), public: account.public.clone() },
					|payload, signature| Call::ocw_submit_best_paths_changes {
						best_path_change_payload: payload,
						signature,
					},
				)
				.ok_or("No local accounts accounts available")?;
			result.map_err(|()| "Unable to submit transaction")?;

			log::info!("Offchain: updated best paths!");
		}

		Ok(())
	}
}
