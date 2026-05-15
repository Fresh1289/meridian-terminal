#!/usr/bin/env bash
# Tail a role's live session JSONL for real-time turn-by-turn visibility.
# Useful while Manager dispatches into a Builder/Lani — open a side pane and
# run this to watch the dispatched session work in real time.
#
# Usage: script/meridian-watch.sh <role>
#   role: builder-1 | builder-2 | builder-3 | laniakea (or any registered role)
#
# Reads ~/.meridian/agents/<role>/{session-id,cwd} to locate the JSONL,
# then `tail -F` it with jq filtering to show user prompts + assistant
# text + tool calls (raw JSONL is too noisy).
#
# Press Ctrl+C to detach. Does not stop the session itself.

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <role>" >&2
    echo "  role: builder-1 | builder-2 | builder-3 | laniakea (etc.)" >&2
    exit 2
fi

role="$1"
role_dir="${HOME}/.meridian/agents/${role}"
session_file="${role_dir}/session-id"
cwd_file="${role_dir}/cwd"

if [[ ! -f "$session_file" || ! -f "$cwd_file" ]]; then
    echo "error: ${role} not registered (need session-id + cwd in ${role_dir})" >&2
    echo "hint: open ${role}'s claude session so the SessionStart hook registers it" >&2
    exit 1
fi

session_id="$(<"$session_file")"
project_cwd="$(<"$cwd_file")"

# Claude Code URL-encodes project paths by replacing / with -
encoded="$(echo "$project_cwd" | sed 's|/|-|g')"
jsonl_path="${HOME}/.claude/projects/${encoded}/${session_id}.jsonl"

if [[ ! -f "$jsonl_path" ]]; then
    echo "error: session JSONL not found at $jsonl_path" >&2
    echo "hint: session may have been reset; rerun the SessionStart hook" >&2
    exit 1
fi

echo "Watching ${role}"
echo "  session: ${session_id}"
echo "  cwd:     ${project_cwd}"
echo "  jsonl:   ${jsonl_path}"
echo "  -- Ctrl+C to detach (does NOT stop the session) --"
echo

# Filter for the high-signal events: user prompts, assistant text replies,
# and tool calls. Each line of jq output = one event.
tail -F "$jsonl_path" 2>/dev/null | jq -r --unbuffered '
  if .type == "user" then
    if (.message.content | type) == "string" then
      "🟦 USER: " + (.message.content | .[0:200])
    else
      "🟦 USER: " + (.message.content[0].text // (.message.content | tostring)[0:200])
    end
  elif .type == "assistant" then
    (.message.content // []) | map(
      if .type == "text" then
        "🟩 ASSISTANT: " + (.text[0:300])
      elif .type == "tool_use" then
        "🛠  TOOL: " + .name + " " + ((.input | tostring)[0:150])
      else empty
      end
    ) | .[]
  elif .type == "tool_result" then
    "✅ TOOL_RESULT" + (if .is_error == true then " (ERROR)" else "" end)
  else empty
  end
' 2>/dev/null
