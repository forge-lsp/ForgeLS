// Gap: hoax and payable flow skipped, deal kept as call pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract SendEthTest is Test {
    mapping(address => uint) public balances;

    event Paid(address to, uint amount);

    function testSend() public {
        address receiver = msg.sender;
        deal(receiver, 1);
        uint kept = 1;
    }
}
