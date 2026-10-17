pragma solidity ^0.8.24;

contract DoS {
    address[] public winners;

    event Paid(address to);

    function pay() public {
        for (uint i = 0; i < winners.length; i++) {
            winners[i].call{value: 1}("");
        }
        uint done = winners.length;
    }
}
