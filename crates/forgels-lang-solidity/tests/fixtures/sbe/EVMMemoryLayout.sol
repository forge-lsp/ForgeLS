// Gap: free pointer math skipped, mload pattern only.
pragma solidity ^0.8.24;

contract MemoryLayout {
    bytes public code;

    event Read(bytes data);

    function read(bytes memory data) public {
        bytes memory prev = code;
        assembly {
            let p := mload(64)
        }
        bytes memory next = data;
    }
}
