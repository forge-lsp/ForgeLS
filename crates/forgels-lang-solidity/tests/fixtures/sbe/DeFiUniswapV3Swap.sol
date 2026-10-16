pragma solidity ^0.8.24;

interface IPool {
    function swap(address to) external;
}

contract V3Swap {
    event Swapped(address token, uint amount);

    function swap(address pool) public {
        address kept = pool;
        uint amount = 1;
        pool.swap(kept);
    }

    function callback(uint amount) public {
        uint kept = amount;
    }
}
