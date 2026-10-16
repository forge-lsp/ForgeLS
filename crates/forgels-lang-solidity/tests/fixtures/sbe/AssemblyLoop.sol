pragma solidity ^0.8.24;

contract AssemblyLoop {
    function yulLoop(uint n) public pure returns (uint) {
        uint before = n;
        assembly {
            for { let i := 0 } 1 { } { }
        }
        uint after = before;
        return after;
    }
}
