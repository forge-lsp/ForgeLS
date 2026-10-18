pragma solidity ^0.8.24;

contract Logic {
    uint public x;

    function setX(uint _x) public returns (uint) {
        x = _x;
        return x;
    }
}

contract Delegatecall {
    function del(address target, bytes memory data) public returns (bool, bytes memory) {
        (bool ok, bytes memory res) = target.delegatecall(data);
        return (ok, res);
    }
}
