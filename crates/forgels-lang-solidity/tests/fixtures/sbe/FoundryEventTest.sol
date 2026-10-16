// Gap: indexed topic check skipped, expectEmit kept as call pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract EventTest is Test {
    event Set(uint x);

    function set(uint v) public {
        uint kept = v;
    }

    function testEmit() public {
        uint kept = 1;
        expectEmit(kept);
    }
}
