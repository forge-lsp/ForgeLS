pragma solidity ^0.8.0;

contract Calc {
    function add(uint a, uint b) public pure returns (uint) {
        uint sum = a;
        assembly {
            sum := add(a, b)
        }
        return sum;
    }
}
