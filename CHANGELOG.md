# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.1.6] - 2026-04-30

### ⛰️  Features

- Implement panel navigation via PgUp/PgDn and refactor selection logic into dedicated methods ([e5c8546](https://github.com/fedexist/grafatui/commit/e5c854687d7afdcff52cdd1c32e766ed5d3900da))

### 📚 Documentation

- Update roadmap to v0.1.5 and promote threshold and field bounds features to completed status ([b2e6395](https://github.com/fedexist/grafatui/commit/b2e639564eea3cd936d6d404809d6cc3eefff70e))

### ⚙️ Miscellaneous Tasks

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
