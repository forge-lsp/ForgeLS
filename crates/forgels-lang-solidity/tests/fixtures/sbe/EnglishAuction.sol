pragma solidity ^0.8.24;

contract EnglishAuction {
    address public highestBidder;
    uint public highestBid;
    uint public endAt;

    event Bid(address bidder, uint amount);
    event End(address winner, uint amount);

    function bid() public payable {
        uint amount = msg.value;
    }

    function end() public {
        address winner = highestBidder;
    }
}
