#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALLER="${REPO_ROOT}/install.sh"
FIXTURE_BIN="${REPO_ROOT}/tests/fixtures/install"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/grafatui-install-test.XXXXXX")"

cleanup() {
  case "${TEST_ROOT}" in
    "${TMPDIR:-/tmp}"/grafatui-install-test.*)
      if [[ -d "${TEST_ROOT}" && ! -L "${TEST_ROOT}" ]]; then
        rm -rf -- "${TEST_ROOT}"
      fi
      ;;
  esac
}
trap cleanup EXIT

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  [[ "${haystack}" == *"${needle}"* ]] || fail "expected output to contain: ${needle}"
}

assert_not_contains() {
  local haystack="$1"
  local needle="$2"
  [[ "${haystack}" != *"${needle}"* ]] || fail "expected output not to contain: ${needle}"
}

assert_file_contains() {
  local path="$1"
  local expected="$2"
  [[ -f "${path}" ]] || fail "expected file to exist: ${path}"
  [[ "$(<"${path}")" == "${expected}" ]] || fail "unexpected contents in: ${path}"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

prepare_case() {
  local case_name="$1"
  local asset_name="$2"

  CASE_DIR="${TEST_ROOT}/${case_name}"
  HOME_DIR="${CASE_DIR}/home"
  INSTALL_DIR="${CASE_DIR}/bin"
  ARCHIVE="${CASE_DIR}/${asset_name}"
  CHECKSUMS="${CASE_DIR}/grafatui-checksums.txt"
  DOWNLOAD_LOG="${CASE_DIR}/downloads.log"

  mkdir -p "${CASE_DIR}/archive" "${HOME_DIR}"
  printf 'fixture grafatui binary\n' > "${CASE_DIR}/archive/grafatui"
  chmod +x "${CASE_DIR}/archive/grafatui"
  tar -czf "${ARCHIVE}" -C "${CASE_DIR}/archive" grafatui
  printf '%s  %s\n' "$(sha256 "${ARCHIVE}")" "${asset_name}" > "${CHECKSUMS}"
  : > "${DOWNLOAD_LOG}"
}

prepare_wget_only_path() {
  WGET_ONLY_BIN="${CASE_DIR}/wget-only-bin"
  mkdir -p "${WGET_ONLY_BIN}"
  ln -s "${FIXTURE_BIN}/uname" "${WGET_ONLY_BIN}/uname"
  ln -s "${FIXTURE_BIN}/wget" "${WGET_ONLY_BIN}/wget"

  local tool
  local tool_path
  for tool in awk chmod cp gzip mkdir mktemp mv rm tar; do
    tool_path="$(command -v "${tool}")"
    ln -s "${tool_path}" "${WGET_ONLY_BIN}/${tool}"
  done

  if command -v sha256sum >/dev/null 2>&1; then
    ln -s "$(command -v sha256sum)" "${WGET_ONLY_BIN}/sha256sum"
  else
    ln -s "$(command -v shasum)" "${WGET_ONLY_BIN}/shasum"
  fi

  [[ -x "${WGET_ONLY_BIN}/gzip" ]] || fail 'wget-only PATH is missing the gzip dependency used by GNU tar'
}

test_installs_latest_linux_x86_64_release() {
  local asset_name='grafatui-x86_64-unknown-linux-gnu.tar.gz'
  local output

  prepare_case 'latest-linux-x86_64' "${asset_name}"

  if ! output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    TEST_UNAME_S='Linux' \
    TEST_UNAME_M='x86_64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='present' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    printf '%s\n' "${output}" >&2
    fail 'latest Linux x86_64 installation failed'
  fi

  assert_file_contains "${INSTALL_DIR}/grafatui" 'fixture grafatui binary'
  [[ -x "${INSTALL_DIR}/grafatui" ]] || fail 'installed binary is not executable'
  assert_contains "$(<"${DOWNLOAD_LOG}")" \
    'https://github.com/fedexist/grafatui/releases/latest/download/grafatui-x86_64-unknown-linux-gnu.tar.gz'
  assert_contains "${output}" "Installed grafatui to ${INSTALL_DIR}/grafatui"
}

test_installs_pinned_macos_arm64_release() {
  local asset_name='grafatui-aarch64-apple-darwin.tar.gz'
  local output

  prepare_case 'pinned-macos-arm64' "${asset_name}"

  if ! output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    GRAFATUI_VERSION='0.2.0' \
    TEST_UNAME_S='Darwin' \
    TEST_UNAME_M='arm64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='present' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    printf '%s\n' "${output}" >&2
    fail 'pinned macOS arm64 installation failed'
  fi

  assert_file_contains "${INSTALL_DIR}/grafatui" 'fixture grafatui binary'
  assert_contains "$(<"${DOWNLOAD_LOG}")" \
    'https://github.com/fedexist/grafatui/releases/download/v0.2.0/grafatui-aarch64-apple-darwin.tar.gz'
  assert_contains "$(<"${DOWNLOAD_LOG}")" \
    'https://github.com/fedexist/grafatui/releases/download/v0.2.0/grafatui-checksums.txt'
}

test_rejects_a_legacy_release_without_checksums() {
  local asset_name='grafatui-aarch64-unknown-linux-gnu.tar.gz'
  local output

  prepare_case 'legacy-without-checksums' "${asset_name}"

  if output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    TEST_UNAME_S='Linux' \
    TEST_UNAME_M='aarch64' \
    TEST_RESOLVED_TAG='v0.1.11' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='missing' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    fail 'legacy release unexpectedly installed without checksums'
  fi

  [[ ! -e "${INSTALL_DIR}/grafatui" ]] || fail 'unverified legacy binary was installed'
  assert_contains "${output}" 'checksum manifest is required for v0.1.11'
}

test_rejects_a_future_release_without_checksums() {
  local asset_name='grafatui-aarch64-unknown-linux-gnu.tar.gz'
  local output

  prepare_case 'future-without-checksums' "${asset_name}"

  if output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    TEST_UNAME_S='Linux' \
    TEST_UNAME_M='aarch64' \
    TEST_RESOLVED_TAG='v0.1.12' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='missing' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    fail 'future release unexpectedly installed without checksums'
  fi

  [[ ! -e "${INSTALL_DIR}/grafatui" ]] || fail 'unverified future binary was installed'
  assert_contains "${output}" 'checksum manifest is required for v0.1.12'
}

test_uses_wget_and_the_default_install_directory() {
  local asset_name='grafatui-x86_64-apple-darwin.tar.gz'
  local output

  prepare_case 'wget-default-directory' "${asset_name}"
  prepare_wget_only_path

  if ! output="$(
    PATH="${WGET_ONLY_BIN}" \
    HOME="${HOME_DIR}" \
    TEST_UNAME_S='Darwin' \
    TEST_UNAME_M='x86_64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='present' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    printf '%s\n' "${output}" >&2
    fail 'wget installation failed'
  fi

  assert_file_contains "${HOME_DIR}/.local/bin/grafatui" 'fixture grafatui binary'
  assert_contains "$(<"${DOWNLOAD_LOG}")" \
    'https://github.com/fedexist/grafatui/releases/latest/download/grafatui-x86_64-apple-darwin.tar.gz'
}

test_wget_rejects_a_pinned_future_release_without_checksums() {
  local asset_name='grafatui-x86_64-apple-darwin.tar.gz'
  local output

  prepare_case 'wget-future-without-checksums' "${asset_name}"
  prepare_wget_only_path

  if output="$(
    PATH="${WGET_ONLY_BIN}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_VERSION='v0.1.12' \
    TEST_UNAME_S='Darwin' \
    TEST_UNAME_M='x86_64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='missing' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    fail 'wget unexpectedly installed a future release without checksums'
  fi

  [[ ! -e "${HOME_DIR}/.local/bin/grafatui" ]] || fail 'wget installed an unverified future binary'
  assert_contains "${output}" 'checksum manifest is required for v0.1.12'
}

test_rejects_a_checksum_mismatch() {
  local asset_name='grafatui-x86_64-unknown-linux-gnu.tar.gz'
  local output

  prepare_case 'checksum-mismatch' "${asset_name}"
  printf '%064d  %s\n' 0 "${asset_name}" > "${CHECKSUMS}"

  if output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    TEST_UNAME_S='Linux' \
    TEST_UNAME_M='x86_64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='present' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    fail 'installation unexpectedly succeeded with a bad checksum'
  fi

  [[ ! -e "${INSTALL_DIR}/grafatui" ]] || fail 'bad-checksum binary was installed'
  assert_contains "${output}" "checksum verification failed for ${asset_name}"
}

test_rejects_an_unsupported_operating_system() {
  local asset_name='grafatui-x86_64-unknown-linux-gnu.tar.gz'
  local output

  prepare_case 'unsupported-operating-system' "${asset_name}"

  if output="$(
    PATH="${FIXTURE_BIN}:${PATH}" \
    HOME="${HOME_DIR}" \
    GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
    TEST_UNAME_S='FreeBSD' \
    TEST_UNAME_M='x86_64' \
    TEST_ARCHIVE="${ARCHIVE}" \
    TEST_CHECKSUMS="${CHECKSUMS}" \
    TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
    TEST_CHECKSUM_MODE='present' \
      /bin/sh "${INSTALLER}" 2>&1
  )"; then
    fail 'installation unexpectedly succeeded on an unsupported operating system'
  fi

  assert_contains "${output}" 'unsupported operating system: FreeBSD'
}

