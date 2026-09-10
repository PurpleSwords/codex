#!/usr/bin/env bash
# Diagnose hosted-runner sandbox startup without touching developer configuration.
set -euo pipefail

: "${CARGO_TARGET_DIR:?}"
: "${RUNNER_TEMP:?}"
diagnostics="$RUNNER_TEMP/sandbox-diagnostics"
mkdir -p "$diagnostics/home" "$diagnostics/workspace"
export CODEX_HOME="$diagnostics/home"
ulimit -c 0

{
  uname -a
  getconf GNU_LIBC_VERSION
  sysctl kernel.unprivileged_userns_clone kernel.apparmor_restrict_unprivileged_userns || true
  command -v bwrap || true
  "$CARGO_TARGET_DIR/debug/bwrap" --version
  "$CARGO_TARGET_DIR/debug/codex" --version
} > "$diagnostics/environment.txt" 2>&1

result=0
cd "$diagnostics/workspace"
for policy in read-only workspace-write; do
  command=("$CARGO_TARGET_DIR/debug/codex" sandbox
    -c "sandbox_mode=\"$policy\""
    -- /bin/echo sandbox-preflight)
  if timeout 30s "${command[@]}" > "$diagnostics/$policy.stdout" 2> "$diagnostics/$policy.stderr"; then
    echo "$policy sandbox startup passed"
  else
    status=$?
    result=1
    echo "::error::$policy sandbox startup failed with exit $status"
    printf '%s\n' "$status" > "$diagnostics/$policy.exit-code"
    # This traced execution is diagnostic, not a retry that can clear failure.
    # Trace only process setup and filesystem calls, never request payloads.
    timeout 30s strace -f -s 256 -e trace=process,file,prctl,unshare,setns,seccomp \
      -o "$diagnostics/$policy.strace" "${command[@]}" \
      > "$diagnostics/$policy.traced.stdout" 2> "$diagnostics/$policy.traced.stderr" || true
  fi
done
exit "$result"
