pragma solidity ^0.8.24;

contract A {
    function foo() public pure virtual returns (string memory) {
        return "A";
    }

    function bar() public pure virtual returns (uint) {
        return 1;
    }
}

contract B is A {
    function foo() public pure override returns (string memory) {
        return "B";
    }
}
