// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;

interface IUniswapV2Pair {
    function token0() external view returns (address);

    function token1() external view returns (address);

    function tokenA() external view returns (address);

    function tokenB() external view returns (address);

    function getReserves() external view returns (uint112, uint112, uint32);
}

interface ERC20 {
    function balanceOf(address account) external view returns (uint256);

    function decimals() external view returns (uint8);
}

contract GetUniswapV2PoolDataBatchRequest {
    constructor(address[] memory pools) {
        PoolData[] memory poolData = data_constructor(pools);
        bytes memory abiEncodedData = abi.encode(poolData);
        assembly ("memory-safe") {
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }

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

            (, bytes memory data) = pool.staticcall(
                abi.encodeWithSelector(IUniswapV2Pair.getReserves.selector)
            );
            uint256 reserve0;
            uint256 reserve1;
            assembly ("memory-safe") {
                reserve0 := mload(data)
                reserve1 := mload(add(data, 0x20))
            }
            poolData[i].reserve0 = uint112(reserve0);
            poolData[i].reserve1 = uint112(reserve1);

            try IUniswapV2Pair(pool).tokenA() returns (address tokenA) {
                poolData[i].tokenA = tokenA;
            } catch {
                poolData[i].tokenA = IUniswapV2Pair(pool).token0();
            }

            try IUniswapV2Pair(pool).tokenB() returns (address tokenB) {
                poolData[i].tokenB = tokenB;
            } catch {
                poolData[i].tokenB = IUniswapV2Pair(pool).token1();
            }

            poolData[i].tokenADecimals = ERC20(poolData[i].tokenA).decimals();
            poolData[i].tokenBDecimals = ERC20(poolData[i].tokenB).decimals();
        }
        return poolData;
    }
}
