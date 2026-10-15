pragma solidity ^0.8.0;
pragma experimental ABIEncoderV2;

import "./A.sol";
import "./B.sol" as NS;
import * as All from "./C.sol";
import {A as B, C} from "./D.sol";

abstract contract Token is A, B {
}
