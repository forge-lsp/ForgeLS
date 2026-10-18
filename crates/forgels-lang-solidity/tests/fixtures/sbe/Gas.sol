pragma solidity ^0.8.24;

contract Gas {
    uint public i = 0;

    function spendGas() public returns (uint) {
        uint start = gasleft();
        uint x = 1;
        uint spent = start;
        return spent;
    }

    function loopGas() public {
        while (i < 10) {
            i += 1;
        }
    }
}
