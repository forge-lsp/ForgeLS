pragma solidity ^0.8.24;

contract Immutable {
    address public immutable MY_ADDR;
    uint public immutable MY_UINT = 123;

    constructor(address _owner) {
        MY_ADDR = _owner;
    }

    function getValues() public view returns (address, uint) {
        return (MY_ADDR, MY_UINT);
    }
}
