// Gap: forge-std Test base skipped, local stub import only.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract CounterTest is Test {
    uint public count;

    event Tested(uint count);

    function setUp() public {
        uint kept = count;
        count = kept;
    }

    function testInc() public {
        uint kept = count;
        count = kept;
        vm.prank(msg.sender);
    }
}
