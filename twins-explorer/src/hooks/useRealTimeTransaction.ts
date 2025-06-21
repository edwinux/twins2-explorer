'use client';

import { useEffect, useState, useCallback } from 'react';
import { TransactionDetail, twinsApi } from '@/lib/api';

interface UseRealTimeTransactionOptions {
  txid: string;
  refreshOnNewBlocks?: boolean;
}

export function useRealTimeTransaction({ txid, refreshOnNewBlocks = true }: UseRealTimeTransactionOptions) {
  const [transaction, setTransaction] = useState<TransactionDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastKnownHeight, setLastKnownHeight] = useState<number>(0);

  const fetchTransaction = useCallback(async () => {
    if (!txid) return;
    
    try {
      setLoading(true);
      const txData = await twinsApi.getTransaction(txid);
      setTransaction(txData);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch transaction:', err);
      setError('Transaction not found');
      setTransaction(null);
    } finally {
      setLoading(false);
    }
  }, [txid]);

  useEffect(() => {
    fetchTransaction();
  }, [fetchTransaction]);

  // Listen for new blocks and refresh unconfirmed transactions
  useEffect(() => {
    if (!refreshOnNewBlocks) return;

    let eventSource: EventSource | null = null;

    const connectToStream = () => {
      try {
        eventSource = new EventSource('http://localhost:3001/api/v1/status/stream');
        
        eventSource.onmessage = (event) => {
          try {
            const statusData = JSON.parse(event.data);
            const currentHeight = statusData.block_height;
            
            // If we have a new block, refresh unconfirmed transactions or recent confirmations
            if (currentHeight > lastKnownHeight) {
              setLastKnownHeight(currentHeight);
              
              // Refresh if transaction is unconfirmed or has few confirmations
              if (transaction && (!transaction.confirmations || transaction.confirmations < 6)) {
                fetchTransaction();
              }
            }
          } catch (error) {
            console.error('Failed to parse SSE data:', error);
          }
        };

        eventSource.onerror = () => {
          // Silently handle errors - we'll just miss real-time updates
        };

      } catch (error) {
        console.error('Failed to create EventSource:', error);
      }
    };

    connectToStream();

    return () => {
      if (eventSource) {
        eventSource.close();
      }
    };
  }, [refreshOnNewBlocks, lastKnownHeight, transaction, fetchTransaction]);

  return {
    transaction,
    loading,
    error,
    refresh: fetchTransaction,
  };
} 