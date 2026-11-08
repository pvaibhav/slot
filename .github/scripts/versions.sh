#!/usr/bin/env bash
set -eu

BASEOS_REPO="${BASEOS_REPO:-pvaibhav/BaseOS}"
LIVE="${LIVE:-https://slot-cfw.fyi/versions.json}"

slot=$(gh release view --repo "$SLOT_REPO" --json tagName,assets | jq '
  .tagName as $t | ($t | ltrimstr("v")) as $v
  | first(.assets[] | select(.name == "slot-\($t)-h700.zip"))
  | {version: $v, tag: $t, zip: .name, size: .size, sha256: (.digest | ltrimstr("sha256:"))}')
echo "$slot" | jq -e '.sha256 | test("^[0-9a-f]{64}$")' > /dev/null

baseos_release() {
  gh release view --repo "$BASEOS_REPO" --json tagName,isPrerelease,assets | jq -e '
    select(.isPrerelease == false)
    | .tagName as $t | ($t | ltrimstr("v")) as $v
    | select($v | test("^[0-9]+\\.[0-9]+\\.[0-9]+$"))
    | [.assets[] | select(.name | test("^baseos-[a-z0-9]+-\($v | gsub("\\."; "\\."))\\.bosupd$"))] as $files
    | select(($files | length) > 0)
    | select(all($files[]; .state == "uploaded" and (.digest // "" | test("^sha256:[0-9a-f]{64}$"))))
    | {version: $v, tag: $t, files: ($files | map({
        key: (.name | capture("^baseos-(?<t>[a-z0-9]+)-").t),
        value: {name: .name, size: .size, sha256: (.digest | ltrimstr("sha256:"))}
      }) | from_entries)}'
}

live_baseos() {
  curl -fsS "$LIVE" | jq -e '.baseos | select(.version | test("^[0-9]+\\.[0-9]+\\.[0-9]+$"))'
}

if ! baseos=$(baseos_release 2>/dev/null); then
  echo "::warning::No usable stable BaseOS release in $BASEOS_REPO, keeping the live entry"
  baseos=$(live_baseos 2>/dev/null) || baseos=null
fi

jq -n --argjson slot "$slot" --argjson baseos "$baseos" \
  '{stable: $slot} + (if $baseos == null then {} else {baseos: $baseos} end)' > "$OUT"
cat "$OUT"
