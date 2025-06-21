#!/bin/bash

# TWINS Auto-Deploy Script
# Handles automated deployment of TWINS Explorer and Node

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$SCRIPT_DIR/config.json"
LOG_FILE="/var/log/twins-deploy.log"

# Colors for output
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[1;33m"
BLUE="\033[0;34m"
NC="\033[0m" # No Color

# Logging function
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date "+%Y-%m-%d %H:%M:%S")
    echo -e "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

log_info() { log "INFO" "$@"; }
log_warn() { log "WARN" "$@"; }
log_error() { log "ERROR" "$@"; }
log_success() { log "SUCCESS" "$@"; }

# Parse JSON config (simple extraction)
get_config() {
    local key="$1"
    grep -o "\"$key\"[^,}]*" "$CONFIG_FILE" | sed "s/\"$key\":[[:space:]]*\"//" | sed "s/\"$//"
}

# Check if a service needs to be deployed
needs_deployment() {
    local repo_path=$(get_config "path")
    cd "$repo_path"
    
    # Fetch latest changes
    git fetch origin main >/dev/null 2>&1
    
    # Check if local is behind remote
    local local_hash=$(git rev-parse HEAD)
    local remote_hash=$(git rev-parse origin/main)
    
    if [ "$local_hash" != "$remote_hash" ]; then
        return 0  # Needs deployment
    else
        return 1  # Up to date
    fi
}

# Create backup
create_backup() {
    local backup_path="/opt/twins-deploy/backups"
    local timestamp=$(date "+%Y%m%d_%H%M%S")
    local backup_dir="$backup_path/$timestamp"
    
    log_info "Creating backup at $backup_dir"
    mkdir -p "$backup_dir"
    
    # Backup current repository
    cp -r /opt/twins2-explorer "$backup_dir/twins2-explorer"
    
    # Backup service states
    systemctl is-active twins-frontend > "$backup_dir/twins-frontend.state" || true
    systemctl is-active twins-node > "$backup_dir/twins-node.state" || true
    
    echo "$backup_dir" > /tmp/last_backup_path
    log_success "Backup created successfully"
}

# Rollback deployment
rollback() {
    local backup_path=$(cat /tmp/last_backup_path 2>/dev/null || echo "")
    
    if [ -z "$backup_path" ] || [ ! -d "$backup_path" ]; then
        log_error "No backup found for rollback"
        return 1
    fi
    
    log_warn "Rolling back to backup: $backup_path"
    
    # Stop services
    systemctl stop twins-frontend twins-node || true
    
    # Restore repository
    rm -rf /opt/twins2-explorer
    cp -r "$backup_path/twins2-explorer" /opt/twins2-explorer
    
    # Restart services if they were running
    if grep -q "active" "$backup_path/twins-frontend.state" 2>/dev/null; then
        systemctl start twins-frontend
    fi
    if grep -q "active" "$backup_path/twins-node.state" 2>/dev/null; then
        systemctl start twins-node
    fi
    
    log_success "Rollback completed"
}

# Deploy updates
deploy() {
    local repo_path="/opt/twins2-explorer"
    
    log_info "${BLUE}�� Starting deployment process${NC}"
    
    # Create backup
    create_backup
    
    # Pull latest changes
    log_info "Pulling latest changes from GitHub"
    cd "$repo_path"
    
    local old_hash=$(git rev-parse HEAD)
    git pull origin main
    local new_hash=$(git rev-parse HEAD)
    
    if [ "$old_hash" = "$new_hash" ]; then
        log_info "No new changes found"
        return 0
    fi
    
    log_info "Updated from $old_hash to $new_hash"
    
    # Show what changed
    log_info "Changes in this deployment:"
    git log --oneline "$old_hash..$new_hash" | while read line; do
        log_info "  📝 $line"
    done
    
    # Determine what needs to be rebuilt
    local changed_files=$(git diff --name-only "$old_hash" "$new_hash")
    local needs_frontend=false
    local needs_node=false
    
    if echo "$changed_files" | grep -q "twins-explorer/"; then
        needs_frontend=true
        log_info "Frontend changes detected"
    fi
    
    if echo "$changed_files" | grep -q "TWINS-Core/twins_node_rust/"; then
        needs_node=true
        log_info "Node changes detected"
    fi
    
    # Deploy frontend if needed
    if [ "$needs_frontend" = true ]; then
        log_info "${YELLOW}📦 Building frontend...${NC}"
        
        cd "$repo_path/twins-explorer"
        if npm run build; then
            log_success "Frontend build completed"
            
            log_info "Restarting frontend service"
            if systemctl restart twins-frontend; then
                log_success "Frontend service restarted"
                
                # Health check
                sleep 5
                if curl -f http://localhost:3000/ >/dev/null 2>&1; then
                    log_success "Frontend health check passed"
                else
                    log_error "Frontend health check failed"
                    rollback
                    return 1
                fi
            else
                log_error "Failed to restart frontend service"
                rollback
                return 1
            fi
        else
            log_error "Frontend build failed"
            rollback
            return 1
        fi
    fi
    
    # Deploy node if needed
    if [ "$needs_node" = true ]; then
        log_info "${YELLOW}🔧 Building TWINS node...${NC}"
        
        cd "$repo_path/TWINS-Core/twins_node_rust"
        if $HOME/.cargo/bin/cargo build --release; then
            log_success "Node build completed"
            
            log_info "Restarting node service"
            if systemctl restart twins-node; then
                log_success "Node service restarted"
                
                # Health check
                sleep 10
                if curl -f http://localhost:3001/api/v1/status >/dev/null 2>&1; then
                    log_success "Node health check passed"
                else
                    log_error "Node health check failed"
                    rollback
                    return 1
                fi
            else
                log_error "Failed to restart node service"
                rollback
                return 1
            fi
        else
            log_error "Node build failed"
            rollback
            return 1
        fi
    fi
    
    # Clean up old backups (keep last 5)
    find /opt/twins-deploy/backups -maxdepth 1 -type d -name "20*" | sort -r | tail -n +6 | xargs rm -rf 2>/dev/null || true
    
    log_success "${GREEN}🎉 Deployment completed successfully!${NC}"
    
    # Send notification if webhook configured
    # notify_deployment_success "$old_hash" "$new_hash"
}

# Main execution
case "${1:-deploy}" in
    "deploy")
        if needs_deployment; then
            deploy
        else
            log_info "Repository is up to date, no deployment needed"
        fi
        ;;
    "force-deploy")
        deploy
        ;;
    "rollback")
        rollback
        ;;
    "check")
        if needs_deployment; then
            echo "UPDATE_AVAILABLE"
            exit 1
        else
            echo "UP_TO_DATE"
            exit 0
        fi
        ;;
    *)
        echo "Usage: $0 {deploy|force-deploy|rollback|check}"
        exit 1
        ;;
esac
