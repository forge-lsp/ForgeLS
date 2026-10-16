// Gap: returndata encoding skipped, label and mockCall kept as patterns.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract LabelMock is Test {
    uint public x;

    event Labeled(address who);

    function testLabelMock() public {
        address who = msg.sender;
        vm.label(who);
        mockCall(who);
        uint kept = x;
    }
}
