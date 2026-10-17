pragma solidity ^0.8.24;

contract DutchAuction {
    address public seller;
    uint public startPrice;
    uint public startAt;

    event Bought(address buyer, uint price);

    function buy() public payable {
        uint price = startPrice;
    }

    function price() public returns (uint) {
        uint nowPrice = startPrice;
        return nowPrice;
    }
}
