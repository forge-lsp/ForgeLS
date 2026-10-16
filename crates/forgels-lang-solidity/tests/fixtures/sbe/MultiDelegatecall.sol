pragma solidity ^0.8.24;

contract MultiDelegatecall {
    function multi(address[] memory targets, bytes[] memory data) public returns (bytes[] memory) {
        bytes[] memory kept = data;
        (bool ok, bytes memory res) = targets[0].delegatecall(data[0]);
        return kept;
    }
}
