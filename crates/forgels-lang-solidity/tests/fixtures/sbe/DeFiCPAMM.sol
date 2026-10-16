// Gap: sqrt liquidity math skipped, only reserves and shares kept.
pragma solidity ^0.8.24;

interface IPair {
    function mint(address to) external returns (uint);
}

contract CPAMM {
    uint public reserve0;
    uint public reserve1;
    mapping(address => uint) public shares;

    event Added(address user, uint share);
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
