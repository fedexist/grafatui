#!/bin/sh

set -eu

PROGRAM='grafatui'
REPOSITORY='fedexist/grafatui'
tmp_dir=''
staged_binary=''

say() {
  printf '%s\n' "$*"
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

download_with_curl() {
  request_url="$1"
  request_path="$2"
  response_headers="${tmp_dir}/curl-headers"
  curl_status=''
  curl_exit=0

  curl_status="$(
    curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
      --retry 3 --dump-header "${response_headers}" --output "${request_path}" \
      --write-out '%{http_code}' "${request_url}"
  )" || curl_exit=$?

  if [ "${curl_exit}" -eq 0 ]; then
    return 0
  fi
  if [ "${curl_status}" = '404' ]; then
    return 10
  fi
  return 11
}

download_with_wget() {
  request_url="$1"
  request_path="$2"
  wget_headers="${tmp_dir}/wget-headers"
  response_headers="${wget_headers}"
  wget_exit=0

  wget --https-only --quiet --server-response \
    --output-document="${request_path}" "${request_url}" 2>"${wget_headers}" || wget_exit=$?

  if [ "${wget_exit}" -eq 0 ]; then
    return 0
  fi

  wget_status="$(awk '/^[[:space:]]*HTTP\// { status = $2 } END { print status }' "${wget_headers}")"
  if [ "${wget_status}" = '404' ]; then
    return 10
  fi
  return 11
}

download() {
  case "${downloader}" in
    curl) download_with_curl "$1" "$2" ;;
    wget) download_with_wget "$1" "$2" ;;
  esac
}

release_tag_from_headers() {
  awk '
    BEGIN { marker = "/releases/download/" }
    tolower($1) == "location:" {
      url = $2
      sub(/\r$/, "", url)
      marker_position = index(url, marker)
      if (marker_position > 0) {
        remainder = substr(url, marker_position + length(marker))
        slash_position = index(remainder, "/")
        if (slash_position > 1) {
          tag = substr(remainder, 1, slash_position - 1)
        }
      }
    }
    END { print tag }
  ' "$1"
}

cleanup() {
  if [ -n "${staged_binary}" ] && [ -f "${staged_binary}" ]; then
    rm -f "${staged_binary}"
  fi

  case "${tmp_dir}" in
    "${TMPDIR:-/tmp}"/grafatui-install.*)
      if [ -d "${tmp_dir}" ] && [ ! -L "${tmp_dir}" ]; then
        rm -rf "${tmp_dir}"
      fi
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

case "$(uname -s)" in
  Linux)
    target_os='unknown-linux-gnu'
    ;;
  Darwin)
    target_os='apple-darwin'
    ;;
  *)
    fail "unsupported operating system: $(uname -s)"
    ;;
esac

case "$(uname -m)" in
  x86_64|amd64)
    target_arch='x86_64'
    ;;
  arm64|aarch64)
    target_arch='aarch64'
    ;;
  *)
    fail "unsupported architecture: $(uname -m)"
    ;;
esac

if [ -n "${GRAFATUI_INSTALL_DIR:-}" ]; then
  install_dir="${GRAFATUI_INSTALL_DIR}"
elif [ -n "${HOME:-}" ]; then
  install_dir="${HOME}/.local/bin"
else
  fail 'HOME is not set; set GRAFATUI_INSTALL_DIR to choose an installation directory'
fi

asset="${PROGRAM}-${target_arch}-${target_os}.tar.gz"
version="${GRAFATUI_VERSION:-latest}"

if [ "${version}" = 'latest' ]; then
  release_path='latest/download'
else
  case "${version}" in
    ''|*[!0-9A-Za-z._-]*)
      fail "invalid GRAFATUI_VERSION: ${version}"
      ;;
  esac
  case "${version}" in
    v*) release_tag="${version}" ;;
    *) release_tag="v${version}" ;;
  esac
  release_path="download/${release_tag}"
fi

base_url="https://github.com/${REPOSITORY}/releases/${release_path}"
archive_url="${base_url}/${asset}"
checksums_url="${base_url}/grafatui-checksums.txt"

if command -v curl >/dev/null 2>&1; then
  downloader='curl'
elif command -v wget >/dev/null 2>&1; then
  downloader='wget'
else
  fail 'curl or wget is required to download grafatui'
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/grafatui-install.XXXXXX")"
archive="${tmp_dir}/${asset}"
checksums="${tmp_dir}/grafatui-checksums.txt"

say "Downloading ${PROGRAM}..."
download_status=0
download "${archive_url}" "${archive}" || download_status=$?
[ "${download_status}" -eq 0 ] || fail "failed to download ${archive_url}"

if [ "${version}" = 'latest' ]; then
  resolved_version="$(release_tag_from_headers "${response_headers}")"
  [ -n "${resolved_version}" ] || fail 'failed to determine the latest release version'
else
  resolved_version="${release_tag}"
fi

download_status=0
download "${checksums_url}" "${checksums}" || download_status=$?
case "${download_status}" in
  0)
    ;;
  10)
    fail "checksum manifest is required for ${resolved_version}"
    ;;
  *)
    fail "failed to download ${checksums_url}"
    ;;
esac

expected_checksum="$(awk -v asset="${asset}" '$2 == asset || $2 == "*" asset { print $1; exit }' "${checksums}")"
[ -n "${expected_checksum}" ] || fail "checksum manifest has no entry for ${asset}"

if command -v sha256sum >/dev/null 2>&1; then
  actual_checksum="$(sha256sum "${archive}" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
  actual_checksum="$(shasum -a 256 "${archive}" | awk '{ print $1 }')"
else
  fail 'sha256sum or shasum is required to verify the download'
fi

[ "${actual_checksum}" = "${expected_checksum}" ] || fail "checksum verification failed for ${asset}"

tar -xzf "${archive}" -C "${tmp_dir}" "${PROGRAM}"
[ -f "${tmp_dir}/${PROGRAM}" ] || fail "downloaded archive does not contain ${PROGRAM}"

mkdir -p "${install_dir}"
staged_binary="$(mktemp "${install_dir}/.${PROGRAM}.tmp.XXXXXX")"
cp "${tmp_dir}/${PROGRAM}" "${staged_binary}"
chmod 755 "${staged_binary}"
mv -f "${staged_binary}" "${install_dir}/${PROGRAM}"
staged_binary=''

say "Installed ${PROGRAM} to ${install_dir}/${PROGRAM}"
