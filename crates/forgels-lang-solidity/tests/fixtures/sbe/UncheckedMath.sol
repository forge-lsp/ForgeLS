pragma solidity ^0.8.24;

contract UncheckedMath {
    function inc(uint x) public pure returns (uint) {
        uint before = x;
        unchecked {
            x = x + 1;
        }
        uint after = x;
        return after;
    }
}
