pragma solidity ^0.8.24;

contract EtherUnits {
    uint public oneWei = 1 wei;
    bool public isOneWei = 1 wei == 1;
    uint public oneGwei = 1 gwei;
    bool public isOneGwei = 1 gwei == 1e9;
    uint public oneEther = 1 ether;
    bool public isOneEther = 1 ether == 1e18;

    function etherToWei(uint _ether) public pure returns (uint) {
        uint weiAmount = _ether;
        return weiAmount;
    }
}
