'use client';

import Link from 'next/link';
import { useRealTimeData } from '@/hooks/useRealTimeData';

export default function StatsPage() {
  const { status, recentBlocks, sporks, loading, error, isConnected, lastUpdate } = useRealTimeData({
    includeBlocks: true,
    includeSporks: true,
    blockCount: 10,
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[color:var(--color-accent)]"></div>
      </div>
    );
  }

  if (error || !status) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6 max-w-2xl mx-auto">
        <div className="text-center">
          <h3 className="text-lg font-medium text-red-800">Error Loading Stats</h3>
          <p className="mt-1 text-sm text-red-700">{error}</p>
        </div>
      </div>
    );
  }

  // Calculate some basic statistics
  const avgBlockTime = recentBlocks.length > 1 
    ? (recentBlocks[0].time - recentBlocks[recentBlocks.length - 1].time) / (recentBlocks.length - 1)
    : 0;

  const totalTransactions = recentBlocks.reduce((sum, block) => sum + block.nTx, 0);
  const avgTxPerBlock = recentBlocks.length > 0 ? totalTransactions / recentBlocks.length : 0;

  return (
    <div className="space-y-8">
      {/* Page Header */}
      <div className="card">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold text-gray-900">TWINS Network Statistics</h1>
            <p className="mt-2 text-gray-600">Comprehensive overview of the TWINS blockchain network</p>
          </div>
          <div className="flex items-center space-x-3">
            <div className="flex items-center space-x-2">
              <div className={`w-3 h-3 rounded-full ${isConnected ? 'bg-green-400' : 'bg-red-400'}`}></div>
              <span className="text-sm font-medium text-gray-700">
                {isConnected ? 'Real-time Data' : 'Offline'}
              </span>
            </div>
            {lastUpdate && (
              <span className="text-xs text-gray-500">
                Last update: {lastUpdate.toLocaleTimeString()}
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Core Network Stats */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <div className="card">
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className="w-8 h-8 rounded-md flex items-center justify-center bg-[color:var(--color-accent)]/20">
                <svg className="w-5 h-5 text-[color:var(--color-accent)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
                </svg>
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900">Block Height</h3>
              <p className="text-2xl font-bold text-[color:var(--color-accent)]">{status.block_height.toLocaleString()}</p>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className={`w-8 h-8 rounded-md flex items-center justify-center ${
                status.status === 'synced' ? 'bg-green-100' : 
                status.status === 'syncing' ? 'bg-yellow-100' : 'bg-red-100'
              }`}>
                <svg className={`w-5 h-5 ${
                  status.status === 'synced' ? 'text-green-600' : 
                  status.status === 'syncing' ? 'text-yellow-600' : 'text-red-600'
                }`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900">Network Status</h3>
              <div className="flex items-center space-x-2">
                <p className={`text-2xl font-bold capitalize ${
                  status.status === 'synced' ? 'text-green-600' : 
                  status.status === 'syncing' ? 'text-yellow-600' : 'text-red-600'
                }`}>{status.status}</p>
                {status.status === 'syncing' && (
                  <span className="text-sm text-gray-500">
                    ({status.sync_progress.toFixed(1)}%)
                  </span>
                )}
              </div>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className="w-8 h-8 bg-purple-100 rounded-md flex items-center justify-center">
                <svg className="w-5 h-5 text-purple-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4" />
                </svg>
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900">Protocol Version</h3>
              <p className="text-2xl font-bold text-purple-600">{status.protocol_version}</p>
            </div>
          </div>
        </div>

        <div className="card">
          <div className="flex items-center">
            <div className="flex-shrink-0">
              <div className="w-8 h-8 bg-orange-100 rounded-md flex items-center justify-center">
                <svg className="w-5 h-5 text-orange-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
            </div>
            <div className="ml-4">
              <h3 className="text-lg font-medium text-gray-900">Avg Block Time</h3>
              <p className="text-2xl font-bold text-orange-600">
                {avgBlockTime > 0 ? `${Math.round(avgBlockTime)}s` : 'N/A'}
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Additional Statistics */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
        {/* Block Statistics */}
        <div className="card">
          <h2 className="text-xl font-bold text-gray-900 mb-4">Block Statistics</h2>
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Current Height</span>
              <span className="text-sm text-gray-900">{status.block_height.toLocaleString()}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Header Height</span>
              <span className="text-sm text-gray-900">{status.header_height.toLocaleString()}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Last Block Time</span>
              <span className="text-sm text-gray-900">
                {status.last_block_time 
                  ? new Date(status.last_block_time * 1000).toLocaleString()
                  : 'Unknown'
                }
              </span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Avg Transactions/Block</span>
              <span className="text-sm text-gray-900">{avgTxPerBlock.toFixed(1)}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Network</span>
              <span className="text-sm text-gray-900 capitalize">{status.network}</span>
            </div>
          </div>
        </div>

        {/* Version Information */}
        <div className="card">
          <h2 className="text-xl font-bold text-gray-900 mb-4">Node Information</h2>
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Node Version</span>
              <span className="text-sm text-gray-900 font-mono">{status.version}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Protocol Version</span>
              <span className="text-sm text-gray-900">{status.protocol_version}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Consensus</span>
              <span className="text-sm text-gray-900">Proof of Stake (PoS)</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Block Time Target</span>
              <span className="text-sm text-gray-900">60 seconds</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-sm font-medium text-gray-500">Block Reward</span>
              <span className="text-sm text-gray-900">Variable (PoS)</span>
            </div>
          </div>
        </div>
      </div>

      {/* Sporks Information */}
      {sporks.length > 0 && (
        <div className="card">
          <h2 className="text-xl font-bold text-gray-900 mb-4">Network Sporks</h2>
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
                             <thead className="bg-gray-50">
                 <tr>
                   <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                     Spork ID
                   </th>
                   <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                     Name
                   </th>
                   <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                     Value
                   </th>
                   <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                     Status
                   </th>
                 </tr>
               </thead>
               <tbody className="bg-white divide-y divide-gray-200">
                 {sporks.map((spork) => (
                   <tr key={spork.id} className="hover:bg-gray-50">
                     <td className="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                       {spork.id}
                     </td>
                     <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900 font-mono">
                       {spork.name}
                     </td>
                     <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                       {spork.value !== null ? spork.value.toLocaleString() : 'Not Set'}
                     </td>
                     <td className="px-6 py-4 whitespace-nowrap">
                       <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
                         spork.active 
                           ? 'bg-green-100 text-green-800' 
                           : 'bg-gray-100 text-gray-800'
                       }`}>
                         {spork.active ? 'Active' : 'Inactive'}
                       </span>
                     </td>
                   </tr>
                 ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Recent Blocks Summary */}
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold text-gray-900">Recent Block Activity</h2>
          <Link
            href="/"
            className="font-medium text-[color:var(--color-accent)] hover:underline"
          >
            View All →
          </Link>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
          {recentBlocks.slice(0, 5).map((block) => (
            <div key={block.hash} className="border rounded-lg p-4 hover:bg-gray-50">
              <Link href={`/block/${block.height}`}>
                <div className="text-center">
                  <div className="text-lg font-bold text-[color:var(--color-accent)]">{block.height.toLocaleString()}</div>
                  <div className="text-sm text-gray-500">{block.nTx} txs</div>
                  <div className="text-xs text-gray-400 mt-1">
                    {new Date(block.time * 1000).toLocaleTimeString()}
                  </div>
                </div>
              </Link>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
} 