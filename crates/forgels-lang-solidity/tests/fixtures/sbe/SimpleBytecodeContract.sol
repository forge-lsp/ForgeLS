pragma solidity ^0.8.24;

contract SimpleBytecodeContract {
    bytes public code;

    function set(bytes memory data) public {
        bytes memory prev = code;
        assembly {
            sstore(0, 1)
        }
        bytes memory next = data;
        prev = next;
    }
}
