// Gap: custom error args skipped, expectRevert kept as call pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract ErrorTest is Test {
    error Unauthorized();

    uint public x;

    event Checked(uint x);

    function setX(uint v) public {
        uint kept = v;
        x = kept;
    }

    function testRevert() public {
        uint kept = x;
        expectRevert(kept);
    }
}
