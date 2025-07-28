#!/usr/bin/env bash
set -euo pipefail

# ── CRON SETUP SCRIPT ──────────────────────────────────────────────────────
# This script helps set up a weekly cron job for Pendle V2 pools insertion

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CRON_SCRIPT="$SCRIPT_DIR/pendle_pools_cron.sh"

echo "🔧 Setting up weekly cron job for Pendle V2 pools insertion"
echo "Project root: $PROJECT_ROOT"
echo "Cron script: $CRON_SCRIPT"

# Check if cron script exists and is executable
if [[ ! -f "$CRON_SCRIPT" ]]; then
    echo "❌ Error: Cron script not found at $CRON_SCRIPT"
    exit 1
fi

if [[ ! -x "$CRON_SCRIPT" ]]; then
    echo "🔧 Making cron script executable..."
    chmod +x "$CRON_SCRIPT"
fi

# Create cron job entry
CRON_ENTRY="0 2 * * 0 $CRON_SCRIPT"  # Every Sunday at 2 AM

echo ""
echo "📅 Cron job configuration:"
echo "   Schedule: Every Sunday at 2:00 AM"
echo "   Script: $CRON_SCRIPT"
echo "   Entry: $CRON_ENTRY"
echo ""

# Check if cron job already exists
if crontab -l 2>/dev/null | grep -q "$CRON_SCRIPT"; then
    echo "⚠️  Cron job for Pendle pools already exists!"
    echo "Current cron jobs containing the script:"
    crontab -l | grep "$CRON_SCRIPT" || true
    echo ""
    read -p "Do you want to replace it? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "❌ Cancelled"
        exit 0
    fi
    
    # Remove existing entry
    crontab -l | grep -v "$CRON_SCRIPT" | crontab -
    echo "🗑️  Removed existing cron job"
fi

# Add new cron job
(crontab -l 2>/dev/null; echo "$CRON_ENTRY") | crontab -

echo "✅ Cron job added successfully!"
echo ""
echo "📋 Current cron jobs:"
crontab -l
echo ""
echo "📝 Notes:"
echo "   • Logs will be saved to: $PROJECT_ROOT/logs/"
echo "   • Make sure your .env file is properly configured"
echo "   • Set ADMIN_EMAIL in .env for error notifications"
echo "   • Test the script manually first: $CRON_SCRIPT"
echo ""
echo "🧪 To test the cron script manually:"
echo "   $CRON_SCRIPT"
echo ""