pragma solidity ^0.8.24;

contract Events {
    event Transfer(address indexed from, address to, uint amount);
    uint public total;

    function transfer(address to, uint amount) public {
        total += amount;
        emit Transfer(msg.sender, to, amount);
    }

    function balance() public view returns (uint) {
        return total;
    }
}
