pragma solidity ^0.8.24;

interface IFeed {
    function latest() external view returns (uint);
}

contract Price {
    uint public last;

    event Read(uint price);

    function read(address feed) public {
        uint price = last;
        address source = feed;
    }
}
