'use client';

import Link from 'next/link';
import SearchBar from '@/components/SearchBar';
import { useRealTimeData } from '@/hooks/useRealTimeData';

export default function HomePage() {
  const { status, recentBlocks, loading, error, isConnected, lastUpdate } = useRealTimeData({
    includeBlocks: true,
    blockCount: 5,
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6 max-w-2xl mx-auto">
        <div className="flex items-center">
          <div className="flex-shrink-0">
            {/* Heroicon name: solid/x-circle */}
            <svg className="h-5 w-5 text-red-400" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 000-1.414z" clipRule="evenodd" />
            </svg>
          </div>
          <div className="ml-3">
            <h3 className="text-sm font-medium text-red-800">API Connection Error</h3>
            <p className="mt-1 text-sm text-red-700">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Search Bar */}
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xl font-bold text-gray-900">Search the TWINS Blockchain</h2>
          <div className="flex items-center space-x-2">
            <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-400' : 'bg-red-400'}`}></div>
            <span className="text-sm text-gray-500">
              {isConnected ? 'Live Data' : 'Offline'}
            </span>
            {lastUpdate && (
              <span className="text-xs text-gray-400">
                Updated {lastUpdate.toLocaleTimeString()}
              </span>
            )}
          </div>
        </div>
        <SearchBar />
      </div>

      {/* Network Status Overview */}
      <div className="card">
        <h2 className="text-2xl font-bold text-gray-900 mb-6">TWINS Network Status</h2>
        {status ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
            <div className="bg-blue-50 rounded-lg p-4">
              <div className="text-blue-600 text-sm font-medium">Block Height</div>
              <div className="text-2xl font-bold text-blue-900">{status.block_height.toLocaleString()}</div>
            </div>
            <div className="bg-green-50 rounded-lg p-4">
              <div className="text-green-600 text-sm font-medium">Network</div>
              <div className="text-2xl font-bold text-green-900">{status.network}</div>
            </div>
            <div className="bg-purple-50 rounded-lg p-4">
              <div className="text-purple-600 text-sm font-medium">Protocol Version</div>
              <div className="text-2xl font-bold text-purple-900">{status.protocol_version}</div>
            </div>
            <div className="bg-orange-50 rounded-lg p-4">
              <div className="text-orange-600 text-sm font-medium">Node Status</div>
              <div className="text-2xl font-bold text-orange-900 capitalize">{status.status}</div>
            </div>
          </div>
        ) : (
          <p className="text-gray-500">Loading network status...</p>
        )}
      </div>

      {/* Recent Blocks */}
      <div className="card">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-2xl font-bold text-gray-900">Recent Blocks</h2>
          <Link 
            href="/blocks" // Assuming a future page for all blocks
            className="text-blue-600 hover:text-blue-800 font-medium"
          >
            View All Blocks →
          </Link>
        </div>
        
        {recentBlocks.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Height</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Hash</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Transactions</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Time</th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {recentBlocks.map((block) => (
                  <tr key={block.hash} className="hover:bg-gray-50">
                    <td className="px-6 py-4 whitespace-nowrap">
                      <Link 
                        href={`/block/${block.height}`}
                        className="text-blue-600 hover:text-blue-800 font-medium"
                      >
                        {block.height.toLocaleString()}
                      </Link>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <Link 
                        href={`/block/${block.hash}`}
                        className="text-gray-900 font-mono text-sm hover:text-blue-600"
                      >
                        {block.hash.substring(0, 16)}...
                      </Link>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {block.nTx}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {new Date(block.time * 1000).toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-gray-500">Loading recent blocks or no blocks to display...</p>
        )}
      </div>

      {/* Quick Actions */}
      <div className="card">
        <h2 className="text-2xl font-bold text-gray-900 mb-6">Explorer Features</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <Link href="/stats" className="p-4 border border-gray-200 rounded-lg hover:border-blue-300 hover:bg-blue-50 transition-colors">
            <div className="font-medium text-gray-900">📊 Network Statistics</div>
            <p className="text-sm text-gray-500 mt-1">View detailed network stats, sporks, and performance metrics.</p>
          </Link>
          <div className="p-4 border border-gray-200 rounded-lg">
            <div className="font-medium text-gray-900">🔍 Search Functionality</div>
            <p className="text-sm text-gray-500 mt-1">Search by block height, hash, transaction ID, or address.</p>
          </div>
          <div className="p-4 border border-gray-200 rounded-lg">
            <div className="font-medium text-gray-900">⚡ Real-time Updates</div>
            <p className="text-sm text-gray-500 mt-1">Live blockchain data with automatic 30-second refresh.</p>
          </div>
        </div>
      </div>
    </div>
  );
}
