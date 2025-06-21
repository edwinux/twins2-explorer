'use client';

import { useEffect, useState, useCallback } from 'react';
import { BlockDetail, twinsApi } from '@/lib/api';

interface UseRealTimeBlockOptions {
  blockId: string;
  refreshOnNewBlocks?: boolean;
}

export function useRealTimeBlock({ blockId, refreshOnNewBlocks = true }: UseRealTimeBlockOptions) {
  const [block, setBlock] = useState<BlockDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastKnownHeight, setLastKnownHeight] = useState<number>(0);

  const fetchBlock = useCallback(async () => {
    if (!blockId) return;
    
    try {
      setLoading(true);
      const isHeight = /^\d+$/.test(blockId);
      const blockData = isHeight 
        ? await twinsApi.getBlockByHeight(parseInt(blockId))
        : await twinsApi.getBlockByHash(blockId);
        
      setBlock(blockData);
      setError(null);
    } catch (err) {
      console.error('Failed to fetch block:', err);
      setError('Block not found');
      setBlock(null);
    } finally {
      setLoading(false);
    }
  }, [blockId]);

  useEffect(() => {
    fetchBlock();
  }, [fetchBlock]);

  // Listen for new blocks and refresh if needed
  useEffect(() => {
    if (!refreshOnNewBlocks) return;

    let eventSource: EventSource | null = null;

    const connectToStream = () => {
      try {
        eventSource = new EventSource('/api/v1/status/stream');
        
        eventSource.onmessage = (event) => {
          try {
            const statusData = JSON.parse(event.data);
            const currentHeight = statusData.block_height;
            
            // If we have a new block and we're viewing a recent block, refresh
            if (currentHeight > lastKnownHeight) {
              setLastKnownHeight(currentHeight);
              
              // Only refresh if we're viewing one of the last 10 blocks
              if (block && currentHeight - block.height <= 10) {
                fetchBlock();
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
  }, [refreshOnNewBlocks, lastKnownHeight, block, fetchBlock]);

  return {
    block,
    loading,
    error,
    refresh: fetchBlock,
  };
} 