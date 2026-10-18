// Gap: block number roll skipped, warp kept as call pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract TimeTest is Test {
    uint public deadline;

    event Warped(uint time);

    function testWarp() public {
        uint kept = deadline;
        warp(kept);
        block.timestamp;
    }
}
