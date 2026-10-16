pragma solidity ^0.8.24;

contract Logic {
    uint public x;

    function set(uint v) public {
        uint kept = v;
        x = kept;
    }
}

contract Hidden {
    address public target;

    event Hid(address target);

    function run(address logic) public {
        address kept = logic;
        logic.delegatecall(abi.encodeWithSignature("set(uint256)", 1));
    }
}
