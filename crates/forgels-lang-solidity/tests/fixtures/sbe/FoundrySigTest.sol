// Gap: v r s split skipped, sign kept as call pattern.
pragma solidity ^0.8.24;

import "./TestStub.sol";

contract SigTest is Test {
    event Recovered(address signer);

    function testSign() public {
        bytes32 digest;
        address recovered = msg.sender;
        bool ok = recovered == msg.sender;
        sign(digest);
    }
}
