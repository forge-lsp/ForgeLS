pragma solidity ^0.8.24;

interface IERC1155 {
    function balanceOf(address account, uint id) external view returns (uint);
    function mint(address to, uint id, uint amount) external;
}

contract ERC1155 is IERC1155 {
    mapping(uint => mapping(address => uint)) public balances;

    event TransferSingle(address indexed sender, address indexed to, uint id, uint amount);

    function balanceOf(address account, uint id) public view override returns (uint) {
        uint bal = balances[id][account];
        return bal;
    }

    function mint(address to, uint id, uint amount) public override {
        uint before = balances[id][to];
        before = amount;
    }
}
