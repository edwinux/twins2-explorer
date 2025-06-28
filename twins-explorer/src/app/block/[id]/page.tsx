'use client';

import { useParams } from 'next/navigation';
import Link from 'next/link';
import { twinsApi } from '@/lib/api';
import { useRealTimeBlock } from '@/hooks/useRealTimeBlock';

export default function BlockPage() {
  const params = useParams();
  const blockId = Array.isArray(params.id) ? params.id[0] : params.id || '';
  
  const { block, loading, error } = useRealTimeBlock({
    blockId,
    refreshOnNewBlocks: true,
  });

  if (loading) {
    return (
      <div className="flex justify-center items-center min-h-[400px]">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-[color:var(--color-accent)]"></div>
      </div>
    );
  }

  if (error || !block) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6">
        <div className="text-center">
          <h3 className="text-lg font-medium text-red-800">Block Not Found</h3>
          <p className="mt-1 text-sm text-red-700">{error}</p>
          <Link
            href="/"
            className="mt-4 inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-[color:var(--color-accent)] hover:brightness-90"
          >
            ← Back to Home
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="card">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">Block {block.height.toLocaleString()}</h1>
            <p className="text-sm text-gray-600 mt-1">
              {block.confirmations ? `${block.confirmations} confirmations` : 'Unconfirmed'}
            </p>
          </div>
          <div className="flex space-x-3">
            {block.previousBlockHash && (
              <Link
                href={`/block/${block.height - 1}`}
                className="inline-flex items-center px-3 py-2 border border-gray-300 shadow-sm text-sm leading-4 font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50"
              >
                ← Previous
              </Link>
            )}
            <Link
              href={`/block/${block.height + 1}`}
              className="inline-flex items-center px-3 py-2 border border-gray-300 shadow-sm text-sm leading-4 font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50"
            >
              Next →
            </Link>
          </div>
        </div>
      </div>

      {/* Block Details */}
      <div className="card p-0">
        <div className="px-6 py-4 border-b border-gray-200">
          <h2 className="text-lg font-semibold text-gray-900">Block Information</h2>
        </div>
        <div className="p-6">
          <dl className="grid grid-cols-1 gap-x-4 gap-y-6 sm:grid-cols-2">
            <div>
              <dt className="text-sm font-medium text-gray-500">Hash</dt>
              <dd className="mt-1 text-sm text-gray-900 font-mono break-all">{block.hash}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Height</dt>
              <dd className="mt-1 text-sm text-gray-900">{block.height.toLocaleString()}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Timestamp</dt>
              <dd className="mt-1 text-sm text-gray-900">{twinsApi.formatTimestamp(block.time)}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Size</dt>
              <dd className="mt-1 text-sm text-gray-900">{block.size.toLocaleString()} bytes</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Version</dt>
              <dd className="mt-1 text-sm text-gray-900">{block.version} ({block.versionHex})</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Transactions</dt>
              <dd className="mt-1 text-sm text-gray-900">{block.nTx}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Merkle Root</dt>
              <dd className="mt-1 text-sm text-gray-900 font-mono break-all">{block.merkleRoot}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Nonce</dt>
              <dd className="mt-1 text-sm text-gray-900">{block.nonce}</dd>
            </div>
            
            <div>
              <dt className="text-sm font-medium text-gray-500">Bits</dt>
              <dd className="mt-1 text-sm text-gray-900 font-mono">{block.bits}</dd>
            </div>
            
            {block.previousBlockHash && (
              <div>
                <dt className="text-sm font-medium text-gray-500">Previous Block</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  <Link 
                    href={`/block/${block.previousBlockHash}`}
                    className="font-mono text-[color:var(--color-accent)] hover:underline break-all"
                  >
                    {block.previousBlockHash}
                  </Link>
                </dd>
              </div>
            )}

            {/* PoS specific fields */}
            {block.coinstakeTxid && (
              <div>
                <dt className="text-sm font-medium text-gray-500">Coinstake Transaction</dt>
                <dd className="mt-1 text-sm text-gray-900">
                  <Link 
                    href={`/tx/${block.coinstakeTxid}`}
                    className="font-mono text-[color:var(--color-accent)] hover:underline break-all"
                  >
                    {block.coinstakeTxid}
                  </Link>
                </dd>
              </div>
            )}

            {block.blockSignature && (
              <div className="sm:col-span-2">
                <dt className="text-sm font-medium text-gray-500">Block Signature (PoS)</dt>
                <dd className="mt-1 text-sm text-gray-900 font-mono break-all">{block.blockSignature}</dd>
              </div>
            )}
          </dl>
        </div>
      </div>

      {/* Transactions */}
      <div className="card p-0">
        <div className="px-6 py-4 border-b border-gray-200">
          <h2 className="text-lg font-semibold text-gray-900">
            Transactions ({block.nTx})
          </h2>
        </div>
        <div className="divide-y divide-gray-200">
          {block.tx.map((txid, index) => (
            <div key={txid} className="px-6 py-4 hover:bg-gray-50">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-3">
                  <span className="text-sm text-gray-500">#{index + 1}</span>
                  <Link
                    href={`/tx/${txid}`}
                    className="font-mono text-sm text-[color:var(--color-accent)] hover:underline"
                  >
                    {twinsApi.shortenHash(txid, 12)}
                  </Link>
                  {index === 0 && block.coinbaseTxid && (
                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                      Coinbase
                    </span>
                  )}
                  {index === 1 && block.coinstakeTxid && (
                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-[color:var(--color-accent)]/20 text-[color:var(--color-accent)]">
                      Coinstake
                    </span>
                  )}
                </div>
                <Link 
                  href={`/tx/${txid}`}
                  className="text-sm text-[color:var(--color-accent)] hover:underline"
                >
                  View →
                </Link>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
} 