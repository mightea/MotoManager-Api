# Changelog

## [0.2.0](https://github.com/mightea/MotoManager-Api/compare/v0.1.1...v0.2.0) (2026-04-13)


### Features

* add parentId and bundled maintenance items support with reconciliation logic ([934b0df](https://github.com/mightea/MotoManager-Api/commit/934b0dfb54288c3563863c4cba06c87905fd6753))
* handle bundled_items in update_maintenance handler ([d21f586](https://github.com/mightea/MotoManager-Api/commit/d21f586beca0e8d76d423f5b33868baa4d7dc332))
* include maintenanceLocations in motorcycle detail response ([690b737](https://github.com/mightea/MotoManager-Api/commit/690b73757a276e2f666977424e57dc70f6f9c13e))

## [0.1.1](https://github.com/mightea/MotoManager-Api/compare/v0.1.0...v0.1.1) (2026-04-08)


### Bug Fixes

* include busiest bike information in home data response ([62527e1](https://github.com/mightea/MotoManager-Api/commit/62527e17fe0389e2a0840d6680bc9d6d2d426d96))
* return busiest bike as a formatted string instead of a full object ([904548f](https://github.com/mightea/MotoManager-Api/commit/904548f1f9527404e4c3f06ec43d0185536dbc64))

## 0.1.0 (2026-03-19)


### Features

* add /api/home route and refactor models to use global camelCase ([e6e53d3](https://github.com/mightea/MotoManager-Api/commit/e6e53d346d6ccc3692a0983a1c01daa5d54cb3ea))
* add info logs to passkey endpoints for better traceability ([e6a3efb](https://github.com/mightea/MotoManager-Api/commit/e6a3efb9d6ec6af634e957fcf100fa5d842752c2))
* add production Docker support and PDF preview generation ([43c273e](https://github.com/mightea/MotoManager-Api/commit/43c273eb9e3485a405bc0cc2ea77f7b2b207b25e))
* add tracing logs and enhance stats API ([e5fa0a0](https://github.com/mightea/MotoManager-Api/commit/e5fa0a0f738168bc08e09caf4804dd3232b0bba1))
* enhance stats and documents API, and add passkey support ([48ff4f6](https://github.com/mightea/MotoManager-Api/commit/48ff4f65f119b2a91c5c96f1b344a1844c6b8e96))
* fix CORS, add WebP support, and simplify file routes ([a77f1de](https://github.com/mightea/MotoManager-Api/commit/a77f1de0da270e73218a7523346be1d3cddb1ca3))
* ignore inspections for motorcycles with no recorded inspection records ([c73281d](https://github.com/mightea/MotoManager-Api/commit/c73281d11d81a4c8786454d9c13345fdbe0bf221))
* implement caching system for resized images and previews ([1305e2d](https://github.com/mightea/MotoManager-Api/commit/1305e2dbd53a0dd80835acbfd84228d825333fb8))
* implement Swiss MFK inspection logic and overdue maintenance calculation ([d9b3cdb](https://github.com/mightea/MotoManager-Api/commit/d9b3cdb5cc9d8f21b7d97ff8a0c21c11fc1c9128))
* include associated documents in motorcycle details and fix cache filename logic ([9b250d5](https://github.com/mightea/MotoManager-Api/commit/9b250d5cf20e1fc768448b9e1fb495bbad27818b))
* migrate fleet statistics aggregation to backend ([97b511e](https://github.com/mightea/MotoManager-Api/commit/97b511e019e35316603fdd4dad17c9c13d8f4355))
* read application version from Cargo.toml at compile time ([640b9df](https://github.com/mightea/MotoManager-Api/commit/640b9df9624da837015ea764b4c427681b3dca4d))
* robust statistics aggregation and authenticator management ([c93f744](https://github.com/mightea/MotoManager-Api/commit/c93f744af5a320f990956a1c5edb4ea339fa8757))


### Bug Fixes

* correctly serialize and retrieve Passkey objects for WebAuthn login ([5dbb8e3](https://github.com/mightea/MotoManager-Api/commit/5dbb8e3c7a718efe080c40ce14e3a2a962e7e166))
* drop all legacy tables at migration start to ensure clean schema ([ab1d99e](https://github.com/mightea/MotoManager-Api/commit/ab1d99ead71929f255445a33397e17cb69531401))
* improve discoverable passkey login by allowing all known credentials ([38cdceb](https://github.com/mightea/MotoManager-Api/commit/38cdceb8c80109f23b0b36ea6f22e6593336b59c))
* improve Passkey login by correctly decoding credential IDs and handling stateless verification ([2b7e880](https://github.com/mightea/MotoManager-Api/commit/2b7e8801dffd56e22f17e18b337dc31ce66be00e))
* include maintenance records in currentLocation calculation ([7ca57cd](https://github.com/mightea/MotoManager-Api/commit/7ca57cdcfd1f93b367dfa0c5f1cd0979e4bf7713))
* include torqueSpecifications in motorcycle and torque_specs handlers ([d6e858c](https://github.com/mightea/MotoManager-Api/commit/d6e858c748111a8769c62c84347aa5d1bbe3f108))
* resolve all build warnings and clippy errors ([51c4fa0](https://github.com/mightea/MotoManager-Api/commit/51c4fa05d0883b6e6e107523bb8eadf80dd22562))
* resolve Passkey login issues by correctly handling credential ID encoding and deserialization ([d8bc6d1](https://github.com/mightea/MotoManager-Api/commit/d8bc6d15e6d9ff1921ea8dc5ba1177d3dabbe868))
* restore snake_case base schema and add data-preserving camelCase migration ([aa5f069](https://github.com/mightea/MotoManager-Api/commit/aa5f069106538076174f98bc7c88b60e488e6156))
* robust preview regeneration and path prefix handling ([8542a5e](https://github.com/mightea/MotoManager-Api/commit/8542a5e12109618df75efe393953e38a74bbf53f))
* support discoverable passkey login (usernameless) ([57ef3c6](https://github.com/mightea/MotoManager-Api/commit/57ef3c68ac5dfd428ff9d7369fd08eec79f9513d))
