# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.10] - 2026-06-28

### ⛰️  Features

- Add graph panel options model ([6783c29](https://github.com/fedexist/grafatui/commit/6783c293e0dbb969ac1fa065b9af1f8af5089b5c))
- Import grafana timeseries options ([3f8e3ed](https://github.com/fedexist/grafatui/commit/3f8e3ed40acc396f9b05b817b6098459ac451dd6))
- Render grafana graph styles ([3ad51de](https://github.com/fedexist/grafatui/commit/3ad51de1b43f0503741f5b74b363a9de55cdb75c))
- Export grafana graph styles ([d70cd97](https://github.com/fedexist/grafatui/commit/d70cd97909a813ca493490d1f8670abd47f70cee))

### 🐛 Bug Fixes

- Preserve graph overlay alignment ([89b3b3c](https://github.com/fedexist/grafatui/commit/89b3b3c2a7b78794cba0d41c4c6912fbb41438b9))
- Align graph plot bounds ([7faf165](https://github.com/fedexist/grafatui/commit/7faf165363fb237a6e1b2165f46702253d90c9b1))
- Keep graph overlays visible ([e511d30](https://github.com/fedexist/grafatui/commit/e511d30aca64e2bd8959ba21fe14eb009a0374d4))
- Harden graph style export ([8dcf1a8](https://github.com/fedexist/grafatui/commit/8dcf1a81785880e969427492c4047f3142e4623b))
- Align demo dashboard instance default ([44f7c66](https://github.com/fedexist/grafatui/commit/44f7c6698391252534f165fdb49d3f7e51ba8e93))
- Keep autogrid behind area fill ([7f619e9](https://github.com/fedexist/grafatui/commit/7f619e98f9170d80f9fd7d5a4c0fb63e41166d5f))
- Align forced graph points ([d46bcea](https://github.com/fedexist/grafatui/commit/d46bceaffe8cd98c74db6ed736916c683bb9b2f7))
- Show aligned forced graph points ([95966d6](https://github.com/fedexist/grafatui/commit/95966d6713018b55a17a341429c72a8a2a5bec96))

### 📚 Documentation

- Update rust msrv references ([efbf225](https://github.com/fedexist/grafatui/commit/efbf225ec35947c28685a2d1b3a8745fce066dcc))
- Document graph style support ([0ca3fc4](https://github.com/fedexist/grafatui/commit/0ca3fc406a0f6291d8d0026922dbdaa65a8719eb))
- Refresh graph custom compatibility ([692073f](https://github.com/fedexist/grafatui/commit/692073feabeda9e55e71eae315fc1ac33882c488))
- Refresh timeseries style coverage ([095abc5](https://github.com/fedexist/grafatui/commit/095abc504242c98dbeefe70a6e689f988c753938))

### ⚙️ Miscellaneous Tasks

- Update ratatui to 0.30.1 ([c84cd67](https://github.com/fedexist/grafatui/commit/c84cd6786407fb5baae7eb8f863223aea3792498))
- Wire panel options through constructors ([ac2519a](https://github.com/fedexist/grafatui/commit/ac2519a45ad66645c16cb4a38f93f6096a547e18))
- Ignore docs/superpowers directory in git tracking ([b1a7193](https://github.com/fedexist/grafatui/commit/b1a7193e33dec6e9af21fbe7025abb76cc7d95f7))


## [0.1.9] - 2026-06-12

### 🐛 Bug Fixes

- **graph:** Honor explicit y-axis bounds ([832afd9](https://github.com/fedexist/grafatui/commit/832afd9fcf095a194d64316153f42e77d238a938))

### 📚 Documentation

- Point contributors to user guide ([bf73a4f](https://github.com/fedexist/grafatui/commit/bf73a4fa3b05c591ddfe8836cdf0565e1764d74d))


## [0.1.8] - 2026-06-07

### ⛰️  Features

- Honor dashboard refresh interval ([3a74e15](https://github.com/fedexist/grafatui/commit/3a74e158b966d533b8d66921834624f8f0e27224))
- Dashboard refresh interval ([#50](https://github.com/fedexist/grafatui/pull/50)) ([312b890](https://github.com/fedexist/grafatui/commit/312b8908d528e9680f9c85ca6122edd34730d66c))
- Support instant target queries ([f886f84](https://github.com/fedexist/grafatui/commit/f886f845683cbf3585965d87d38c99d7a3d3fe26))

### 📚 Documentation

- Publish the user guide at [fedexist.github.io/grafatui](https://fedexist.github.io/grafatui/).
- Align roadmap with Grafana parity priorities ([b0e128d](https://github.com/fedexist/grafatui/commit/b0e128de94fa91da94c6a9b6c723ba44d21e8e41))
- Add instant query dashboard example ([2b38ded](https://github.com/fedexist/grafatui/commit/2b38ded40532fb13de39c4d09e9f6705d6cf8271))
- Add mdBook user guide ([a0feba1](https://github.com/fedexist/grafatui/commit/a0feba1e6cbe97e0d19aaab8183b3209d28e5b09))


## [0.1.7] - 2026-05-13

### ⛰️  Features

- Update recording engine with timing metadata and expand panel type rendering support ([4f6fafb](https://github.com/fedexist/grafatui/commit/4f6fafb08ce94fc35d17d668f9f412588ce1d2e6))
- Enrich recording manifest metadata ([e3e26d5](https://github.com/fedexist/grafatui/commit/e3e26d5849e8b9c0a2aafd2cb62e2e26421c03bd))

### 🐛 Bug Fixes

- Finalize recording on quit ([2c8c221](https://github.com/fedexist/grafatui/commit/2c8c221c32d5a4e0db701a5315ff6f452efda506))
- Validate recording frame limit ([203586d](https://github.com/fedexist/grafatui/commit/203586d921af6a963e03d7980d75e51bc7249373))

### 📚 Documentation

- Expand export recording guidance ([7f31e30](https://github.com/fedexist/grafatui/commit/7f31e305cbb9bf0c4126ac0b1c14d132c86c33a4))

### 🚜 Refactor

- Reorganize project structure into modules and implement event-driven app architecture ([e0a110f](https://github.com/fedexist/grafatui/commit/e0a110f3c50962dddd64d71fa7612cd2446384de))
- Modularize graph panel into sub-components for labels, thresholds, autogrid, bounds, and overlays ([0e13dde](https://github.com/fedexist/grafatui/commit/0e13ddeb831d417c948ec05c766ff80f0c39f43b))
- Extract key handling logic into a dedicated input module ([10d2c65](https://github.com/fedexist/grafatui/commit/10d2c65c2faa6d7749917b640aca7234ed113cce))
- Restrict visibility of internal state structures to crate-level ([9abadc6](https://github.com/fedexist/grafatui/commit/9abadc61a98b43fd221c8d6470b8dd3c7ab19d51))
- Route export shortcuts through input actions ([43c1c85](https://github.com/fedexist/grafatui/commit/43c1c8562b64e2cc4f7a682347ee3ddc7a5c44f7))

### 🎨 Styling

- Apply rustfmt import ordering ([90d96c5](https://github.com/fedexist/grafatui/commit/90d96c5c623ca736e7faf94cbc709542a2c3d334))

### ⚙️ Miscellaneous Tasks

- **release:** Run release automation from main ([81126e1](https://github.com/fedexist/grafatui/commit/81126e1ff02285993be5ee7ece7451df83675c94))


## [0.1.6] - 2026-04-30

### ⛰️  Features

- Implement panel navigation via PgUp/PgDn and refactor selection logic into dedicated methods ([e5c8546](https://github.com/fedexist/grafatui/commit/e5c854687d7afdcff52cdd1c32e766ed5d3900da))
- Implement adaptive time axis formatting based on range duration ([c189d77](https://github.com/fedexist/grafatui/commit/c189d77d8ab070354b750de6ace8eee9821d21bb))
- Replace label rendering logic with improved centered placement calculation and add tests ([41aa507](https://github.com/fedexist/grafatui/commit/41aa507b7b492cf24a91b09bf2b0225e2bc85933))
- Add configurable autogrid color and improve grid label rendering logic ([6b6987c](https://github.com/fedexist/grafatui/commit/6b6987c1e5404c07bf1ddd3a8d23c4094db371b0))
- Implement per-panel autogrid toggle and global runtime override ([6d517d7](https://github.com/fedexist/grafatui/commit/6d517d760a15a5934bd821d189cb9b211ba125a6))

### 🐛 Bug Fixes

- Pin UI time bounds to last refresh and optimize render loop for event-driven updates ([d248341](https://github.com/fedexist/grafatui/commit/d2483415eacd555750875c52973da800157affb1))

### 🚜 Refactor

- Update release trigger to created and improve tag resolution logic for asset and homebrew updates ([2982af8](https://github.com/fedexist/grafatui/commit/2982af8268fc88a4d4f5020ea92cc31609f13fdd))

### 📚 Documentation

- Update roadmap to v0.1.5 and promote threshold and field bounds features to completed status ([b2e6395](https://github.com/fedexist/grafatui/commit/b2e639564eea3cd936d6d404809d6cc3eefff70e))

### ⚙️ Miscellaneous Tasks

- Trigger release workflow on published event and unify tag resolution using RELEASE_TAG env variable ([b057097](https://github.com/fedexist/grafatui/commit/b05709736eac31a0194da0797956d6a525de09bd))
- Update release-assets.yml ([04d5cad](https://github.com/fedexist/grafatui/commit/04d5caddf51fab87d5703f2e6860bc82cc8556d6))


## [0.1.5] - 2026-04-24

### ⛰️  Features

- Implement dynamic panel coloring based on configurable thresholds and axis bounds ([8b8893f](https://github.com/fedexist/grafatui/commit/8b8893fa677e322b5e307b49b28740aea4288269))
- Implement support for dynamic panel thresholds and add a demo dashboard for verification ([f2b5bbe](https://github.com/fedexist/grafatui/commit/f2b5bbec09e951434c61029b44c5573035f02b0b))
- Add configurable threshold markers and styles with support for custom rendering modes ([44c5dbb](https://github.com/fedexist/grafatui/commit/44c5dbb397fb9d67b53a115ff53dffb7e1e97316))
- Add quadrant, sextant, and octant threshold markers with custom line rendering support ([9ed1ba6](https://github.com/fedexist/grafatui/commit/9ed1ba67f50e02e96e71ed9d9138876bf03c7593))
- Update default threshold marker to dashed-line ([6bd62b8](https://github.com/fedexist/grafatui/commit/6bd62b883c3874e1df853fc27f47f05e8d3fb0d8))
- Fix dashed-line threshold rendering not replacing the graph marker ([1d0d835](https://github.com/fedexist/grafatui/commit/1d0d8351ef92e65676c9f8e97c2b53196ba000ff))

### 🐛 Bug Fixes

- Fix threshold overlay rendering for graph charts, now having precedence over thresholds ([ebb805c](https://github.com/fedexist/grafatui/commit/ebb805cee3b801bcdf9110c6c96399b6d329c9ce))

### 📚 Documentation

- Add Grafana compatibility matrix and project roadmap to documentation ([3f068e0](https://github.com/fedexist/grafatui/commit/3f068e06b24e38682d4784222cbdf2f5695f74e7))
- Add threshold-marker configuration option to README documentation ([283dc73](https://github.com/fedexist/grafatui/commit/283dc73e63147cee3c5095dec6a1c4ae21cb424d))
- Add instructions for triggering a manual release to the release process documentation ([b4ddc43](https://github.com/fedexist/grafatui/commit/b4ddc432259b848f17998b55aebc9c07d89ef807))

### ⚙️ Miscellaneous Tasks

- Upgrade ratatui to 0.30.0 and update run_app trait bounds ([558b2b4](https://github.com/fedexist/grafatui/commit/558b2b45aea417a5c59b001ca2ecd705836baa0e))


## [0.1.4] - 2026-01-25

### ⛰️  Features

- Add asciicast demo and update the demo quick start command. ([b7f86e6](https://github.com/fedexist/grafatui/commit/b7f86e646c642f918af8804b1358197cbacfb8da))
- Add shell completion generation for various shells and update documentation. ([20acb0a](https://github.com/fedexist/grafatui/commit/20acb0a07b05cf0684841d46e5215776e39ef0c6))
- Add man page generation via a new CLI command and introduce Homebrew support with a new formula and automated workflow. ([26ee256](https://github.com/fedexist/grafatui/commit/26ee25645bdefcc54892a6a6a0ec18b73c861975))
- Implement automatic `.deb` and `.rpm` package generation and release for Linux x86_64. ([aaa7a44](https://github.com/fedexist/grafatui/commit/aaa7a44e958e01a2dda3f3b85fa0d73bf3452ef6))

### 🐛 Bug Fixes

- Use rustls-only TLS to remove OpenSSL system dependency ([a652ff0](https://github.com/fedexist/grafatui/commit/a652ff065c9a00c5d66114f0c965f39b4c17a6bc))


## [0.1.3] - 2026-01-06

### 🐛 Bug Fixes

- Propagate error when loading configured dashboard fails ([1f9a6e5](https://github.com/fedexist/grafatui/commit/1f9a6e5e09640f9a548427cb9be01d9dc8413d78))
- Improve error handling for config loading and HTTP client ([090ec71](https://github.com/fedexist/grafatui/commit/090ec714aaf0fa925dd9daf4e5df1bff894fa9b6))


## [0.1.2] - 2026-01-06

### 🐛 Bug Fixes

- Add path expansion for `~` in config and Grafana dashboard paths, and improve robustness for empty panels. ([394ba79](https://github.com/fedexist/grafatui/commit/394ba79ce36353157b719b2484169119099e7bbb))


## [0.1.1] - 2025-12-05

### ⛰️  Features

- Establish project governance with contribution guidelines, issue templates, and Apache-2.0 license. ([f18b74c](https://github.com/fedexist/grafatui/commit/f18b74c2dc910f1766bcd4c335e4fcf9131016f6))
- Add Apache 2.0 license header to all source files. ([55edce5](https://github.com/fedexist/grafatui/commit/55edce5bf6cbc0e08c632d8e2c149c0329414bbd))
- Add support for config file variables with minor app logic refactoring. ([d37efe5](https://github.com/fedexist/grafatui/commit/d37efe5f8ee202a2feedd1cfca0be91dca92ca0c))
- Add `step` configuration option, update demo `grafatui.toml` with new settings and `vars` ([7c3651c](https://github.com/fedexist/grafatui/commit/7c3651c9bb06ea28f222cb32bcbfbf3b3bcaecac))

### 📚 Documentation

- Update README with a quick start guide, detailed features, comprehensive installation methods, usage options, and a Grafana comparison. ([7057933](https://github.com/fedexist/grafatui/commit/7057933db24d2c3df223fb69dc417d10cfa2126f))

### ⚙️ Miscellaneous Tasks

- Add cross-platform binary release workflow ([3b226e3](https://github.com/fedexist/grafatui/commit/3b226e3318ea4645da1c53583538c31dac9c18f8))
- Cleanup CI workflows (remove debug steps and redundant files) ([250383c](https://github.com/fedexist/grafatui/commit/250383cd56049dec4d6b94e7f45b4dcf74e292c9))
- Update Cargo.toml metadata and Rust version to 1.85 ([085ce48](https://github.com/fedexist/grafatui/commit/085ce48dc094b62f8e8390f446de228dc70412b2))


## [0.1.0] - 2025-11-22

### ⛰️  Features

- Introduce concurrent panel data fetching, enhance Prometheus client with timeouts and URL building, and improve expression variable expansion. ([90c17a9](https://github.com/fedexist/grafatui/commit/90c17a95968aae05f578634dcf80c35225318132))
- Parse Grafana dashboard templating variables and legend formats, display skipped panel count, and update Prometheus port mapping. ([59303e4](https://github.com/fedexist/grafatui/commit/59303e46046aa95dfecf28657c37601551dd501f))
- Add latest series value to legend, page/home/end scrolling, and dynamic series coloring. ([d5d9deb](https://github.com/fedexist/grafatui/commit/d5d9deb6dad7e6c5249080211572df9acbafb820))
- Add CI/CD workflows and README, improve CLI argument parsing, and enhance code documentation. ([4658e99](https://github.com/fedexist/grafatui/commit/4658e9935de438cf0c8666c913bd580ef857b56f))
- Add Y-axis scaling modes, panel selection highlighting, and series visibility control with improved axis labels. ([3649504](https://github.com/fedexist/grafatui/commit/3649504bfd53d2df2ff8f44f1b6829c11a976f32))
- Implement client-side downsampling and Prometheus query caching with in-flight request deduplication. ([dd98571](https://github.com/fedexist/grafatui/commit/dd98571b87fe54d3004866afd248e6976cd1148c))
- Add configuration file loading and UI theming with multiple color schemes. ([bfd433f](https://github.com/fedexist/grafatui/commit/bfd433fee4d1c09b5f5f6438203178394fdbb3c9))
- Add Solarized Dark/Light, Gruvbox, Tokyo Night, and Catppuccin color themes and update README. ([ba18d6d](https://github.com/fedexist/grafatui/commit/ba18d6ddd7387aba2ce69913242f839a7d5a8d2b))
- Add hash-based color generation for series when the palette is insufficient, converting HSL to RGB. ([16164a3](https://github.com/fedexist/grafatui/commit/16164a3ee3ad1090dd7d0a7fd51b3fb56b6ae91a))
- Implement time panning and live mode with new keybindings, and refactor zoom controls. ([62cc92d](https://github.com/fedexist/grafatui/commit/62cc92d44d2e7a73dbbf91a9fc4738c2712f10ca))
- Add automated release process with release-plz, git-cliff, and GitHub Actions, including updated documentation. ([dc59df0](https://github.com/fedexist/grafatui/commit/dc59df0ca7aa28379a6accf2b373b8eda7ab2fd6))
- Configure detailed changelog generation with custom header, body, and commit parsers for release-plz. ([438d093](https://github.com/fedexist/grafatui/commit/438d09328ed6b5ede1ae88649cfacd7a3b7908c2))
- Add release-plz repo URL, enable GitHub releases, set PR branch prefix, and remove main branch restriction. ([70730fb](https://github.com/fedexist/grafatui/commit/70730fb58df5260bf7bd778fe47ec7d026341995))

### 🚜 Refactor

- Extract helper functions for Y-axis bounds calculation, panel data fetching, and Prometheus API requests. ([91bd5db](https://github.com/fedexist/grafatui/commit/91bd5dbcc8ae4d7e557877c4d8ba4e9d85ccca1e))
- Separate series name from value in `SeriesView` and dynamically format legend in UI ([23e78ad](https://github.com/fedexist/grafatui/commit/23e78ad3c7af520ff294a4d7def28b5e22e34b01))


<!-- generated by git-cliff -->
