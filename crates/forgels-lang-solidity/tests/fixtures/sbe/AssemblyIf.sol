pragma solidity ^0.8.24;

contract AssemblyIf {
    function yulIf(uint x) public pure returns (uint) {
        uint before = x;
        assembly {
            if x { }
        }
        uint after = before;
        return after;
    }
}
