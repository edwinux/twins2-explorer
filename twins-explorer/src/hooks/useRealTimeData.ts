'use client';

import { useEffect, useState, useCallback } from 'react';
import { BlockchainStatus, BlockSummary, SporkDetail, twinsApi } from '@/lib/api';

interface RealTimeData {
  status: BlockchainStatus | null;
  recentBlocks: BlockSummary[];
  sporks: SporkDetail[];
  isConnected: boolean;
  lastUpdate: Date | null;
  error: string | null;
}

interface UseRealTimeDataOptions {
  includeBlocks?: boolean;
  includeSporks?: boolean;
  blockCount?: number;
}

export function useRealTimeData(options: UseRealTimeDataOptions = {}) {
  const { includeBlocks = false, includeSporks = false, blockCount = 5 } = options;
  
  const [data, setData] = useState<RealTimeData>({
    status: null,
    recentBlocks: [],
    sporks: [],
    isConnected: false,
    lastUpdate: null,
    error: null,
  });
  
  const [loading, setLoading] = useState(true);

  const fetchStaticData = useCallback(async () => {
    try {
      const promises: Promise<any>[] = [];
      
      if (includeBlocks) {
        promises.push(twinsApi.getLatestBlocks(blockCount));
      }
      
      if (includeSporks) {
        promises.push(twinsApi.getSporks());
      }

      const results = await Promise.all(promises);
      let resultIndex = 0;
      
      setData(prev => ({
        ...prev,
        recentBlocks: includeBlocks ? results[resultIndex++] : prev.recentBlocks,
        sporks: includeSporks ? results[resultIndex++] : prev.sporks,
        error: null,
      }));
    } catch (error) {
      console.error('Failed to fetch static data:', error);
      setData(prev => ({
        ...prev,
        error: 'Failed to load additional data',
      }));
    }
  }, [includeBlocks, includeSporks, blockCount]);

  useEffect(() => {
    let eventSource: EventSource | null = null;
    let fallbackInterval: NodeJS.Timeout | null = null;
    let staticDataInterval: NodeJS.Timeout | null = null;

    const connectToRealTimeStream = () => {
      try {
        eventSource = new EventSource('http://localhost:3001/api/v1/status/stream');
        
        eventSource.onopen = () => {
          console.log('Connected to real-time blockchain stream');
          setData(prev => ({ ...prev, isConnected: true, error: null }));
          setLoading(false);
        };

        eventSource.onmessage = (event) => {
          try {
            const statusData: BlockchainStatus = JSON.parse(event.data);
            
            setData(prev => ({
              ...prev,
              status: statusData,
              lastUpdate: new Date(),
              error: null,
            }));
          } catch (error) {
            console.error('Failed to parse SSE data:', error);
          }
        };

        eventSource.onerror = (error) => {
          console.error('SSE connection error:', error);
          setData(prev => ({ ...prev, isConnected: false }));
          
          // Fallback to polling if SSE fails
          if (!fallbackInterval) {
            console.log('Falling back to polling mode');
            fallbackInterval = setInterval(async () => {
              try {
                const statusData = await twinsApi.getStatus();
                setData(prev => ({
                  ...prev,
                  status: statusData,
                  lastUpdate: new Date(),
                  error: null,
                }));
                setLoading(false);
              } catch (error) {
                setData(prev => ({
                  ...prev,
                  error: 'Failed to connect to node',
                }));
              }
            }, 5000); // Poll every 5 seconds as fallback
          }
        };

      } catch (error) {
        console.error('Failed to create EventSource:', error);
        setData(prev => ({ ...prev, isConnected: false, error: 'Connection failed' }));
      }
    };

    // Initial connection
    connectToRealTimeStream();

    // Fetch static data initially and periodically
    fetchStaticData();
    if (includeBlocks || includeSporks) {
      staticDataInterval = setInterval(fetchStaticData, 30000); // Update static data every 30 seconds
    }

    return () => {
      if (eventSource) {
        eventSource.close();
      }
      if (fallbackInterval) {
        clearInterval(fallbackInterval);
      }
      if (staticDataInterval) {
        clearInterval(staticDataInterval);
      }
    };
  }, [fetchStaticData, includeBlocks, includeSporks]);

  return {
    ...data,
    loading,
    refresh: fetchStaticData,
  };
} 