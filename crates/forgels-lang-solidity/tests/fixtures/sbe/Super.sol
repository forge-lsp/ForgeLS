pragma solidity ^0.8.24;

contract A {
    event Log(string message);

    function foo() public virtual {
        emit Log("A");
    }
}

contract B is A {
    function foo() public override {
        super.foo();
    }
}
