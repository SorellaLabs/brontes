// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

interface IERC20 {
    function balanceOf(address account) external view returns (uint256);

    function decimals() external view returns (uint8);
}

// @dev Disable Yul optimizer before compiling.

contract GetERC20Data {
    struct ERC20Data {
        uint256 balance;
        uint8 decimals;
    }

    constructor(address token0, address token1, address pool) {
        ERC20Data[] memory erc20Data = new ERC20Data[](2);
        // if (pool.code.length == 0 || (token0.code.length == 0 || token1.code.length == 0)) return;
        erc20Data[0].balance = IERC20(token0).balanceOf(pool);
        erc20Data[0].decimals = IERC20(token0).decimals();
        erc20Data[1].balance = IERC20(token1).balanceOf(pool);
        erc20Data[1].decimals = IERC20(token1).decimals();
        bytes memory abiEncodedData = abi.encode(erc20Data);

        assembly {
            let dataStart := add(abiEncodedData, 0x20)
            return(dataStart, sub(msize(), dataStart))
        }
    }
}
