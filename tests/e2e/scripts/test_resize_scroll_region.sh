#!/bin/bash
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# E2E Tests: Resize + Scroll-Region (Inline Mode)
# ─────────────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

E2E_SUITE_SCRIPT="$SCRIPT_DIR/test_resize_scroll_region.sh"
export E2E_SUITE_SCRIPT
ONLY_CASE="${E2E_ONLY_CASE:-}"

ALL_CASES=(
    resize_scroll_region_inline
)

if [[ ! -x "${E2E_HARNESS_BIN:-}" ]]; then
    LOG_FILE="$E2E_LOG_DIR/resize_missing.log"
    for t in "${ALL_CASES[@]}"; do
        log_test_skip "$t" "ftui-harness binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
    done
    exit 0
fi

run_case() {
    local name="$1"
    shift
    if [[ -n "$ONLY_CASE" && "$ONLY_CASE" != "$name" ]]; then
        LOG_FILE="$E2E_LOG_DIR/${name}.log"
        log_test_skip "$name" "filtered (E2E_ONLY_CASE=$ONLY_CASE)"
        record_result "$name" "skipped" 0 "$LOG_FILE" "filtered"
        return 0
    fi
    local start_ms
    start_ms="$(date +%s%3N)"

    if "$@"; then
        local end_ms
        end_ms="$(date +%s%3N)"
        local duration_ms=$((end_ms - start_ms))
        log_test_pass "$name"
        record_result "$name" "passed" "$duration_ms" "$LOG_FILE"
        return 0
    fi

    local end_ms
    end_ms="$(date +%s%3N)"
    local duration_ms=$((end_ms - start_ms))
    log_test_fail "$name" "resize/scroll-region assertions failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "resize/scroll-region assertions failed"
    return 1
}

resize_scroll_region_inline() {
    LOG_FILE="$E2E_LOG_DIR/resize_scroll_region_inline.log"
    local output_file="$E2E_LOG_DIR/resize_scroll_region_inline.pty"

    log_test_start "resize_scroll_region_inline"

    TERM="xterm-256color" \
    PTY_COLS=80 \
    PTY_ROWS=24 \
    PTY_RESIZE_DELAY_MS=300 \
    PTY_RESIZE_COLS=100 \
    PTY_RESIZE_ROWS=30 \
    FTUI_HARNESS_SCREEN_MODE=inline \
    FTUI_HARNESS_UI_HEIGHT=6 \
    FTUI_HARNESS_EXIT_AFTER_MS=2000 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$E2E_HARNESS_BIN"

    # Resize event should be logged by the harness.
    if ! grep -a -q "Resize: 100x30" "$output_file"; then
        log_warn "Resize event line not found in PTY capture" || true
    fi

    # Inline mode should set and reset scroll regions around resize.
    grep -a -F -q $'\x1b[r' "$output_file" || return 1
    grep -a -F -q $'\x1b[1;24r' "$output_file" || return 1

    # Cursor save/restore should be used for inline rendering.
    grep -a -F -q $'\x1b7' "$output_file" || return 1
    grep -a -F -q $'\x1b8' "$output_file" || return 1
}

FAILURES=0
run_case "resize_scroll_region_inline" resize_scroll_region_inline || FAILURES=$((FAILURES + 1))
exit "$FAILURES"
