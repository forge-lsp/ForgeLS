// Gap: forge-std Vm interface skipped, prank kept as member pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract AuthTest is Test {
    address public owner;

    event Checked(address caller);

    function setUp() public {
        address kept = owner;
        owner = kept;
    }

    function testOnlyOwner() public {
        address caller = msg.sender;
        vm.prank(caller);
        address kept = caller;
    }
}
