// SPDX-License-Identifier: GPL-3.0-or-later
pragma solidity ^0.8.17;
import {IUniswapV3Pool} from "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import {ERC20} from "solmate/tokens/ERC20.sol";
import {IRamsesV2Pool} from "contracts/Interfaces/IUni.sol";
import "@openzeppelin/contracts/utils/Address.sol";

/// @title UniV3Facet
/// @author Eisen (https://app.eisenfinance.com)
/// @notice Provides functionality for UniswapV3 protocol
/// @custom:version 1.0.0
contract GetUniswapV3TickDataBatchRequest {
    using Address for address;

    int24 private constant _MIN_TICK = -887272;
    int24 private constant _MAX_TICK = 887272;

    struct PoolData {
        address tokenA;
        uint8 tokenADecimals;
        address tokenB;
        uint8 tokenBDecimals;
        uint128 liquidity;
        uint160 sqrtPrice;
        int24 tick;
        int24 tickSpacing;
        uint24 fee;
        int128 liquidityNet;
    }
    struct TickData {
        bool initialized;
        int24 tick;
        int128 liquidityNet;
    }

    /// @notice Get uniV3 pool param info using pool address

    function data_constructor(address[] memory pools) public view returns (PoolData[] memory) {
        PoolData[] memory poolData = new PoolData[](pools.length);
        for (uint256 i = 0; i < pools.length; i++) {
            address pool = pools[i];
            poolData[i].tokenA = IUniswapV3Pool(pool).token0();
            poolData[i].tokenADecimals = ERC20(IUniswapV3Pool(pool).token0()).decimals();
            poolData[i].tokenB = IUniswapV3Pool(pool).token1();
            poolData[i].tokenBDecimals = ERC20(IUniswapV3Pool(pool).token1()).decimals();
            poolData[i].liquidity = IUniswapV3Pool(pool).liquidity();
            uint160 sqrtPriceX96;
            uint24 fee = IUniswapV3Pool(pool).fee();
            int24 tick;
            (bool success, bytes memory output) = pool.staticcall(
                abi.encodeWithSelector(IRamsesV2Pool.currentFee.selector)
            );
            bytes memory result = pool.functionStaticCall(abi.encodeWithSelector(0x3850c7bd)); // slot0 call
            assembly ("memory-safe") {
                let len := mload(result)
                mstore(sqrtPriceX96, mload(add(result, 0x20))) // response.sqrtPriceX96
                mstore(tick, mload(add(result, 0x40))) // response.tick
                if and(gt(len, 0xc0), success) {
                    mstore(fee, mload(add(output, 0x20))) // response.feeProtocol [ramses cases]
                }
            }
            (success, output) = pool.staticcall(abi.encodeWithSelector(0xf30dba93, tick));
            int128 liquidityNet;
            assembly ("memory-safe") {
                liquidityNet := mload(add(output, 0x20))
            }
            poolData[i].fee = fee;
            poolData[i].tick = tick;
            poolData[i].sqrtPrice = sqrtPriceX96;
            poolData[i].tickSpacing = IUniswapV3Pool(pool).tickSpacing();
            poolData[i].liquidityNet = liquidityNet;
        }
        return poolData;
    }

    function tick_constructor(
        address pool,
        bool zeroForOne,
        int24 currentTick,
        uint16 numTicks,
        int24 tickSpacing
    ) public view returns (TickData[] memory, uint64) {
        int24 tickRange = int24(int16(numTicks)) * tickSpacing;
        int24 boundaryTick = zeroForOne ? currentTick + tickRange : currentTick - tickRange;
        if (boundaryTick < _MIN_TICK) {
            boundaryTick = _MIN_TICK;
        }
        if (boundaryTick > _MAX_TICK) {
            boundaryTick = _MAX_TICK;
        }

        int24[] memory initTicks = new int24[](uint256(int256((boundaryTick - currentTick + 1) / tickSpacing)) + 1);

        uint256 counter = 0;
        (int16 pos, int16 endPos) = (int16((currentTick / tickSpacing) >> 8), int16((boundaryTick / tickSpacing) >> 8));
        if (zeroForOne) {
            for (; pos <= endPos; pos++) {
                uint256 bm = IUniswapV3Pool(pool).tickBitmap(pos);
                while (bm != 0) {
                    uint8 bit = _leastSignificantBit(bm);
                    bm ^= 1 << bit;
                    int24 extractedTick = ((int24(pos) << 8) | int24(uint24(bit))) * int24(tickSpacing);
                    if (extractedTick >= currentTick && extractedTick <= boundaryTick) {
                        initTicks[counter++] = extractedTick;
                    }
                    if (counter == numTicks) {
                        break;
                    }
                }
            }
        } else {
            for (; pos >= endPos; pos--) {
                uint256 bm = IUniswapV3Pool(pool).tickBitmap(pos);
                while (bm != 0) {
                    uint8 bit = _leastSignificantBit(bm);
                    bm ^= 1 << bit;
                    int24 extractedTick = ((int24(pos) << 8) | int24(uint24(bit))) * int24(tickSpacing);
                    if (extractedTick >= boundaryTick && extractedTick <= currentTick) {
                        initTicks[counter++] = extractedTick;
                    }
                    if (counter == numTicks) {
                        break;
                    }
                }
            }
        }

        TickData[] memory ticks = new TickData[](counter);

        for (uint256 i = 0; i < counter; i++) {
            (bool success, bytes memory outputs) = address(pool).staticcall(
                abi.encodeWithSelector(0xf30dba93, initTicks[i])
            ); // ticks(int24 tick)
            if (!success) {
                continue;
            }

            int128 liquidityNet;

            assembly ("memory-safe") {
                let data := add(outputs, 0x20)
                liquidityNet := mload(add(data, 0x20))
            }

            ticks[i].liquidityNet = liquidityNet;
            ticks[i].tick = initTicks[i];
            ticks[i].initialized = true;
        }
        return (ticks, uint64(counter));
    }

    /// @dev Gets the least significant bit
    function _leastSignificantBit(uint256 x) private pure returns (uint8 r) {
        require(x > 0, "x is 0");
        x = x & (~x + 1);

        if (x >= 0x100000000000000000000000000000000) {
            x >>= 128;
            r += 128;
        }
        if (x >= 0x10000000000000000) {
            x >>= 64;
            r += 64;
        }
        if (x >= 0x100000000) {
            x >>= 32;
            r += 32;
        }
        if (x >= 0x10000) {
            x >>= 16;
            r += 16;
        }
        if (x >= 0x100) {
            x >>= 8;
            r += 8;
        }
        if (x >= 0x10) {
            x >>= 4;
            r += 4;
        }
        if (x >= 0x4) {
            x >>= 2;
            r += 2;
        }
        if (x >= 0x2) r += 1;
    }
}
