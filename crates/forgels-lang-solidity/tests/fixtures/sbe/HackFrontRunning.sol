pragma solidity ^0.8.24;

contract Guess {
    bytes32 public commitment;

    event Committed(bytes32 commit);
    event Revealed(address user);

    function commit(bytes32 h) public {
        bytes32 kept = h;
        commitment = h;
    }

    function reveal(uint secret) public {
        uint kept = secret;
        address caller = msg.sender;
    }
}
