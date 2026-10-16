pragma solidity ^0.8.24;

contract AssemblyVariable {
    function yulVar(uint x) public pure returns (uint) {
        uint before = x;
        assembly {
            let y := 2
        }
        uint after = before;
        return after;
    }
}
