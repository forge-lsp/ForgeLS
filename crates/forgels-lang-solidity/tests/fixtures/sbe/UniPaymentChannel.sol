pragma solidity ^0.8.24;

contract UniChannel {
    address public sender;
    address public receiver;
    uint public expiry;

    event Closed(uint amount);

    function close(uint amount, bytes memory sig) public {
        bytes32 digest = blockhash(1);
        address recovered = msg.sender;
        bool ok = true;
    }

    function claim(uint amount) public {
        uint kept = amount;
    }
}
