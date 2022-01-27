use codec::{Decode, Encode};
use frame_support::dispatch::Parameter;
use frame_support::traits::tokens::Balance;
use sp_runtime::RuntimeDebug;
use sp_runtime::traits::Member;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

pub trait Currency: Member + Parameter + Ord {}
impl <T: Member + Parameter + Ord> Currency for T {}

pub trait Provider: Member + Parameter + Ord {}
impl <T: Member + Parameter + Ord> Provider for T {}

pub trait Amount: Balance + Ord {}
impl <T: Balance + Ord> Amount for T {}

/// Per provider, source and target currency. Represents price points from each provider
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, Ord, PartialOrd)]
pub struct Pair<C: Currency> {
    pub source: C,
    pub target: C,
}

/// Per provider, source and target currency. Represents price points from each provider
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, Ord, PartialOrd)]
pub struct ProviderPair<C: Currency, P: Provider> {
    pub pair: Pair<C>,
    pub provider: P,
}

/// Path for every ProviderPair. Consists of `hops` and overall cost
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct PricePath<C: Currency, P: Provider, A: Amount> {
    pub total_cost: A,
    pub steps: Vec<PathStep<C, P, A>>,
}

/// A `hop` between different currencies, via a provider.
#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct PathStep<C: Currency, P: Provider, A: Amount> {
    pub pair: Pair<C>,
    pub provider: P,
    pub cost: A,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum Operation {
	Add,
	Del,
}

#[derive(RuntimeDebug)]
pub enum CalculatorError {
    NegativeCyclesError,
    ConversionError,
}
