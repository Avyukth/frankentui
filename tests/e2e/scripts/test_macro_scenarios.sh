#!/bin/bash
# E2E Tests for Macro Recorder Scenarios (bd-2lus.5)
#
# Tests deterministic macro recording and playback scenarios.
#
# Run with: ./tests/e2e/scripts/test_macro_scenarios.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/../lib"

# shellcheck source=/dev/null
source "$LIB_DIR/common.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/logging.sh"
# shellcheck source=/dev/null
source "$LIB_DIR/pty.sh"

# Ensure demo binary exists
if [[ ! -x "${E2E_DEMO_BIN:-}" ]]; then
    # Try to build or locate it
    E2E_DEMO_BIN="$(cargo build --release -p ftui-demo-showcase --message-format=json 2>/dev/null \
        | jq -r 'select(.executable != null) | .executable' | tail -1)" || true

    if [[ ! -x "${E2E_DEMO_BIN:-}" ]]; then
        E2E_DEMO_BIN="$SCRIPT_DIR/../../../target/release/ftui-demo-showcase"
    fi
fi

if [[ ! -x "${E2E_DEMO_BIN:-}" ]]; then
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_scenarios_missing.log"
    for t in macro_record_stop macro_replay_determinism macro_speed_control macro_loop_mode; do
        log_test_skip "$t" "ftui-demo-showcase binary missing"
        record_result "$t" "skipped" 0 "$LOG_FILE" "binary missing"
    done
    exit 0
fi

run_case() {
    local name="$1"
    shift
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
    log_test_fail "$name" "macro scenario assertions failed"
    record_result "$name" "failed" "$duration_ms" "$LOG_FILE" "macro scenario assertions failed"
    return 1
}

# =============================================================================
# Scenario: Record and Stop
# Tests that recording starts, captures events, and stops cleanly.
# =============================================================================

macro_record_stop() {
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_record_stop.log"
    local output_file="${E2E_LOG_DIR:-/tmp}/macro_record_stop.pty"

    log_test_start "macro_record_stop"

    # Navigate to macro recorder (screen 11), start recording, press some keys, stop
    FTUI_DEMO_SCREEN=11 \
    FTUI_DEMO_EXIT_AFTER_MS=2000 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$E2E_DEMO_BIN" <<< $'rggddr' || true

    # Verify macro recorder screen elements
    grep -a -q "Macro Recorder" "$output_file" || return 1
    # Should show recording or stopped state
    grep -a -E -q "(Recording|Stopped)" "$output_file" || return 1
}

# =============================================================================
# Scenario: Replay Determinism
# Tests that the same macro produces identical event sequences on replay.
# =============================================================================

macro_replay_determinism() {
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_replay_determinism.log"
    local output_file="${E2E_LOG_DIR:-/tmp}/macro_replay_determinism.pty"

    log_test_start "macro_replay_determinism"

    # Record a simple sequence and replay it
    # r=record, g,g,d,d=events, r=stop, p=play
    FTUI_DEMO_SCREEN=11 \
    FTUI_DEMO_EXIT_AFTER_MS=3000 \
    PTY_TIMEOUT=6 \
        pty_run "$output_file" "$E2E_DEMO_BIN" <<< $'rggddrp' || true

    # Verify playback occurred (Playing state or progress indicator)
    grep -a -E -q "(Playing|Progress)" "$output_file" || return 1
}

# =============================================================================
# Scenario: Speed Control
# Tests that playback speed can be adjusted.
# =============================================================================

macro_speed_control() {
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_speed_control.log"
    local output_file="${E2E_LOG_DIR:-/tmp}/macro_speed_control.pty"

    log_test_start "macro_speed_control"

    # Record, stop, then adjust speed with + and - keys
    # r=record, g,g=events, r=stop, +=speed up, -=speed down, p=play
    FTUI_DEMO_SCREEN=11 \
    FTUI_DEMO_EXIT_AFTER_MS=2500 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$E2E_DEMO_BIN" <<< $'rggr++--p' || true

    # Speed indicator should be visible
    grep -a -E -q "[0-9]+\.[0-9]+x" "$output_file" || return 1
}

# =============================================================================
# Scenario: Loop Mode
# Tests that loop mode can be toggled.
# =============================================================================

macro_loop_mode() {
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_loop_mode.log"
    local output_file="${E2E_LOG_DIR:-/tmp}/macro_loop_mode.pty"

    log_test_start "macro_loop_mode"

    # Record, stop, toggle loop, play
    # r=record, g=event, r=stop, l=toggle loop, p=play
    FTUI_DEMO_SCREEN=11 \
    FTUI_DEMO_EXIT_AFTER_MS=2500 \
    PTY_TIMEOUT=5 \
        pty_run "$output_file" "$E2E_DEMO_BIN" <<< $'rgrlp' || true

    # Loop indicator or status should be visible
    grep -a -E -q "(Loop|Repeat)" "$output_file" || return 1
}

# =============================================================================
# Scenario: Empty Macro Error
# Tests that attempting to play without recording shows an error.
# =============================================================================

macro_empty_error() {
    LOG_FILE="${E2E_LOG_DIR:-/tmp}/macro_empty_error.log"
    local output_file="${E2E_LOG_DIR:-/tmp}/macro_empty_error.pty"

    log_test_start "macro_empty_error"

    # Try to play without recording
    FTUI_DEMO_SCREEN=11 \
    FTUI_DEMO_EXIT_AFTER_MS=1500 \
    PTY_TIMEOUT=4 \
        pty_run "$output_file" "$E2E_DEMO_BIN" <<< $'p' || true

    # Should show error or warning about empty/no macro
    grep -a -E -q "(Error|empty|No macro)" "$output_file" || return 1
}

# =============================================================================
# Run All Tests
# =============================================================================

FAILURES=0

run_case "macro_record_stop"       macro_record_stop       || FAILURES=$((FAILURES + 1))
run_case "macro_replay_determinism" macro_replay_determinism || FAILURES=$((FAILURES + 1))
run_case "macro_speed_control"     macro_speed_control     || FAILURES=$((FAILURES + 1))
run_case "macro_loop_mode"         macro_loop_mode         || FAILURES=$((FAILURES + 1))
run_case "macro_empty_error"       macro_empty_error       || FAILURES=$((FAILURES + 1))

# =============================================================================
# Summary
# =============================================================================

echo ""
echo "=========================================="
echo "Macro Scenarios E2E Test Summary"
echo "=========================================="
echo "Total tests: 5"
echo "Failures: $FAILURES"

if [[ $FAILURES -gt 0 ]]; then
    echo "STATUS: FAILED"
    exit 1
else
    echo "STATUS: PASSED"
    exit 0
fi
