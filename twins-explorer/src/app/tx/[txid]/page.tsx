'use client';

import { useParams } from 'next/navigation';
import Link from 'next/link';
import { useRealTimeTransaction } from '@/hooks/useRealTimeTransaction';

export default function TransactionPage() {
  const params = useParams();
  const txid = params.txid as string;
  
  const { transaction, loading, error } = useRealTimeTransaction({
    txid,
    refreshOnNewBlocks: true,
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  if (error || !transaction) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6 max-w-4xl mx-auto">
        <div className="flex items-center">
          <div className="flex-shrink-0">
            <svg className="h-5 w-5 text-red-400" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
              <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 000-1.414z" clipRule="evenodd" />
            </svg>
          </div>
          <div className="ml-3">
            <h3 className="text-sm font-medium text-red-800">Transaction Not Found</h3>
            <p className="mt-1 text-sm text-red-700">{error}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Transaction Header */}
      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-2xl font-bold text-gray-900">Transaction Details</h1>
          <div className="flex space-x-2">
            {transaction.isCoinbase && (
              <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-800">
                Coinbase
              </span>
            )}
            {transaction.isCoinstake && (
              <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                Coinstake
              </span>
            )}
          </div>
        </div>
        
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <label className="block text-sm font-medium text-gray-500">Transaction ID</label>
            <p className="mt-1 text-sm font-mono text-gray-900 break-all">{transaction.txid}</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-500">Hash</label>
            <p className="mt-1 text-sm font-mono text-gray-900 break-all">{transaction.hash}</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-500">Size</label>
            <p className="mt-1 text-sm text-gray-900">{transaction.size} bytes</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-500">Version</label>
            <p className="mt-1 text-sm text-gray-900">{transaction.version}</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-500">Lock Time</label>
            <p className="mt-1 text-sm text-gray-900">{transaction.locktime}</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-500">Confirmations</label>
            <p className="mt-1 text-sm text-gray-900">
              {transaction.confirmations ? transaction.confirmations.toLocaleString() : 'Unconfirmed'}
            </p>
          </div>
        </div>

        {transaction.blockhash && (
          <div className="mt-4">
            <label className="block text-sm font-medium text-gray-500">Block Hash</label>
            <Link 
              href={`/block/${transaction.blockhash}`}
              className="mt-1 text-sm font-mono text-blue-600 hover:text-blue-800 break-all"
            >
              {transaction.blockhash}
            </Link>
          </div>
        )}

        {transaction.time && (
          <div className="mt-4">
            <label className="block text-sm font-medium text-gray-500">Time</label>
            <p className="mt-1 text-sm text-gray-900">
              {new Date(transaction.time * 1000).toLocaleString()}
            </p>
          </div>
        )}
      </div>

      {/* Transaction Inputs */}
      <div className="card">
        <h2 className="text-xl font-bold text-gray-900 mb-4">
          Inputs ({transaction.vin.length})
        </h2>
        <div className="space-y-4">
          {transaction.vin.map((input, index) => (
            <div key={index} className="border rounded-lg p-4 bg-gray-50">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-medium text-gray-500">Input #{index}</span>
                <span className="text-sm text-gray-500">Sequence: {input.sequence}</span>
              </div>
              
              {input.coinbase ? (
                <div>
                  <label className="block text-sm font-medium text-gray-500">Coinbase</label>
                  <p className="mt-1 text-sm font-mono text-gray-900 break-all">{input.coinbase}</p>
                </div>
              ) : (
                <div className="space-y-2">
                  <div>
                    <label className="block text-sm font-medium text-gray-500">Previous Transaction</label>
                    <p className="mt-1 text-sm font-mono text-blue-600 break-all">{input.txid}</p>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-500">Output Index</label>
                    <p className="mt-1 text-sm text-gray-900">{input.vout}</p>
                  </div>
                </div>
              )}
              
              <div className="mt-2">
                <label className="block text-sm font-medium text-gray-500">Script Signature</label>
                <p className="mt-1 text-xs font-mono text-gray-600 break-all bg-white p-2 rounded">
                  {input.scriptSig.hex || 'No script'}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Transaction Outputs */}
      <div className="card">
        <h2 className="text-xl font-bold text-gray-900 mb-4">
          Outputs ({transaction.vout.length})
        </h2>
        <div className="space-y-4">
          {transaction.vout.map((output, index) => (
            <div key={index} className="border rounded-lg p-4 bg-gray-50">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-medium text-gray-500">Output #{output.n}</span>
                <span className="text-lg font-bold text-green-600">{output.value} TWINS</span>
              </div>
              
              <div className="space-y-2">
                <div>
                  <label className="block text-sm font-medium text-gray-500">Script Type</label>
                  <p className="mt-1 text-sm text-gray-900">{output.scriptPubKey.type}</p>
                </div>
                
                {output.scriptPubKey.addresses && output.scriptPubKey.addresses.length > 0 && (
                  <div>
                    <label className="block text-sm font-medium text-gray-500">Address(es)</label>
                    <div className="mt-1 space-y-1">
                      {output.scriptPubKey.addresses.map((address, addrIndex) => (
                        <p key={addrIndex} className="text-sm font-mono text-blue-600 break-all">
                          {address}
                        </p>
                      ))}
                    </div>
                  </div>
                )}
                
                <div>
                  <label className="block text-sm font-medium text-gray-500">Script Public Key</label>
                  <p className="mt-1 text-xs font-mono text-gray-600 break-all bg-white p-2 rounded">
                    {output.scriptPubKey.hex}
                  </p>
                </div>

                {output.spentTxid && (
                  <div>
                    <label className="block text-sm font-medium text-gray-500">Spent In</label>
                    <Link 
                      href={`/tx/${output.spentTxid}`}
                      className="mt-1 text-sm font-mono text-red-600 hover:text-red-800 break-all"
                    >
                      {output.spentTxid}
                    </Link>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
} 