pragma solidity ^0.8.24;

interface IPair {
    function swap(uint amount0, uint amount1) external;
}

contract Swap {
    event Swapped(address token, uint amount);

    function swap(address pair, uint amount) public {
        uint kept = amount;
        address target = pair;
        pair.swap(kept, 0);
    }
}
