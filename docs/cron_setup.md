# Weekly Cron Job Setup for Pendle V2 Pools

This guide explains how to set up an automated weekly cron job to fetch and insert new Pendle V2 SY pools into your ClickHouse database.

## Quick Setup

### 1. Automated Setup (Recommended)

```bash
# Run the setup script
./scripts/setup_cron.sh
```

This will:

- Set up a cron job to run every Sunday at 2:00 AM
- Handle existing cron jobs
- Verify script permissions

### 2. Manual Setup

If you prefer to set it up manually:

```bash
# Edit crontab
crontab -e

# Add this line (runs every Sunday at 2 AM):
0 2 * * 0 /path/to/your/brontes/scripts/pendle_pools_cron.sh
```

## Configuration

### Environment Variables

Ensure your `.env` file contains:

```bash
# Required
BRONTES_DB_PATH=/path/to/brontes/db
RETH_ENDPOINT=http://localhost  # OR RPC_URL=https://your.rpc.endpoint

# ClickHouse (optional, uses defaults if not set)
CLICKHOUSE_URL=localhost
CLICKHOUSE_PORT=9000
CLICKHOUSE_USER=default
CLICKHOUSE_PASSWORD=
```

## Cron Schedule Options

| Schedule                 | Cron Expression | Description                   |
| ------------------------ | --------------- | ----------------------------- |
| **Weekly** (recommended) | `0 2 * * 0`     | Every Sunday at 2:00 AM       |
| Daily                    | `0 2 * * *`     | Every day at 2:00 AM          |
| Bi-weekly                | `0 2 * * 0/2`   | Every other Sunday at 2:00 AM |
| Monthly                  | `0 2 1 * *`     | 1st day of month at 2:00 AM   |

## Features

### ✅ **Smart Duplicate Detection**

- Only inserts pools that don't already exist
- Uses `--skip-existing=true` by default

### 📝 **Comprehensive Logging**

- Timestamped logs in `logs/` directory
- Automatic log rotation (keeps 30 days)
- Both success and error logging

### 📧 **Error Notifications**

- Optional email alerts on failures
- Set `ADMIN_EMAIL` environment variable

### 🔒 **Error Handling**

- Validates environment variables
- Graceful failure handling
- Exit codes for monitoring

## Testing

### Test the cron script manually:

```bash
./scripts/pendle_pools_cron.sh
```

### Test with dry run:

```bash
# Modify the script temporarily to add --dry-run flag
./target/release/brontes db pendle-pools --skip-existing=true --dry-run
```

## Monitoring

### Check cron job status:

```bash
# View current cron jobs
crontab -l

# Check recent logs
ls -la logs/pendle_pools_*.log | tail -5

# View latest log
tail -f logs/pendle_pools_$(ls logs/pendle_pools_*.log | tail -1 | xargs basename)
```

### Log locations:

- **Success logs**: `logs/pendle_pools_YYYYMMDD_HHMMSS.log`
- **Cron output**: Usually in `/var/mail/username` or `/var/log/cron`

## Troubleshooting

### Common Issues

1. **Permission denied**

   ```bash
   chmod +x scripts/pendle_pools_cron.sh
   ```

2. **Environment variables not found**

   - Ensure `.env` file exists in project root
   - Check variable names and values

3. **Cron job not running**

   ```bash
   # Check if cron service is running
   sudo service cron status  # Ubuntu/Debian
   sudo systemctl status crond  # CentOS/RHEL
   ```

4. **Build failures**
   - Pre-build the binary: `cargo build --release --bin brontes --features="local-clickhouse,arbitrum"`
   - Comment out the build step in the cron script

### Debug Mode

Enable verbose logging by modifying the cron script:

```bash
# Add to the top of pendle_pools_cron.sh
set -x  # Enable debug mode
```

## Security Considerations

- Store sensitive credentials in `.env` file with restricted permissions:
  ```bash
  chmod 600 .env
  ```
- Consider using environment-specific configuration files
- Regularly rotate API keys and database credentials

## Production Recommendations

1. **Pre-build binaries** in CI/CD and comment out the build step
2. **Monitor disk space** for log files
3. **Set up alerting** for job failures
4. **Use dedicated database user** with minimal required permissions
5. **Regular backups** of the database before bulk insertions
