/*!
 * cw-tokenfactory-issuer-sdk v 0.0.1
 * (c) Sunny Aggarwal <sunnya97@protonmail.ch>
 * Released under the MIT OR Apache-2.0 License.
 */

(function (global, factory) {
    typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
    typeof define === 'function' && define.amd ? define(['exports'], factory) :
    (global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global["counter-sdk"] = {}));
})(this, (function (exports) { 'use strict';

    /******************************************************************************
    Copyright (c) Microsoft Corporation.

    Permission to use, copy, modify, and/or distribute this software for any
    purpose with or without fee is hereby granted.

    THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
    REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
    AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
    INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM
    LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR
    OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
    PERFORMANCE OF THIS SOFTWARE.
    ***************************************************************************** */
    /* global Reflect, Promise */

    var extendStatics = function(d, b) {
        extendStatics = Object.setPrototypeOf ||
            ({ __proto__: [] } instanceof Array && function (d, b) { d.__proto__ = b; }) ||
            function (d, b) { for (var p in b) if (Object.prototype.hasOwnProperty.call(b, p)) d[p] = b[p]; };
        return extendStatics(d, b);
    };

    function __extends(d, b) {
        if (typeof b !== "function" && b !== null)
            throw new TypeError("Class extends value " + String(b) + " is not a constructor or null");
        extendStatics(d, b);
        function __() { this.constructor = d; }
        d.prototype = b === null ? Object.create(b) : (__.prototype = b.prototype, new __());
    }

    var __assign = function() {
        __assign = Object.assign || function __assign(t) {
            for (var s, i = 1, n = arguments.length; i < n; i++) {
                s = arguments[i];
                for (var p in s) if (Object.prototype.hasOwnProperty.call(s, p)) t[p] = s[p];
            }
            return t;
        };
        return __assign.apply(this, arguments);
    };

    function __awaiter(thisArg, _arguments, P, generator) {
        function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
        return new (P || (P = Promise))(function (resolve, reject) {
            function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
            function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
            function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
            step((generator = generator.apply(thisArg, _arguments || [])).next());
        });
    }

    function __generator(thisArg, body) {
        var _ = { label: 0, sent: function() { if (t[0] & 1) throw t[1]; return t[1]; }, trys: [], ops: [] }, f, y, t, g;
        return g = { next: verb(0), "throw": verb(1), "return": verb(2) }, typeof Symbol === "function" && (g[Symbol.iterator] = function() { return this; }), g;
        function verb(n) { return function (v) { return step([n, v]); }; }
        function step(op) {
            if (f) throw new TypeError("Generator is already executing.");
            while (_) try {
                if (f = 1, y && (t = op[0] & 2 ? y["return"] : op[0] ? y["throw"] || ((t = y["return"]) && t.call(y), 0) : y.next) && !(t = t.call(y, op[1])).done) return t;
                if (y = 0, t) op = [op[0] & 2, t.value];
                switch (op[0]) {
                    case 0: case 1: t = op; break;
                    case 4: _.label++; return { value: op[1], done: false };
                    case 5: _.label++; y = op[1]; op = [0]; continue;
                    case 7: op = _.ops.pop(); _.trys.pop(); continue;
                    default:
                        if (!(t = _.trys, t = t.length > 0 && t[t.length - 1]) && (op[0] === 6 || op[0] === 2)) { _ = 0; continue; }
                        if (op[0] === 3 && (!t || (op[1] > t[0] && op[1] < t[3]))) { _.label = op[1]; break; }
                        if (op[0] === 6 && _.label < t[1]) { _.label = t[1]; t = op; break; }
                        if (t && _.label < t[2]) { _.label = t[2]; _.ops.push(op); break; }
                        if (t[2]) _.ops.pop();
                        _.trys.pop(); continue;
                }
                op = body.call(thisArg, _);
            } catch (e) { op = [6, e]; y = 0; } finally { f = t = 0; }
            if (op[0] & 5) throw op[1]; return { value: op[0] ? op[1] : void 0, done: true };
        }
    }

    /**
    * This file was automatically generated by @cosmwasm/ts-codegen@0.16.5.
    * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
    * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
    */

    var _0 = /*#__PURE__*/Object.freeze({
        __proto__: null
    });

    /**
    * This file was automatically generated by @cosmwasm/ts-codegen@0.16.5.
    * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
    * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
    */
    var TokenfactoryIssuerQueryClient = /** @class */ (function () {
        function TokenfactoryIssuerQueryClient(client, contractAddress) {
            var _this = this;
            this.isFrozen = function () { return __awaiter(_this, void 0, void 0, function () {
                return __generator(this, function (_a) {
                    return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                            is_frozen: {}
                        })];
                });
            }); };
            this.denom = function () { return __awaiter(_this, void 0, void 0, function () {
                return __generator(this, function (_a) {
                    return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                            denom: {}
                        })];
                });
            }); };
            this.owner = function () { return __awaiter(_this, void 0, void 0, function () {
                return __generator(this, function (_a) {
                    return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                            owner: {}
                        })];
                });
            }); };
            this.burnAllowance = function (_a) {
                var address = _a.address;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                burn_allowance: {
                                    address: address
                                }
                            })];
                    });
                });
            };
            this.burnAllowances = function (_a) {
                var limit = _a.limit, startAfter = _a.startAfter;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                burn_allowances: {
                                    limit: limit,
                                    start_after: startAfter
                                }
                            })];
                    });
                });
            };
            this.mintAllowance = function (_a) {
                var address = _a.address;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                mint_allowance: {
                                    address: address
                                }
                            })];
                    });
                });
            };
            this.mintAllowances = function (_a) {
                var limit = _a.limit, startAfter = _a.startAfter;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                mint_allowances: {
                                    limit: limit,
                                    start_after: startAfter
                                }
                            })];
                    });
                });
            };
            this.isBlacklisted = function (_a) {
                var address = _a.address;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                is_blacklisted: {
                                    address: address
                                }
                            })];
                    });
                });
            };
            this.blacklistees = function (_a) {
                var limit = _a.limit, startAfter = _a.startAfter;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                blacklistees: {
                                    limit: limit,
                                    start_after: startAfter
                                }
                            })];
                    });
                });
            };
            this.isBlacklister = function (_a) {
                var address = _a.address;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                is_blacklister: {
                                    address: address
                                }
                            })];
                    });
                });
            };
            this.blacklisterAllowances = function (_a) {
                var limit = _a.limit, startAfter = _a.startAfter;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                blacklister_allowances: {
                                    limit: limit,
                                    start_after: startAfter
                                }
                            })];
                    });
                });
            };
            this.isFreezer = function (_a) {
                var address = _a.address;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                is_freezer: {
                                    address: address
                                }
                            })];
                    });
                });
            };
            this.freezerAllowances = function (_a) {
                var limit = _a.limit, startAfter = _a.startAfter;
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        return [2 /*return*/, this.client.queryContractSmart(this.contractAddress, {
                                freezer_allowances: {
                                    limit: limit,
                                    start_after: startAfter
                                }
                            })];
                    });
                });
            };
            this.client = client;
            this.contractAddress = contractAddress;
            this.isFrozen = this.isFrozen.bind(this);
            this.denom = this.denom.bind(this);
            this.owner = this.owner.bind(this);
            this.burnAllowance = this.burnAllowance.bind(this);
            this.burnAllowances = this.burnAllowances.bind(this);
            this.mintAllowance = this.mintAllowance.bind(this);
            this.mintAllowances = this.mintAllowances.bind(this);
            this.isBlacklisted = this.isBlacklisted.bind(this);
            this.blacklistees = this.blacklistees.bind(this);
            this.isBlacklister = this.isBlacklister.bind(this);
            this.blacklisterAllowances = this.blacklisterAllowances.bind(this);
            this.isFreezer = this.isFreezer.bind(this);
            this.freezerAllowances = this.freezerAllowances.bind(this);
        }
        return TokenfactoryIssuerQueryClient;
    }());
    var TokenfactoryIssuerClient = /** @class */ (function (_super) {
        __extends(TokenfactoryIssuerClient, _super);
        function TokenfactoryIssuerClient(client, sender, contractAddress) {
            var _this = _super.call(this, client, contractAddress) || this;
            _this.changeTokenFactoryAdmin = function (_a, fee, memo, funds) {
                var newAdmin = _a.newAdmin;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    change_token_factory_admin: {
                                        new_admin: newAdmin
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.changeContractOwner = function (_a, fee, memo, funds) {
                var newOwner = _a.newOwner;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    change_contract_owner: {
                                        new_owner: newOwner
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.setMinter = function (_a, fee, memo, funds) {
                var address = _a.address, allowance = _a.allowance;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    set_minter: {
                                        address: address,
                                        allowance: allowance
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.setBurner = function (_a, fee, memo, funds) {
                var address = _a.address, allowance = _a.allowance;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    set_burner: {
                                        address: address,
                                        allowance: allowance
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.setBlacklister = function (_a, fee, memo, funds) {
                var address = _a.address, status = _a.status;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    set_blacklister: {
                                        address: address,
                                        status: status
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.setFreezer = function (_a, fee, memo, funds) {
                var address = _a.address, status = _a.status;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    set_freezer: {
                                        address: address,
                                        status: status
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.mint = function (_a, fee, memo, funds) {
                var amount = _a.amount, toAddress = _a.toAddress;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    mint: {
                                        amount: amount,
                                        to_address: toAddress
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.burn = function (_a, fee, memo, funds) {
                var amount = _a.amount, fromAddress = _a.fromAddress;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    burn: {
                                        amount: amount,
                                        from_address: fromAddress
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.blacklist = function (_a, fee, memo, funds) {
                var address = _a.address, status = _a.status;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    blacklist: {
                                        address: address,
                                        status: status
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.freeze = function (_a, fee, memo, funds) {
                var status = _a.status;
                if (fee === void 0) { fee = "auto"; }
                return __awaiter(_this, void 0, void 0, function () {
                    return __generator(this, function (_b) {
                        switch (_b.label) {
                            case 0: return [4 /*yield*/, this.client.execute(this.sender, this.contractAddress, {
                                    freeze: {
                                        status: status
                                    }
                                }, fee, memo, funds)];
                            case 1: return [2 /*return*/, _b.sent()];
                        }
                    });
                });
            };
            _this.client = client;
            _this.sender = sender;
            _this.contractAddress = contractAddress;
            _this.changeTokenFactoryAdmin = _this.changeTokenFactoryAdmin.bind(_this);
            _this.changeContractOwner = _this.changeContractOwner.bind(_this);
            _this.setMinter = _this.setMinter.bind(_this);
            _this.setBurner = _this.setBurner.bind(_this);
            _this.setBlacklister = _this.setBlacklister.bind(_this);
            _this.setFreezer = _this.setFreezer.bind(_this);
            _this.mint = _this.mint.bind(_this);
            _this.burn = _this.burn.bind(_this);
            _this.blacklist = _this.blacklist.bind(_this);
            _this.freeze = _this.freeze.bind(_this);
            return _this;
        }
        return TokenfactoryIssuerClient;
    }(TokenfactoryIssuerQueryClient));

    var _1 = /*#__PURE__*/Object.freeze({
        __proto__: null,
        TokenfactoryIssuerQueryClient: TokenfactoryIssuerQueryClient,
        TokenfactoryIssuerClient: TokenfactoryIssuerClient
    });

    /**
    * This file was automatically generated by @cosmwasm/ts-codegen@0.16.5.
    * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
    * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
    */
    exports.contracts = void 0;
    (function (contracts) {
        contracts.TokenfactoryIssuer = __assign(__assign({}, _0), _1);
    })(exports.contracts || (exports.contracts = {}));

    Object.defineProperty(exports, '__esModule', { value: true });

}));
//# sourceMappingURL=index.umd.js.map
