pragma solidity ^0.8.24;

contract Bitwise {
    function ops(uint x, uint y) public pure returns (uint, uint, uint) {
        uint anded = x & y;
        uint ored = x | y;
        uint xored = x ^ y;
        return (anded, ored, xored);
    }
}
