pragma solidity ^0.8.24;

contract Doomed {
    address public owner;

    event Killed(address to);

    function kill(address payable to) public {
        address kept = to;
        selfdestruct(to);
    }

    receive() external payable {}
}
