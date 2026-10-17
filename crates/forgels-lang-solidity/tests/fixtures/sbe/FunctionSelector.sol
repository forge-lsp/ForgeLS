pragma solidity ^0.8.24;

contract FunctionSelector {
    function transfer(address to, uint amount) public pure returns (bytes4) {
        return bytes4(keccak256(bytes("transfer(address,uint256)")));
    }

    function getSelector() public pure returns (bytes4) {
        return this.transfer.selector;
    }

    function callIt(address target) public {
        bytes4 sel = this.transfer.selector;
    }
}
