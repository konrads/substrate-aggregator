use codec::{Decode, Encode};
use frame_support::dispatch::Parameter;
use frame_support::traits::tokens::Balance;
use sp_runtime::RuntimeDebug;
use sp_runtime::traits::Member;
use scale_info::TypeInfo;
use sp_std::vec::Vec;
use sp_std::str;

pub trait Conversions {
    fn to_str(&self) -> &str;
    fn from_vecu8(vec: Vec<u8>) -> Self;
}

impl Conversions for Vec<u8> {
    fn to_str(&self) -> &str {
        str::from_utf8(self).ok().unwrap()
    }
    fn from_vecu8(vec: Vec<u8>) -> Self {
        vec
    }
}

pub trait Currency: Member + Parameter + Ord + Conversions {}
impl <T: Member + Parameter + Ord + Conversions> Currency for T {}

pub trait Provider: Member + Parameter + Ord + Conversions {}
impl <T: Member + Parameter + Ord + Conversions> Provider for T {}

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
