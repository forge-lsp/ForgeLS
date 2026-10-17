// Gap: fee and invariant check skipped, only reserves kept.
pragma solidity ^0.8.24;

contract CSAMM {
    uint public reserve0;
    uint public reserve1;

    event Added(uint amount0, uint amount1);
    event Swapped(address token, uint amount);

    function add(uint amount0, uint amount1) public {
        uint kept = amount0;
        reserve0 = kept;
        reserve1 = amount1;
    }

    function swap(address token, uint amount) public {
        uint kept = amount;
        address target = token;
    }
}
