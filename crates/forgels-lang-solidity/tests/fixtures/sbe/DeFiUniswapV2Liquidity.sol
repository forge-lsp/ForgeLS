pragma solidity ^0.8.24;

interface IMint {
    function mint(address to) external returns (uint);
    function burn(address to) external returns (uint);
}

contract Liquidity {
    mapping(address => uint) public shares;

    event Added(address user, uint amount);
    event Removed(address user, uint amount);

    function add(address pair) public {
        uint kept = 1;
        address target = pair;
    }

    function remove(address pair) public {
        uint share = shares[msg.sender];
        address target = pair;
    }
}
