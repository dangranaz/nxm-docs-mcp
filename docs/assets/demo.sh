#!/usr/bin/env bash
# Demo driver for the asciinema recording. Types commands with a human-like
# cadence, then runs a real nxm-docs-mcp conversion over stdio: Markdown → PDF.
set -u

BIN="${NXM_BIN:-nxm-docs-mcp}"
SRC="/tmp/notes.md"
OUT="/tmp/notes.pdf"

# --- sample Markdown to convert -------------------------------------------
cat > "$SRC" <<'MD'
# Quarterly Report

A short summary of the quarter, written in **Markdown**.

## Highlights

- Shipped the new converter
- Zero system dependencies
- One Rust binary

## Next Steps

Move on to *native PDF* everywhere.
MD

# --- tiny typewriter helpers ----------------------------------------------
type_cmd() {  # echo a prompt + command, char by char
  printf '\033[1;32m$\033[0m '
  s="$1"
  i=0
  while [ "$i" -lt "${#s}" ]; do
    printf '%s' "${s:$i:1}"
    i=$((i + 1))
    sleep 0.02
  done
  printf '\n'
}
pause() { sleep "${1:-0.6}"; }

clear
pause 0.4
type_cmd "# nxm-docs-mcp — Markdown → native PDF, inside your AI agent"
pause 0.5

type_cmd "nxm-docs-mcp  # start the MCP server (stdio)"
pause 0.4
printf '\033[90m…speaks JSON-RPC over stdin/stdout\033[0m\n'
pause 0.6

type_cmd "# convert a Markdown file to a real PDF at $OUT"
pause 0.4

# Real call: initialize + tools/call md_to_pdf → write file.
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"md_to_pdf\",\"arguments\":{\"input_path\":\"$SRC\",\"output_path\":\"$OUT\"}}}" \
  | "$BIN" >/tmp/nxm_docs_demo_out.json 2>/dev/null

pause 0.5
printf '\033[1;34m==>\033[0m typeset with an embedded Typst engine — no browser, no LaTeX:\n'
pause 0.4
# Show the confirmation line from the tool result.
grep -o 'Wrote [0-9]* bytes of PDF to [^"]*' /tmp/nxm_docs_demo_out.json | sed 's/^/    /'
pause 0.8

type_cmd "file $OUT"
pause 0.3
file "$OUT" | sed 's/^/    /'
pause 0.6

type_cmd "head -c 8 $OUT | xxd"
pause 0.3
head -c 8 "$OUT" | xxd | sed 's/^/    /'
pause 1.2

printf '\n\033[1;32m✓\033[0m a real %%PDF document from Markdown — one Rust binary, zero system deps.\n'
pause 1.6
