#!/usr/bin/env bash
set -euo pipefail

# ── CRON JOB SCRIPT FOR PENDLE V2 POOLS INSERTION ──────────────────────────
# This script is designed to be run by cron on a weekly basis
# It fetches and inserts new Pendle V2 SY pools into ClickHouse

# ── CONFIG ──────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LOG_DIR="${PROJECT_ROOT}/logs"
LOG_FILE="${LOG_DIR}/pendle_pools_$(date +%Y%m%d_%H%M%S).log"

# Create logs directory if it doesn't exist
mkdir -p "$LOG_DIR"

# Function to log with timestamp
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

# Function to handle errors
handle_error() {
    local exit_code=$?
    log "ERROR: Script failed with exit code $exit_code"
    log "Check log file: $LOG_FILE"
    
    # Optional: Send email notification on error
    if command -v mail >/dev/null 2>&1 && [[ -n "${ADMIN_EMAIL:-}" ]]; then
        echo "Pendle pools cron job failed. Check logs at $LOG_FILE" | \
            mail -s "Brontes Pendle Pools Job Failed" "$ADMIN_EMAIL"
    fi
    
    exit $exit_code
}

# Set up error handling
trap handle_error ERR

# ── MAIN EXECUTION ──────────────────────────────────────────────────────────
log "Starting Pendle V2 pools insertion cron job"

# Change to project directory
cd "$PROJECT_ROOT"

# Load environment variables if .env file exists
if [[ -f ".env" ]]; then
    log "Loading environment variables from .env"
    set -a  # automatically export all variables
    source .env
    set +a
else
    log "WARNING: No .env file found in $PROJECT_ROOT"
fi

# Verify required environment variables
required_vars=("BRONTES_DB_PATH")
for var in "${required_vars[@]}"; do
    if [[ -z "${!var:-}" ]]; then
        log "ERROR: Required environment variable $var is not set"
        exit 1
    fi
done

# Check if either RETH_ENDPOINT or RPC_URL is set
if [[ -z "${RETH_ENDPOINT:-}" ]] && [[ -z "${RPC_URL:-}" ]]; then
    log "ERROR: Either RETH_ENDPOINT or RPC_URL must be set"
    exit 1
fi

log "Environment check passed"

# Build the project (optional, comment out if pre-built)
log "Building brontes binary..."
cargo build --bin brontes --features="local-clickhouse,arbitrum" --release 2>&1 | tee -a "$LOG_FILE"

# Run the Pendle pools insertion
log "Executing Pendle pools insertion..."
./target/release/brontes db pendle-pools \
    --skip-existing=true \
    2>&1 | tee -a "$LOG_FILE"

log "Pendle V2 pools insertion completed successfully"

# Clean up old log files (keep last 30 days)
find "$LOG_DIR" -name "pendle_pools_*.log" -mtime +30 -delete 2>/dev/null || true

log "Cron job completed successfully" 