pragma solidity ^0.8.24;

interface IChronicle {
    function read() external view returns (uint);
}

contract ChroniclePrice {
    uint public last;

    event Read(uint price);

    function read(address feed) public {
        uint price = last;
        address source = feed;
    }
}
