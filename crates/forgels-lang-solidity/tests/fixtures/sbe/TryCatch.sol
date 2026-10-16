pragma solidity ^0.8.24;

contract External {
    function foo() public pure returns (uint) {
        return 1;
    }
}

contract TryCatch {
    event Log(string message);

    function attempt(address target) public returns (uint) {
        try External(target).foo() returns (uint x) {
            return x;
        } catch {
            emit Log("fail");
        }
    }
}
