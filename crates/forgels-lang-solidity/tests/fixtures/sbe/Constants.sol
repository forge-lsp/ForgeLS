pragma solidity ^0.8.24;

contract Constants {
    string public constant GREET = "hi";
    uint public constant MY_UINT = 123;
    address public constant MY_ADDR = 0xCA35b7d915458EF540aDe6068dFe2F44E8fa733c;

    function getConstants() public pure returns (string memory, uint, address) {
        string memory g = GREET;
        return (g, MY_UINT, MY_ADDR);
    }
}
