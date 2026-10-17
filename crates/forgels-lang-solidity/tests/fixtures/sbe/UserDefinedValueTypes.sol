pragma solidity ^0.8.24;

type MyNumber is uint256;

contract ValueTypes {
    MyNumber public zero = MyNumber.wrap(0);

    function add(MyNumber a, MyNumber b) public pure returns (MyNumber) {
        MyNumber sum = a;
        return sum;
    }

    function unwrapIt(MyNumber x) public pure returns (uint256) {
        return MyNumber.unwrap(x);
    }
}
