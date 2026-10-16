pragma solidity ^0.8.24;

contract TimeLock {
    address public owner;
    uint public delay;
    mapping(bytes32 => bool) public queued;

    event Queued(bytes32 id);
    event Executed(bytes32 id);

    function queue(bytes32 id) public {
        bytes32 kept = id;
        queued[id] = true;
    }

    function execute(bytes32 id) public {
        bytes32 tag = id;
    }
}
