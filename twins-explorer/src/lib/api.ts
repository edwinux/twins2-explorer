// API client for TWINS Rust Node
const TWINS_API_BASE_URL = process.env.NEXT_PUBLIC_TWINS_API_URL || 'http://localhost:3001';

export interface BlockchainStatus {
  status: string;
  version: string;
  protocol_version: number;
  block_height: number;
  header_height: number;
  estimated_network_height: number;
  sync_progress: number; // Percentage (0.0 to 100.0)
  peer_count: number;
  last_block_time: number | null;
  network: string;
}

export interface BlockSummary {
  hash: string;
  height: number;
  time: number;
  nTx: number;
}

export interface BlockDetail {
  hash: string;
  confirmations: number | null;
  size: number;
  height: number;
  version: number;
  versionHex: string;
  merkleRoot: string;
  time: number;
  medianTime: number | null;
  nonce: number;
  bits: string;
  difficulty: number | null;
  chainwork: string | null;
  nTx: number;
  previousBlockHash: string | null;
  nextBlockHash: string | null;
  coinbaseTxid: string | null;
  coinstakeTxid: string | null;
  blockSignature: string | null;
  accumulatorCheckpoint: string | null;
  tx: string[];
}

export interface TransactionInput {
  coinbase: string | null;
  txid: string | null;
  vout: number | null;
  scriptSig: {
    asm: string | null;
    hex: string;
  };
  sequence: number;
  prevOutValue: string | null;
  prevOutAddress: string | null;
}

export interface TransactionOutput {
  value: string;
  n: number;
  scriptPubKey: {
    asm: string | null;
    hex: string;
    reqSigs: number | null;
    type: string;
    addresses: string[] | null;
  };
  spentTxid: string | null;
  spentIndex: number | null;
}

export interface TransactionDetail {
  txid: string;
  hash: string;
  version: number;
  size: number;
  locktime: number;
  vin: TransactionInput[];
  vout: TransactionOutput[];
  blockhash: string | null;
  confirmations: number | null;
  time: number | null;
  blocktime: number | null;
  isCoinbase: boolean;
  isCoinstake: boolean;
}

export interface SporkDetail {
  id: number;
  name: string;
  value: number | null;
  active: boolean;
}

export interface TorrentSyncData {
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

class TwinsApiClient {
  private baseUrl: string;

  constructor(baseUrl: string = TWINS_API_BASE_URL) {
    this.baseUrl = baseUrl;
  }

  private async request<T>(endpoint: string): Promise<T> {
    const url = `${this.baseUrl}/api/v1${endpoint}`;
    
    try {
      const response = await fetch(url, {
        headers: {
          'Content-Type': 'application/json',
        },
        // Add cache control for real-time data
        cache: 'no-store',
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      return await response.json();
    } catch (error) {
      console.error(`API request failed for ${endpoint}:`, error);
      throw error;
    }
  }

  // Blockchain status and info
  async getStatus(): Promise<BlockchainStatus> {
    return this.request<BlockchainStatus>('/status');
  }

  // Block endpoints
  async getLatestBlocks(count: number = 10): Promise<BlockSummary[]> {
    return this.request<BlockSummary[]>(`/blocks/latest?count=${count}`);
  }

  async getBlockByHeight(height: number): Promise<BlockDetail> {
    return this.request<BlockDetail>(`/block/height/${height}`);
  }

  async getBlockByHash(hash: string): Promise<BlockDetail> {
    return this.request<BlockDetail>(`/block/hash/${hash}`);
  }

  // Transaction endpoints
  async getTransaction(txid: string): Promise<TransactionDetail> {
    return this.request<TransactionDetail>(`/tx/${txid}`);
  }

  // Sporks endpoint
  async getSporks(): Promise<SporkDetail[]> {
    return this.request<SporkDetail[]>('/sporks');
  }

  // Torrent sync endpoint
  async getTorrentSync(): Promise<TorrentSyncData> {
    return this.request<TorrentSyncData>('/torrent-sync');
  }

  // Utility methods
  formatTwinsAmount(satoshis: string | number): string {
    const amount = typeof satoshis === 'string' ? parseFloat(satoshis) : satoshis;
    return (amount / 100_000_000).toFixed(8);
  }

  formatTimestamp(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString();
  }

  shortenHash(hash: string, length: number = 8): string {
    return `${hash.slice(0, length)}...${hash.slice(-length)}`;
  }
}

export const twinsApi = new TwinsApiClient();
export default twinsApi; 