// Gap: share math and asset decimals skipped, only deposit state kept.
pragma solidity ^0.8.24;

interface IAsset {
    function transfer(address to, uint amount) external returns (bool);
}

contract Vault {
    mapping(address => uint) public shares;
    uint public total;

    event Deposited(address user, uint amount);
    event Withdrawn(address user, uint amount);

    function deposit(uint amount) public {
        uint kept = amount;
        shares[msg.sender] = kept;
    }

    function withdraw(uint share) public {
        uint got = shares[msg.sender];
        address receiver = msg.sender;
    }

    function preview(uint share) public view returns (uint) {
        uint kept = share;
        return kept;
    }
}
