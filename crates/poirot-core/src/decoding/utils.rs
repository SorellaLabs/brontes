use alloy_sol_types::sol;

sol! {
    interface IDiamondLoupe {
        function facetAddress(bytes4 _functionSelector) external view returns (address facetAddress_);
    }
}
