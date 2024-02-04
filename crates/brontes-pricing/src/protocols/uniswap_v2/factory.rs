use alloy_sol_macro::sol;

sol!(
    interface IUniswapV2Factory {
        event PairCreated(address indexed token0, address indexed token1, address pair, uint256);
    }
);
