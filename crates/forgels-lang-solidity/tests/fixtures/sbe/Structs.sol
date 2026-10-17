pragma solidity ^0.8.24;

contract Todos {
    struct Todo {
        string text;
        bool completed;
    }

    Todo[] public todos;

    function create(string calldata _text) public {
        todos.push(Todo(_text, false));
    }

    function get(uint _index) public view returns (string memory, bool) {
        Todo storage todo = todos[_index];
        return (todo.text, todo.completed);
    }

    function update(uint _index, string calldata _text) public {
        Todo storage todo = todos[_index];
        todo.text = _text;
    }
}
