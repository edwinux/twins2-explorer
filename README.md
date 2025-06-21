# TWINS 2.0 Explorer

A modern blockchain explorer for the TWINS network, featuring a high-performance Rust node with intelligent sync capabilities and a Next.js frontend.

## 🚀 Project Overview

This project consists of two main components:

### 1. TWINS Rust Node (`TWINS-Core/twins_node_rust`)
A high-performance TWINS blockchain node written in Rust with:
- **Hybrid Sync Manager**: Intelligently switches between sync modes based on peer types
- **Standard Sync**: Works with existing C++ TWINS nodes
- **Fast Sync Ready**: Prepared for future Rust nodes with parallel download capabilities
- **Real-time API**: REST endpoints for blockchain data, status, and sync metrics

### 2. TWINS Explorer Frontend (`twins-explorer`)
A modern Next.js 15 web application with:
- Real-time blockchain dashboard
- Block and transaction explorer
- Network statistics and monitoring
- Responsive design with Tailwind CSS

## 📊 Performance Metrics

Current sync performance with the hybrid sync implementation:
- **Sync Speed**: 100-150 blocks/second (average)
- **Burst Speed**: Up to 200+ blocks/second
- **Network**: Successfully syncs with TWINS mainnet
- **Peer Management**: Maintains 8-13 active peer connections

## 🛠️ Technical Stack

### Backend (Rust Node)
- **Language**: Rust
- **Database**: SQLite
- **Networking**: Tokio async runtime
- **API**: Axum web framework
- **Serialization**: Custom Bitcoin-style protocol implementation

### Frontend (Explorer)
- **Framework**: Next.js 15 with App Router
- **Styling**: Tailwind CSS
- **Language**: TypeScript
- **API Client**: Custom REST client with TypeScript interfaces

## 🏗️ Architecture Highlights

### Hybrid Sync Manager
The node features an intelligent sync manager that:
- Detects peer types (C++ vs Rust) via user agent strings
- Tracks sync metrics in real-time
- Automatically switches between sync modes:
  - **Standard Mode**: For C++ peers (currently active)
  - **Fast Mode**: For Rust peers (ready for future deployment)
  - **Hybrid Mode**: Mix of both peer types

### Key Innovations
1. **Parallel Sync Manager**: Multi-threaded block downloading (ready for activation)
2. **Orphan Block Handling**: Manages out-of-order block reception
3. **Automatic Peer Recovery**: Reconnects when peer count drops
4. **Real-time Metrics**: Comprehensive sync statistics via API

## 📦 Installation

### Prerequisites
- Rust 1.70+ and Cargo
- Node.js 18+ and npm
- SQLite3

### Building the Rust Node
```bash
cd TWINS-Core/twins_node_rust
cargo build --release
```

### Running the Node
```bash
./target/release/twins_node_rust
```

The node will:
- Listen on port 37817 (P2P)
- Serve API on port 3001 (HTTP)
- Store blockchain data in `twins_node_data.sqlite`

### Running the Explorer Frontend
```bash
cd twins-explorer
npm install
npm run dev
```

Visit http://localhost:3000 to view the explorer.

## 🔌 API Endpoints

The Rust node provides the following REST API endpoints:

- `GET /api/v1/status` - Node and sync status
- `GET /api/v1/blocks` - List recent blocks
- `GET /api/v1/blocks/:id` - Get block by height or hash
- `GET /api/v1/transactions/:hash` - Get transaction details
- `GET /api/v1/sporks` - Active sporks list
- `GET /api/v1/masternodes` - Masternode list
- `GET /api/v1/hybrid-sync` - Hybrid sync manager status
- `GET /api/v1/torrent-sync` - Parallel sync statistics

## 🚦 Current Status

✅ **Completed**:
- Full P2P protocol implementation
- Blockchain synchronization with mainnet
- Block and transaction storage
- REST API with CORS support
- Hybrid sync manager
- Basic explorer frontend
- Real-time sync monitoring

🔄 **In Progress**:
- Enhanced frontend features
- WebSocket support for real-time updates
- Advanced search functionality

🎯 **Future Plans**:
- Deploy Rust nodes to activate fast sync
- Implement mempool monitoring
- Add governance proposal tracking
- Enhanced masternode analytics

## 📈 Sync Progress Example

```json
{
  "syncMode": "standard",
  "totalBlocksProcessed": 34484,
  "blocksPerSecond": 96.67,
  "elapsedSeconds": 356.69,
  "rustPeerCount": 0,
  "cppPeerCount": 8,
  "fastSyncActive": false,
  "modeDescription": "Standard sync from 8 C++ nodes"
}
```

## 🤝 Contributing

This project is part of the TWINS blockchain ecosystem. Contributions are welcome!

## 📝 License

[License information to be added]

## 🙏 Acknowledgments

Built for the TWINS blockchain community to provide a modern, high-performance explorer experience. 