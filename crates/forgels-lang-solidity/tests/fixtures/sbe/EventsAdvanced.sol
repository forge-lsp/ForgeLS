pragma solidity ^0.8.24;

contract EventsAdvanced {
    event Log(address indexed sender, string message);
    event AnonymousLog(bytes data) anonymous;
    uint public count;

    function log(string memory message) public {
        count += 1;
        emit Log(msg.sender, message);
    }

    function countLogs() public view returns (uint) {
        return count;
    }
}
