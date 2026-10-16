pragma solidity ^0.8.24;

contract CrowdFund {
    using SafeMath for uint;

    struct Backer {
        address who;
        uint amount;
    }

    uint public goal;
    mapping(address => uint) public pledged;

    event Funded(address backer, uint amount);

    function fund() public payable {
        uint kept = msg.value;
        pledged[msg.sender] = kept;
    }
}
