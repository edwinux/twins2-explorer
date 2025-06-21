'use client';

import { useEffect, useState } from 'react';
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

  useEffect(() => {
    const fetchData = async () => {
      try {
        const statusData = await twinsApi.getStatus();
        let recentBlocks: BlockSummary[] = [];
        let sporks: SporkDetail[] = [];
        
        if (includeBlocks) {
          recentBlocks = await twinsApi.getLatestBlocks(blockCount);
        }
        
        if (includeSporks) {
          sporks = await twinsApi.getSporks();
        }
        
        setData({
          status: statusData,
          recentBlocks,
          sporks,
          isConnected: true,
          lastUpdate: new Date(),
          error: null,
        });
        setLoading(false);
      } catch {
        setData(prev => ({
          ...prev,
          error: 'Failed to connect to node',
          isConnected: false,
        }));
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 30000);
    
    return () => clearInterval(interval);
  }, [includeBlocks, includeSporks, blockCount]);

  const refresh = async () => {
    // Placeholder refresh function
  };

  return {
    ...data,
    loading,
    refresh,
  };
}
