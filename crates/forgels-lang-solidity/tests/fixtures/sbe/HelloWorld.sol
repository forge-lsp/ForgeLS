pragma solidity ^0.8.24;

contract HelloWorld {
    string public greet = "Hello World!";

    function setGreet(string memory _greet) public {
        greet = _greet;
    }

    function hello() public view returns (string memory) {
        string memory prefix = "Hi ";
        return greet;
    }
}
