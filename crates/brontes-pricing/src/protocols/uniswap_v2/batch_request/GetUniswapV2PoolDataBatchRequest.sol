// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;
import {IUniswapV2Pair} from "@uniswap/v2-core/contracts/interfaces/IUniswapV2Pair.sol";
import {ERC20} from "solmate/tokens/ERC20.sol";

contract GetUniswapV2PoolDataBatchRequest {
    struct PoolData {
        address tokenA;
        uint8 tokenADecimals;
        address tokenB;
        uint8 tokenBDecimals;
        uint112 reserve0;
        uint112 reserve1;
    }

    /// @notice Get uniV2 pool param info using pool address
    function data_constructor(
        address[] memory pools
    ) public view returns (PoolData[] memory) {
        PoolData[] memory poolData = new PoolData[](pools.length);
        for (uint256 i = 0; i < pools.length; i++) {
            address pool = pools[i];
            (poolData[i].reserve0, poolData[i].reserve1, ) = IUniswapV2Pair(
                pool
            ).getReserves();
            poolData[i].tokenA = IUniswapV2Pair(pool).token0();
            poolData[i].tokenADecimals = ERC20(IUniswapV2Pair(pool).token0())
                .decimals();
            poolData[i].tokenB = IUniswapV2Pair(pool).token1();
            poolData[i].tokenBDecimals = ERC20(IUniswapV2Pair(pool).token1())
                .decimals();
        }
        return poolData;
    }
}
