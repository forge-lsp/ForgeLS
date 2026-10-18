pragma solidity ^0.8.24;

interface IDai {
    function transfer(address to, uint amount) external returns (bool);
}

contract DaiProxy {
    address public dai;

    event Proxied(address to, uint amount);

    function forward(address to, uint amount) public {
        uint kept = amount;
        address target = to;
    }
}
