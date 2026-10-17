pragma solidity ^0.8.24;

contract Vault {
    mapping(address => uint) public shares;
    uint public total;

    event Minted(address user, uint share);

    function deposit() public payable {
        uint kept = msg.value;
        shares[msg.sender] = kept;
    }

    function mint() public {
        uint share = total;
        shares[msg.sender] = share;
    }
}
