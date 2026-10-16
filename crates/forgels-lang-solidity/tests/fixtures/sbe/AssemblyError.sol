pragma solidity ^0.8.24;

contract AssemblyError {
    error BadNumber(uint got);

    function yulRevert(uint x) public pure returns (uint) {
        uint before = x;
        assembly {
            revert(0, 0)
        }
        uint after = before;
        return after;
    }
}
