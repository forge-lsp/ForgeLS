pragma solidity ^0.8.24;

contract Fallback {
    event Log(string message);
    uint public count;

    fallback() external {
        count += 1;
        emit Log("fallback");
    }

    receive() external payable {
        count += 1;
    }
}
