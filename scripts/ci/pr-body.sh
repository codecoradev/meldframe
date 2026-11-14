#!/bin/sh
# Usage: pr-body.sh <file> or pr-body.sh - (stdin). Exit 0 when the pull request description has
# the four required sections (What, Why, Changes, Testing) as level-2 headings, each with text
# under it; 1 otherwise.
set -eu
src="${1:--}"
if [ "$src" = "-" ]; then text=$(cat); else text=$(cat "$src"); fi
missing=""
for section in What Why Changes Testing; do
  # The section body is the text between its heading and the next level-2 heading (or the end).
  # Comment lines of the template (<!-- ... -->) do not count as content.
  body=$(printf '%s\n' "$text" | tr -d '\r' | awk -v s="$section" '
    /^##[ \t]+/ {
      h = $0; sub(/^##[ \t]+/, "", h); sub(/[ \t]+$/, "", h)
      on = (tolower(h) == tolower(s)); next
    }
    on { print }
  ' | sed -e 's/<!--.*-->//g' | tr -d '[:space:]')
  if ! printf '%s\n' "$text" | tr -d '\r' | grep -Eiq "^##[[:space:]]+$section[[:space:]]*\$"; then
    missing="$missing $section(heading)"
  elif [ -z "$body" ]; then
    missing="$missing $section(empty)"
  fi
done
if [ -n "$missing" ]; then
  echo "::error::The pull request description needs the sections ## What, ## Why, ## Changes and ## Testing, each with text. Problem:$missing"
  exit 1
fi
echo "ok: What, Why, Changes and Testing are present"