test_termination_signal_stops_before_the_next_download() {
  local asset_name='grafatui-x86_64-unknown-linux-gnu.tar.gz'
  local output_file
  local ready_file
  local installer_pid
  local installer_status
  local attempt

  prepare_case 'termination-signal' "${asset_name}"
  output_file="${CASE_DIR}/output.log"
  ready_file="${CASE_DIR}/archive-ready"

  PATH="${FIXTURE_BIN}:${PATH}" \
  HOME="${HOME_DIR}" \
  GRAFATUI_INSTALL_DIR="${INSTALL_DIR}" \
  TEST_UNAME_S='Linux' \
  TEST_UNAME_M='x86_64' \
  TEST_ARCHIVE="${ARCHIVE}" \
  TEST_CHECKSUMS="${CHECKSUMS}" \
  TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
  TEST_CHECKSUM_MODE='present' \
  TEST_CURL_READY_FILE="${ready_file}" \
  TEST_CURL_DELAY_SECONDS='0.5' \
    /bin/sh "${INSTALLER}" > "${output_file}" 2>&1 &
  installer_pid=$!

  for attempt in {1..100}; do
    [[ ! -e "${ready_file}" ]] || break
    sleep 0.01
  done
  [[ -e "${ready_file}" ]] || fail 'timed out waiting for installer download'

  kill -TERM "${installer_pid}"
  installer_status=0
  wait "${installer_pid}" || installer_status=$?

  [[ "${installer_status}" -eq 143 ]] || fail "terminated installer exited with ${installer_status}, expected 143"
  assert_not_contains "$(<"${DOWNLOAD_LOG}")" 'grafatui-checksums.txt'
}

test_installs_latest_linux_x86_64_release
test_installs_pinned_macos_arm64_release
test_rejects_a_legacy_release_without_checksums
test_rejects_a_future_release_without_checksums
test_uses_wget_and_the_default_install_directory
test_wget_rejects_a_pinned_future_release_without_checksums
test_rejects_a_checksum_mismatch
test_rejects_an_unsupported_operating_system
test_termination_signal_stops_before_the_next_download
printf 'PASS: installer behavior tests\n'
