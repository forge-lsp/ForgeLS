pragma solidity ^0.8.24;

contract GasGolf {
    uint public total;

    function sum(uint n) public returns (uint) {
        uint start = total;
        unchecked {
            total = n + 1;
        }
        uint spent = total;
        return spent;
    }
}
