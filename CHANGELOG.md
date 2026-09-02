# Changelog

## [0.20.0](https://github.com/mightea/MotoManager-Api/compare/v0.19.0...v0.20.0) (2026-09-02)


### Features

* MCP server with personal API tokens ([f4293da](https://github.com/mightea/MotoManager-Api/commit/f4293da9bb0d812d449dc2f0a719de74cbc76e15))
* **mcp:** OAuth 2.1 authorization server for connector clients ([80c1ae2](https://github.com/mightea/MotoManager-Api/commit/80c1ae280c994a7aa13215e93d936b9f2862e152))


### Bug Fixes

* **mcp:** emit ttlMs/cacheScope on tools/list for MCP 2026-07-28 clients ([63d3789](https://github.com/mightea/MotoManager-Api/commit/63d37894eb684efa098986e85f024673ea870e13))

## [0.19.0](https://github.com/mightea/MotoManager-Api/compare/v0.18.1...v0.19.0) (2026-08-31)


### Features

* admin impersonation for user support ([3daa514](https://github.com/mightea/MotoManager-Api/commit/3daa51463ba4b7ad0388d0e18f131ddf5557c420))
* match F 650 GS VINs via type code 0172 ([4d5fd4a](https://github.com/mightea/MotoManager-Api/commit/4d5fd4a46c8632eb8f312d67be2d9f41ec669c65))

## [0.18.1](https://github.com/mightea/MotoManager-Api/compare/v0.18.0...v0.18.1) (2026-08-23)


### Bug Fixes

* recognize workshop location changes ([548e6f8](https://github.com/mightea/MotoManager-Api/commit/548e6f8b5a128243f6c8a4c3076133dd6da1b65a))

## [0.18.0](https://github.com/mightea/MotoManager-Api/compare/v0.17.0...v0.18.0) (2026-08-22)


### Features

* add admin-configurable app upgrade build requirements ([a5d3273](https://github.com/mightea/MotoManager-Api/commit/a5d3273f18b859c399d07cbfc5a9792786004589))
* record the ios app version a user last connected with ([7c47ca3](https://github.com/mightea/MotoManager-Api/commit/7c47ca346eb9f5f80f9e9b56a3212d9214adecd7))

## [0.17.0](https://github.com/mightea/MotoManager-Api/compare/v0.16.1...v0.17.0) (2026-08-21)


### Features

* cost consumed parts onto their maintenance record ([c50e605](https://github.com/mightea/MotoManager-Api/commit/c50e6058c224ea70dd7159124b5cdaf28c09d8d7))
* create a missing sqlite database file on startup ([3876304](https://github.com/mightea/MotoManager-Api/commit/3876304ea783003f514ee5db95a047c50ffe3f4c))


### Bug Fixes

* include per-user averages in the stats response ([59b6ab5](https://github.com/mightea/MotoManager-Api/commit/59b6ab51be7120537996b77114a01b03ec2fd8cc))

## [0.16.1](https://github.com/mightea/MotoManager-Api/compare/v0.16.0...v0.16.1) (2026-08-07)


### Bug Fixes

* derive the OpenAPI info.version from the crate version ([b652a9b](https://github.com/mightea/MotoManager-Api/commit/b652a9bfd75f3f464ee5f8591bb68a58fdf820d9))

## [0.16.0](https://github.com/mightea/MotoManager-Api/compare/v0.15.0...v0.16.0) (2026-07-21)


### Features

* persist previous owner ordering ([75480d4](https://github.com/mightea/MotoManager-Api/commit/75480d49e9a78e3ff7e0ce68940f9f9830a17c76))
* publish API contract and harden deployment ([9db6046](https://github.com/mightea/MotoManager-Api/commit/9db60466c04b0737440f09b5a4eefc100a595197))


### Bug Fixes

* restore root backend runtime ([ffa51e3](https://github.com/mightea/MotoManager-Api/commit/ffa51e32d4bdf27cbf1a90a6fbf61a0fbbcb9781))

## [0.15.0](https://github.com/mightea/MotoManager-Api/compare/v0.14.0...v0.15.0) (2026-07-17)


### Features

* motorcycle details (Title/Value pairs) with offline sync ([dd86744](https://github.com/mightea/MotoManager-Api/commit/dd867441085b14ad3e83f349f0a337f89e820597))

## [0.14.0](https://github.com/mightea/MotoManager-Api/compare/v0.13.0...v0.14.0) (2026-07-17)


### Features

* fuel additive and lead substitute flags on fuel records ([1db118c](https://github.com/mightea/MotoManager-Api/commit/1db118c4252dab78703ea6542510ad719b19e7cd))
* motorcycle status (active/archived/sold) and sale details ([67e19a1](https://github.com/mightea/MotoManager-Api/commit/67e19a149ca216281c305ab4782a45e08afcd908))

## [0.13.0](https://github.com/mightea/MotoManager-Api/compare/v0.12.0...v0.13.0) (2026-07-15)


### Features

* add motorcycle drive type (chain/shaft) ([f3f87de](https://github.com/mightea/MotoManager-Api/commit/f3f87de1a2110017e7a269c868d32b9e2cfb2a5a))
* add per-wheel brake type to motorcycles ([2de5995](https://github.com/mightea/MotoManager-Api/commit/2de5995627b50f3cf630bc986f32c5808820eede))

## [0.12.0](https://github.com/mightea/MotoManager-Api/compare/v0.11.0...v0.12.0) (2026-07-14)


### Features

* add configurable final-drive gearbox oil interval ([5c6686c](https://github.com/mightea/MotoManager-Api/commit/5c6686cd03dd470641dec409ba7b1a69081187c8))
* add locations merge endpoint for de-duplication ([58fd3fa](https://github.com/mightea/MotoManager-Api/commit/58fd3fa552c5afb34020578f1f6461415b224fd7))
* add minimum yearly kilometres setting ([1638366](https://github.com/mightea/MotoManager-Api/commit/16383662cfac47c454495d677d2791d53a64b127))
* add proximity search to the locations API ([acf1191](https://github.com/mightea/MotoManager-Api/commit/acf119120ecc003fde7519c9aaac61e10c4ea091))


### Bug Fixes

* detach storage locations when deleting a location ([2d60cce](https://github.com/mightea/MotoManager-Api/commit/2d60cce0846d26e14f65be6cd3e810e88895b99a))

## [0.11.0](https://github.com/mightea/MotoManager-Api/compare/v0.10.0...v0.11.0) (2026-07-13)


### Features

* skip scheduled backup when nothing changed ([5a24b39](https://github.com/mightea/MotoManager-Api/commit/5a24b395f73accf0085136c15dd5410aff17b4d8))

## [0.10.0](https://github.com/mightea/MotoManager-Api/compare/v0.9.0...v0.10.0) (2026-07-13)


### Features

* add in-process database and file backups with admin monitoring ([80488db](https://github.com/mightea/MotoManager-Api/commit/80488dbbe1d9fabf23b9ddf7bdfb08d986c5c849))
* record frontend/backend versions in backups ([3a8bf83](https://github.com/mightea/MotoManager-Api/commit/3a8bf837c7f1bb3c0fd6759842d299a8ad5d8e46))

## [0.9.0](https://github.com/mightea/MotoManager-Api/compare/v0.8.0...v0.9.0) (2026-07-12)


### Features

* flag motorcycles with incomplete ownership history ([a2deae4](https://github.com/mightea/MotoManager-Api/commit/a2deae4fbf78ffd2463b2c24285aa1be1f8d7ace))
* flag torque specs from uncertain sources as unverified ([a2f126a](https://github.com/mightea/MotoManager-Api/commit/a2f126af18b67d0609e8e8a813153b183949c7dd))
* per-configuration tire pressures and motorcycle sidecar flag ([8fc208c](https://github.com/mightea/MotoManager-Api/commit/8fc208cfbf857c1798da6074ce34ab2660ba3ffc))


### Bug Fixes

* accept session token as query param on document file routes ([2b78fb0](https://github.com/mightea/MotoManager-Api/commit/2b78fb05e59d2d9070fb7a536809e8ee47dc0099))

## [0.8.0](https://github.com/mightea/MotoManager-Api/compare/v0.7.0...v0.8.0) (2026-07-10)


### Features

* add parts inventory with model series, storage locations and part images ([52ead64](https://github.com/mightea/MotoManager-Api/commit/52ead64e340ac03dc2fe93bea7e8a8087ba384ba))
* add R 80 GS PD (CH) catalog model with its Swiss serial block ([2797f70](https://github.com/mightea/MotoManager-Api/commit/2797f70d8e9164651429bd3acbbdade7704899c7))
* add the 1980-84 R 100 catalog model with its frame block ([f52e125](https://github.com/mightea/MotoManager-Api/commit/f52e12504946eed9c8e367fb06d0a2b7687183ac))
* anchor root storage locations to workshop/garage places ([5e8f77e](https://github.com/mightea/MotoManager-Api/commit/5e8f77e1c60df1cb1a1fa03364bc5253d2aefa34))
* decode classic frame numbers and fix motorcycle field merging ([89378b4](https://github.com/mightea/MotoManager-Api/commit/89378b4eddf281c2b48d5d808f53c9911aa8be09))
* hierarchical model catalog with curation and VIN decoding ([35352a2](https://github.com/mightea/MotoManager-Api/commit/35352a25d15e0f2d31c471dc41112655eed4c78b))
* mark part stock entries as used/salvaged parts ([e8d0aae](https://github.com/mightea/MotoManager-Api/commit/e8d0aae030d8f333f3a56d6f271cdd804a83b962))
* mirror the BMWBike.com catalog and refine VIN assignment depth ([ce8c536](https://github.com/mightea/MotoManager-Api/commit/ce8c53638e4ea35427b67cfcd445a2e9f365f475))
* parse supplier-invoice PDFs into reviewed part imports ([cfd5d74](https://github.com/mightea/MotoManager-Api/commit/cfd5d7443631dcf805238c10cd023f73c94d462d))

## [0.7.0](https://github.com/mightea/MotoManager-Api/compare/v0.6.2...v0.7.0) (2026-07-04)


### Features

* harden, optimize and document the API with an offline-sync backend ([a62b2c9](https://github.com/mightea/MotoManager-Api/commit/a62b2c9416e9b163d69dcfc5c9d3227924c7d0d6))

## [0.6.2](https://github.com/mightea/MotoManager-Api/compare/v0.6.1...v0.6.2) (2026-06-12)


### Performance Improvements

* add indexes for per-motorcycle lookups ([5cc062a](https://github.com/mightea/MotoManager-Api/commit/5cc062add2929bdeb50f29457e4939571f5e3a57))
* eliminate n+1 queries in expense and document listings ([88446ce](https://github.com/mightea/MotoManager-Api/commit/88446ce5a20e611aa7b32426157967e23f8ae958))
* enable wal journal mode and tuned sqlite pragmas ([b97f22a](https://github.com/mightea/MotoManager-Api/commit/b97f22a544cc91a2886c4850de74aa19ac68d2dc))
* offload image resizing to blocking thread pool ([19e105b](https://github.com/mightea/MotoManager-Api/commit/19e105b898be9bff72e35f571b800a81d8727f6d))

## [0.6.1](https://github.com/mightea/MotoManager-Api/compare/v0.6.0...v0.6.1) (2026-06-05)


### Bug Fixes

* allow deleting a location that is still referenced ([612fe65](https://github.com/mightea/MotoManager-Api/commit/612fe65b9960cb5ccba4ba8c7046474cf355f49c))

## [0.6.0](https://github.com/mightea/MotoManager-Api/compare/v0.5.0...v0.6.0) (2026-06-04)


### Features

* derive a motorcycle's current location only from storage locations ([d1f38c9](https://github.com/mightea/MotoManager-Api/commit/d1f38c9f78552757221068d094cd7c09bf856615))


### Bug Fixes

* revert migration 005 content so sqlx checksum validates again ([d9f5796](https://github.com/mightea/MotoManager-Api/commit/d9f5796b0e7bc21be6838ac1b09fe0d3812a8089))

## [0.5.0](https://github.com/mightea/MotoManager-Api/compare/v0.4.1...v0.5.0) (2026-06-01)


### Features

* add tire pressure endpoint per motorcycle ([9cb4e6d](https://github.com/mightea/MotoManager-Api/commit/9cb4e6d9e46a5119bab862f8f4feedf6f45f602e))
* typed locations, issue titles, and startup migration verification ([c15cd7b](https://github.com/mightea/MotoManager-Api/commit/c15cd7b07642e8ba2b9d0232fa1a254ca39d8edc))


### Bug Fixes

* apply rustfmt and add required title to issue lifecycle test ([4efd37c](https://github.com/mightea/MotoManager-Api/commit/4efd37c3b6459ff6766138aeac3eb7b805643191))

## [0.4.1](https://github.com/mightea/MotoManager-Api/compare/v0.4.0...v0.4.1) (2026-05-24)


### Bug Fixes

* collapse if-is_owner into match guards in document update ([dfb127a](https://github.com/mightea/MotoManager-Api/commit/dfb127a2ed963cc329258429a5568c3408935fa1))

## [0.4.0](https://github.com/mightea/MotoManager-Api/compare/v0.3.0...v0.4.0) (2026-05-24)


### Features

* add filtering by type to maintenance records list ([9daa3a3](https://github.com/mightea/MotoManager-Api/commit/9daa3a3961c2a904a7c94685e673ecb3b5b4e4f1))
* import torque specs by id list instead of source motorcycle ([8f17a5f](https://github.com/mightea/MotoManager-Api/commit/8f17a5fd8a7cc75ece16f1c7407e86d31783a5d7))
* include userId on allMotorcycles in /documents response ([af32480](https://github.com/mightea/MotoManager-Api/commit/af324801c0c2d02e7aa5cadf8ab2caa749743dd8))


### Bug Fixes

* use workflow_run to trigger docker publish from release-please ([7e69ff3](https://github.com/mightea/MotoManager-Api/commit/7e69ff30bc771f231e3e5e9ca3d2498b61e8d130))

## [0.3.0](https://github.com/mightea/MotoManager-Api/compare/v0.2.0...v0.3.0) (2026-04-14)


### Features

* implement shared expenses tracking across multiple motorcycles ([b5e6cde](https://github.com/mightea/MotoManager-Api/commit/b5e6cde443ab07edf5a3656f28df6fd5e2505463))
* include motorcycle owners in document motorcycle list ([2517baa](https://github.com/mightea/MotoManager-Api/commit/2517baa07bee2bb39b6bec3fa010b590c16c37ab))


### Bug Fixes

* update CI to include all migrations for sqlx macro validation ([2fbe5e1](https://github.com/mightea/MotoManager-Api/commit/2fbe5e1e467959befd78390736124c67ca77066a))

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
