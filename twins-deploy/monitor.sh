#!/bin/bash

# TWINS GitHub Monitor
# Continuously monitors GitHub for changes and triggers deployments

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_SCRIPT="$SCRIPT_DIR/deploy.sh"
LOG_FILE="/var/log/twins-deploy.log"
PID_FILE="/var/run/twins-monitor.pid"

# Colors
GREEN="\033[0;32m"
YELLOW="\033[1;33m"
BLUE="\033[0;34m"
NC="\033[0m"

# Logging
log() {
    local timestamp=$(date "+%Y-%m-%d %H:%M:%S")
    echo -e "[$timestamp] [MONITOR] $*" | tee -a "$LOG_FILE"
}

# Signal handlers for graceful shutdown
cleanup() {
    log "Monitor shutting down gracefully"
    rm -f "$PID_FILE"
    exit 0
}

trap cleanup SIGTERM SIGINT

# Write PID file
echo $$ > "$PID_FILE"

log "${BLUE}🔍 TWINS GitHub Monitor started (PID: $$)${NC}"
log "Monitoring repository: edwinux/twins2-explorer"
log "Check interval: 60 seconds"

# Main monitoring loop
while true; do
    # Check if deployment script exists
    if [ ! -x "$DEPLOY_SCRIPT" ]; then
        log "ERROR: Deploy script not found or not executable: $DEPLOY_SCRIPT"
        sleep 60
        continue
    fi
    
    # Check for updates
    log "${YELLOW}⏱️  Checking for repository updates...${NC}"
    
    if "$DEPLOY_SCRIPT" check >/dev/null 2>&1; then
        log "Repository is up to date"
    else
        log "${GREEN}📥 New changes detected - triggering deployment${NC}"
        
        # Run deployment
        if "$DEPLOY_SCRIPT" deploy; then
            log "${GREEN}✅ Deployment completed successfully${NC}"
        else
            log "❌ Deployment failed - check logs for details"
        fi
    fi
    
    # Wait for next check
    sleep 60
done
