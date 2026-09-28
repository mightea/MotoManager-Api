#!/usr/bin/env bash
# Install the Tesseract models the invoice OCR is tuned against into DIR
# (default: ./.tessdata); point TESSDATA_PREFIX at DIR afterwards.
#
# These are the standard `tessdata` models, not the `tessdata_fast` ones that
# Debian/Ubuntu's tesseract-ocr-* packages and Homebrew ship: on scanned
# Huggett invoices the fast models misread part numbers ("83 30 …" as
# "88 30 …") and drop decimal points, which no parser cleanup can undo.
#
# Used by the Dockerfile and CI; pinned to a release and verified by hash.
set -euo pipefail

DIR="${1:-.tessdata}"
BASE_URL="https://raw.githubusercontent.com/tesseract-ocr/tessdata/4.1.0"
MODELS=(
  "deu 896b3b4956503ab9daa10285db330881b2d74b70d889b79262cc534b9ec699a4"
  "eng daa0c97d651c19fba3b25e81317cd697e9908c8208090c94c3905381c23fc047"
)

if command -v sha256sum >/dev/null; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$DIR"
for model in "${MODELS[@]}"; do
  read -r lang hash <<<"$model"
  file="$DIR/$lang.traineddata"
  if [ -f "$file" ] && [ "$(sha256 "$file")" = "$hash" ]; then
    continue
  fi
  curl -fsSL -o "$file.tmp" "$BASE_URL/$lang.traineddata"
  actual="$(sha256 "$file.tmp")"
  if [ "$actual" != "$hash" ]; then
    rm -f "$file.tmp"
    echo "checksum mismatch for $lang.traineddata: $actual" >&2
    exit 1
  fi
  mv "$file.tmp" "$file"
done
echo "Tesseract models installed in $DIR"
