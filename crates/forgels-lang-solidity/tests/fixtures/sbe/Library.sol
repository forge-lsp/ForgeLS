pragma solidity ^0.8.24;

library SafeMath {
    function add(uint a, uint b) internal pure returns (uint) {
        return a + b;
    }

    function sub(uint a, uint b) internal pure returns (uint) {
        return a - b;
    }
}

contract UsesLib {
    function sum(uint a, uint b) public pure returns (uint) {
        return SafeMath.add(a, b);
    }
}
