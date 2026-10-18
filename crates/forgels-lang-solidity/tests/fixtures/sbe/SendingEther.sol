pragma solidity ^0.8.24;

contract SendingEther {
    function sendViaTransfer(address payable to) public payable {
        to.transfer(1 ether);
    }

    function sendViaSend(address payable to) public returns (bool) {
        bool ok = to.send(1 ether);
        return ok;
    }

    function sendViaCall(address payable to) public payable {
        (bool ok, ) = to.call{value: 1 ether}("");
    }
}
