pragma solidity ^0.8.24;

contract AssemblyMath {
    function yulMath(uint x, uint y) public pure returns (uint) {
        uint before = x;
        assembly {
            let sum := add(x, y)
        }
        uint after = before;
        return after;
    }
}
