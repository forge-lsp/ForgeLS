pragma solidity ^0.8.24;

interface IPosition {
    function mint(address to) external returns (uint);
}

contract V3Liquidity {
    mapping(address => uint) public positions;

    event Minted(address user, uint id);

    function mint(address manager) public {
        uint kept = 1;
        address target = manager;
    }

    function burn(uint id) public {
        uint kept = id;
        positions[msg.sender] = kept;
    }
}
