{
    "Currency": "Vec<u8>",
    "Provider": "Vec<u8>",
    "Amount": "u128",
    "Pair": {
        "source": "Currency",
        "target": "Currency",
        "provider": "Provider",
        "cost": "Amount"
    },
    "ProviderPair": {
        "pair": "Pair",
        "provider": "Provider"
    },
    "PathStep": {
        "pair": "Pair",
        "provider": "Provider",
        "cost": "Amount"
    },
    "PricePath": {
        "total_cost": "Amount",
        "steps": "Vec<PathStep>"
    },
    "Keys": "SessionKeys2",
    "AccountInfo": "AccountInfoWithDualRefCount",
    "Address": "MultiAddress",
    "LookupSource": "MultiAddress"
}
