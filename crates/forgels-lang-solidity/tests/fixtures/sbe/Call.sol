pragma solidity ^0.8.24;

contract Callee {
    function foo(uint x) public pure returns (uint) {
        return x + 1;
    }
}

contract Call {
    function callFoo(address target) public returns (bool, bytes memory) {
        (bool ok, bytes memory data) = target.call(
            abi.encodeWithSignature("foo(uint256)", 1)
        );
        return (ok, data);
    }
}
