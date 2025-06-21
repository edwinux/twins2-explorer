'use client';

import { useState, useEffect } from 'react';

interface BlockchainStatus {
  status: string;
  version: string;
  protocol_version: number;
  block_height: number;
  header_height: number;
  estimated_network_height: number;
  sync_progress: number;
  peer_count: number;
  last_block_time?: number;
  network: string;
}

interface TorrentSyncData {
  is_active: boolean;
  total_segments: number;
  completed_segments: number;
  downloading_segments: number;
  waiting_segments: number;
  failed_segments: number;
  blocks_per_second: number;
  visual_progress: string;
  active_downloads: Array<{
    start_height: number;
    end_height: number;
    completion_percentage: number;
    peer_addr: string | null;
  }>;
  elapsed_seconds: number;
  total_downloaded_blocks: number;
  total_synced_blocks: number;
  sync_range: [number, number];
}

interface NodeStatusProps {
  compact?: boolean;
}

export default function NodeStatus({ compact = false }: NodeStatusProps) {
  const [status, setStatus] = useState<BlockchainStatus | null>(null);
  const [torrentSync, setTorrentSync] = useState<TorrentSyncData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = async () => {
    try {
      const [statusResponse, torrentResponse] = await Promise.all([
        fetch('/api/v1/status'),
        fetch('/api/v1/torrent-sync')
      ]);

      if (!statusResponse.ok || !torrentResponse.ok) {
        throw new Error('Failed to fetch status');
      }

      const statusData = await statusResponse.json();
      const torrentData = await torrentResponse.json();
      
      setStatus(statusData);
      setTorrentSync(torrentData);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 2000); // Update every 2 seconds
    return () => clearInterval(interval);
  }, []);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'synced': return 'text-green-400';
      case 'syncing': return 'text-yellow-400';
      case 'disconnected': return 'text-red-400';
      default: return 'text-gray-400';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'synced': return '✅';
      case 'syncing': return '🔄';
      case 'disconnected': return '❌';
      default: return '❓';
    }
  };

  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const formatDuration = (seconds: number) => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    
    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`;
    } else {
      return `${secs}s`;
    }
  };

  // Convert visual progress string to visual blocks
  const renderVisualProgress = (progress: string) => {
    return progress.split('').map((char, index) => {
      let bgColor = 'bg-gray-700';
      let title = 'Waiting';
      
      switch (char) {
        case '█':
          bgColor = 'bg-green-500';
          title = 'Completed';
          break;
        case '▓':
          bgColor = 'bg-green-400';
          title = 'Almost done (75%+)';
          break;
        case '▒':
          bgColor = 'bg-yellow-400';
          title = 'Half done (50%+)';
          break;
        case '░':
          bgColor = 'bg-orange-400';
          title = 'Started (25%+)';
          break;
        case '▁':
          bgColor = 'bg-blue-400';
          title = 'Just started';
          break;
        case '·':
          bgColor = 'bg-gray-600';
          title = 'Waiting';
          break;
        default:
          bgColor = 'bg-gray-700';
          title = 'Unknown';
      }
      
      return (
        <div
          key={index}
          className={`w-1 h-4 ${bgColor} rounded-sm`}
          title={title}
        />
      );
    });
  };

  if (compact) {
    return (
      <div className="flex items-center space-x-3">
        <div className="flex items-center space-x-1">
          <span className="text-xl">{status ? getStatusIcon(status.status) : '⏳'}</span>
          <span className={`text-sm font-medium ${status ? getStatusColor(status.status) : 'text-gray-400'}`}>
            {loading ? 'Checking...' : status?.status.toUpperCase() || 'Unknown'}
          </span>
        </div>
        
        {status && (
          <div className="flex items-center space-x-2 text-sm text-gray-600">
            <span>{status.block_height.toLocaleString()}</span>
            <span>•</span>
            <span>{status.peer_count} peers</span>
            {status.sync_progress < 100 && (
              <>
                <span>•</span>
                <span className="text-blue-600 font-medium">
                  {status.sync_progress.toFixed(1)}%
                </span>
              </>
            )}
          </div>
        )}

        {torrentSync && torrentSync.is_active && (
          <div className="flex items-center space-x-1">
            <span className="text-xs text-blue-500">🌊</span>
            <span className="text-xs text-blue-500 font-medium">
              {torrentSync.blocks_per_second.toFixed(0)} blk/s
            </span>
          </div>
        )}
      </div>
    );
  }

  if (loading) {
    return (
      <div className="bg-gray-800 rounded-lg p-4 border border-gray-700">
        <div className="animate-pulse">
          <div className="h-4 bg-gray-700 rounded w-1/4 mb-2"></div>
          <div className="h-3 bg-gray-700 rounded w-1/2"></div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-900/20 border border-red-500 rounded-lg p-4">
        <div className="text-red-400 font-medium">Node Connection Error</div>
        <div className="text-red-300 text-sm mt-1">{error}</div>
      </div>
    );
  }

  if (!status) return null;

  return (
    <div className="bg-gray-800 rounded-lg p-6 border border-gray-700 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white">Node Status</h3>
        <div className="flex items-center space-x-2">
          <span className="text-2xl">{getStatusIcon(status.status)}</span>
          <span className={`font-medium ${getStatusColor(status.status)}`}>
            {status.status.toUpperCase()}
          </span>
        </div>
      </div>

      {/* Basic Stats */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div className="bg-gray-700 rounded p-3">
          <div className="text-gray-400 text-sm">Block Height</div>
          <div className="text-white font-mono text-lg">{status.block_height.toLocaleString()}</div>
        </div>
        <div className="bg-gray-700 rounded p-3">
          <div className="text-gray-400 text-sm">Network Height</div>
          <div className="text-white font-mono text-lg">{status.estimated_network_height.toLocaleString()}</div>
        </div>
        <div className="bg-gray-700 rounded p-3">
          <div className="text-gray-400 text-sm">Sync Progress</div>
          <div className="text-white font-mono text-lg">{status.sync_progress.toFixed(1)}%</div>
        </div>
        <div className="bg-gray-700 rounded p-3">
          <div className="text-gray-400 text-sm">Peers</div>
          <div className="text-white font-mono text-lg">{status.peer_count}</div>
        </div>
      </div>

      {/* Sync Visualization - Show for both torrent and regular sync */}
      {torrentSync && (torrentSync.is_active || torrentSync.total_downloaded_blocks > 0) && (
        <div className="space-y-4">
          <div className="border-t border-gray-600 pt-4">
            <h4 className="text-white font-medium mb-3 flex items-center">
              {torrentSync.is_active ? '🌊 Torrent-Like Sync Active' : '📊 Sync Metrics'}
              {torrentSync.elapsed_seconds > 0 && (
                <span className="ml-2 text-sm text-gray-400">
                  ({formatDuration(torrentSync.elapsed_seconds)})
                </span>
              )}
            </h4>
            
            {/* Progress Bar - Show different views for torrent vs regular sync */}
            {torrentSync.is_active ? (
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-400">
                    {torrentSync.completed_segments.toLocaleString()} / {torrentSync.total_segments.toLocaleString()} segments
                  </span>
                  <span className="text-white font-mono">
                    {torrentSync.blocks_per_second.toFixed(0)} blocks/s
                  </span>
                </div>
                
                {/* Visual Progress */}
                <div className="bg-gray-900 p-2 rounded border">
                  <div className="flex gap-px overflow-hidden">
                    {renderVisualProgress(torrentSync.visual_progress)}
                  </div>
                </div>
                
                {/* Legend */}
                <div className="flex flex-wrap gap-4 text-xs text-gray-400">
                  <div className="flex items-center gap-1">
                    <div className="w-2 h-2 bg-green-500 rounded"></div>
                    <span>Complete</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <div className="w-2 h-2 bg-yellow-400 rounded"></div>
                    <span>Downloading</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <div className="w-2 h-2 bg-blue-400 rounded"></div>
                    <span>Starting</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <div className="w-2 h-2 bg-gray-600 rounded"></div>
                    <span>Waiting</span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-400">Regular Sync Mode</span>
                  <span className="text-white font-mono">
                    {((torrentSync.total_downloaded_blocks - (torrentSync.total_downloaded_blocks - 100)) / 10).toFixed(0)} blocks/s
                  </span>
                </div>
                
                {/* Simple Progress Bar */}
                <div className="bg-gray-900 p-2 rounded border">
                  <div className="w-full bg-gray-700 rounded-full h-4">
                    <div 
                      className="bg-gradient-to-r from-blue-500 to-green-500 h-4 rounded-full transition-all duration-300"
                      style={{ width: `${Math.min(status.sync_progress, 100)}%` }}
                    ></div>
                  </div>
                  <div className="text-center text-xs text-gray-400 mt-1">
                    {status.sync_progress.toFixed(1)}% synced
                  </div>
                </div>
              </div>
            )}

            {/* Status Cards - Different for torrent vs regular sync */}
            {torrentSync.is_active ? (
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mt-4">
                <div className="bg-green-900/20 border border-green-500/30 rounded p-3">
                  <div className="text-green-400 text-sm">✅ Completed</div>
                  <div className="text-white font-mono text-lg">{torrentSync.completed_segments}</div>
                </div>
                <div className="bg-blue-900/20 border border-blue-500/30 rounded p-3">
                  <div className="text-blue-400 text-sm">⬇ Downloading</div>
                  <div className="text-white font-mono text-lg">{torrentSync.downloading_segments}</div>
                </div>
                <div className="bg-gray-900/20 border border-gray-500/30 rounded p-3">
                  <div className="text-gray-400 text-sm">⏳ Waiting</div>
                  <div className="text-white font-mono text-lg">{torrentSync.waiting_segments}</div>
                </div>
                <div className="bg-red-900/20 border border-red-500/30 rounded p-3">
                  <div className="text-red-400 text-sm">❌ Failed</div>
                  <div className="text-white font-mono text-lg">{torrentSync.failed_segments}</div>
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-2 md:grid-cols-3 gap-4 mt-4">
                <div className="bg-blue-900/20 border border-blue-500/30 rounded p-3">
                  <div className="text-blue-400 text-sm">🔗 Current Height</div>
                  <div className="text-white font-mono text-lg">{status.block_height.toLocaleString()}</div>
                  <div className="text-blue-300 text-xs">local blockchain</div>
                </div>
                <div className="bg-green-900/20 border border-green-500/30 rounded p-3">
                  <div className="text-green-400 text-sm">🌐 Network Height</div>
                  <div className="text-white font-mono text-lg">{status.estimated_network_height.toLocaleString()}</div>
                  <div className="text-green-300 text-xs">estimated target</div>
                </div>
                <div className="bg-purple-900/20 border border-purple-500/30 rounded p-3">
                  <div className="text-purple-400 text-sm">📶 Peers</div>
                  <div className="text-white font-mono text-lg">{status.peer_count}</div>
                  <div className="text-purple-300 text-xs">connected</div>
                </div>
              </div>
            )}

            {/* Download Metrics */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
              <div className="bg-purple-900/20 border border-purple-500/30 rounded p-3">
                <div className="text-purple-400 text-sm">📥 Downloaded</div>
                <div className="text-white font-mono text-lg">{torrentSync.total_downloaded_blocks.toLocaleString()}</div>
                <div className="text-purple-300 text-xs">blocks received</div>
              </div>
              <div className="bg-cyan-900/20 border border-cyan-500/30 rounded p-3">
                <div className="text-cyan-400 text-sm">🔗 Synced</div>
                <div className="text-white font-mono text-lg">{torrentSync.total_synced_blocks.toLocaleString()}</div>
                <div className="text-cyan-300 text-xs">blocks processed</div>
              </div>
              <div className="bg-orange-900/20 border border-orange-500/30 rounded p-3">
                <div className="text-orange-400 text-sm">📦 Pending</div>
                <div className="text-white font-mono text-lg">{(torrentSync.total_downloaded_blocks - torrentSync.total_synced_blocks).toLocaleString()}</div>
                <div className="text-orange-300 text-xs">awaiting sync</div>
              </div>
            </div>

            {/* Sync Range Info - Only for torrent sync */}
            {torrentSync.is_active && torrentSync.sync_range[0] > 0 && (
              <div className="mt-4 bg-gray-900 rounded p-3">
                <div className="text-gray-400 text-sm mb-1">Sync Range</div>
                <div className="text-white font-mono text-sm">
                  Heights {torrentSync.sync_range[0].toLocaleString()} - {torrentSync.sync_range[1].toLocaleString()}
                  <span className="text-gray-400 ml-2">
                    ({(torrentSync.sync_range[1] - torrentSync.sync_range[0] + 1).toLocaleString()} blocks total)
                  </span>
                </div>
              </div>
            )}

            {/* Active Downloads */}
            {torrentSync.active_downloads.length > 0 && (
              <div className="mt-4">
                <h5 className="text-white font-medium mb-2">🔄 Active Downloads</h5>
                <div className="bg-gray-900 rounded p-3 max-h-40 overflow-y-auto">
                  <div className="space-y-1 text-xs font-mono">
                    {torrentSync.active_downloads.slice(0, 8).map((download, index) => (
                      <div key={index} className="flex justify-between text-gray-300">
                        <span>
                          {download.start_height.toLocaleString()}-{download.end_height.toLocaleString()}
                        </span>
                        <span className="text-gray-400">
                          {download.completion_percentage.toFixed(0)}%
                        </span>
                        <span className="text-blue-400 truncate max-w-24">
                          {download.peer_addr?.split(':')[0] || 'Unknown'}
                        </span>
                      </div>
                    ))}
                    {torrentSync.active_downloads.length > 8 && (
                      <div className="text-gray-500 text-center">
                        ... and {torrentSync.active_downloads.length - 8} more
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Footer Info */}
      <div className="border-t border-gray-600 pt-4 text-sm text-gray-400">
        <div className="flex justify-between">
          <span>Version: {status.version}</span>
          <span>Network: {status.network}</span>
        </div>
        {status.last_block_time && (
          <div className="mt-1">
            Last block: {formatTime(status.last_block_time)}
          </div>
        )}
      </div>
    </div>
  );
} 