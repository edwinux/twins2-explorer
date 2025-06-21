// TWINS-Core/twins_node_rust/src/p2p/messages.rs
use std::net::{Ipv4Addr};
use std::io::{Read, Write, Error as IoError, ErrorKind as IoErrorKind, Cursor};
use byteorder::{LittleEndian, BigEndian, ReadBytesExt, WriteBytesExt};
use chrono;
use sha2::{Sha256, Digest};
use secp256k1::{Secp256k1, Message as SecpMessage, PublicKey, ecdsa::Signature};
use hex;
use serde::{Serialize, Deserialize};

pub const MAINNET_MAGIC: [u8; 4] = [0x2f, 0x1c, 0xd3, 0x0a];
pub const PROTOCOL_VERSION: i32 = 70927;
pub const MIN_PEER_PROTO_VERSION: i32 = 70927;
pub const NODE_NETWORK: u64 = 1;
pub const CMD_VERSION: &[u8; 12] = b"version\0\0\0\0\0";
pub const CMD_VERACK: &[u8; 12] = b"verack\0\0\0\0\0\0";
pub const CMD_PING: &[u8; 12] = b"ping\0\0\0\0\0\0\0\0";
pub const CMD_PONG: &[u8; 12] = b"pong\0\0\0\0\0\0\0\0";
pub const CMD_GETHEADERS: &[u8; 12] = b"getheaders\0\0";
pub const CMD_GETBLOCKS: &[u8; 12] = b"getblocks\0\0\0";
pub const CMD_HEADERS: &[u8; 12] = b"headers\0\0\0\0\0";
pub const CMD_GETDATA: &[u8; 12] = b"getdata\0\0\0\0\0";
pub const CMD_BLOCK: &[u8; 12] = b"block\0\0\0\0\0\0\0";
pub const CMD_MNB: &[u8; 12] = b"mnb         ";
pub const CMD_MNP: &[u8; 12] = b"mnp         ";
pub const CMD_GETSPORKS: &[u8;12] = b"getsporks   ";
pub const CMD_SPORK: &[u8;12] =     b"spork       ";
pub const CMD_MPROP: &[u8;12] =    b"mprop       ";
pub const CMD_MVOTE: &[u8;12] =    b"mvote       ";
pub const CMD_FBS: &[u8;12] =      b"fbs         ";
pub const CMD_FBVOTE: &[u8;12] =   b"fbvote      ";
pub const CMD_MNVS: &[u8;12] =     b"mnvs        ";
pub const CMD_IX: &[u8;12] =       b"ix          ";
pub const CMD_TXLVOTE: &[u8;12] =  b"txlvote     ";
pub const CMD_QFC: &[u8;12] =      b"qfcommit    ";
pub const CMD_DSEG: &[u8;12] =     b"dseg        ";
pub const CMD_SSC: &[u8;12] =      b"ssc         ";
pub const CMD_MNGET: &[u8;12] =    b"mnget       ";
pub const CMD_GETADDR: &[u8;12] =  b"getaddr     ";
pub const CMD_ADDR: &[u8;12] =     b"addr        ";
pub const CMD_MEMPOOL: &[u8;12] =  b"mempool     ";
pub const CMD_REJECT: &[u8;12] =   b"reject      ";
pub const CMD_NOTFOUND: &[u8;12]= b"notfound    ";
pub const CMD_INV: &[u8; 12] = b"inv\0\0\0\0\0\0\0\0\0";
pub const CMD_DSTX: &[u8;12] =     b"dstx        ";
pub const CMD_MNW: &[u8;12] =      b"mnw         ";
pub const MAX_HEADERS_PER_MSG: usize = 2000;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InventoryType { Error = 0, Tx = 1, Block = 2, FilteredBlock = 3, TxLockRequest = 4, TxLockVote = 5, Spork = 6, MasternodeWinner = 7, MasternodeScanningError = 8, BudgetVote = 9, BudgetProposal = 10, BudgetFinalized = 11, BudgetFinalizedVote = 12, MasternodeQuorum = 13, MasternodeAnnounce = 14, MasternodePing = 15, Dstx = 16, }
impl InventoryType { pub fn from_u32(val: u32) -> Option<Self> { match val { 0=>Some(Self::Error),1=>Some(Self::Tx),2=>Some(Self::Block),3=>Some(Self::FilteredBlock),4=>Some(Self::TxLockRequest),5=>Some(Self::TxLockVote),6=>Some(Self::Spork),7=>Some(Self::MasternodeWinner),8=>Some(Self::MasternodeScanningError),9=>Some(Self::BudgetVote),10=>Some(Self::BudgetProposal),11=>Some(Self::BudgetFinalized),12=>Some(Self::BudgetFinalizedVote),13=>Some(Self::MasternodeQuorum),14=>Some(Self::MasternodeAnnounce),15=>Some(Self::MasternodePing),16=>Some(Self::Dstx),_=>None,} } }
pub trait Encodable{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>;}
pub trait Decodable:Sized{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>;}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct MessageHeader{pub magic:[u8;4],pub command:[u8;12],pub length:u32,pub checksum:[u8;4]}
impl MessageHeader{pub const SIZE:usize=24;pub fn new(cmd:[u8;12],len:u32,chk:[u8;4])->Self{Self{magic:MAINNET_MAGIC,command:cmd,length:len,checksum:chk}}}
impl Encodable for MessageHeader{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{w.write_all(&self.magic)?;w.write_all(&self.command)?;w.write_u32::<LittleEndian>(self.length)?;w.write_all(&self.checksum)?;Ok(Self::SIZE)}}
impl Decodable for MessageHeader{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let mut mgc=[0u8;4];r.read_exact(&mut mgc)?;let mut cmd=[0u8;12];r.read_exact(&mut cmd)?;let len=r.read_u32::<LittleEndian>()?;let mut chk=[0u8;4];r.read_exact(&mut chk)?;Ok(Self{magic:mgc,command:cmd,length:len,checksum:chk})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct NetAddr{pub services:u64,pub ip:[u8;16],pub port:u16}
impl NetAddr{pub fn new(ip:std::net::IpAddr,p:u16,s:u64)->Self{let ipb=match ip{std::net::IpAddr::V4(v4)=>v4.to_ipv6_mapped().octets(),std::net::IpAddr::V6(v6)=>v6.octets()};Self{services:s,ip:ipb,port:p.to_be()}}}
impl Encodable for NetAddr{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{w.write_u64::<LittleEndian>(self.services)?;w.write_all(&self.ip)?;w.write_u16::<BigEndian>(self.port)?;Ok(26)}}
impl Decodable for NetAddr{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let s=r.read_u64::<LittleEndian>()?;let mut ip=[0u8;16];r.read_exact(&mut ip)?;let p=r.read_u16::<BigEndian>()?;Ok(Self{services:s,ip,port:p})}}
fn write_var_int<W:Write+WriteBytesExt>(w:&mut W,n:u64)->Result<usize,IoError>{if n<0xfd{w.write_u8(n as u8)?;Ok(1)}else if n<=0xffff{w.write_u8(0xfd)?;w.write_u16::<LittleEndian>(n as u16)?;Ok(3)}else if n<=0xffffffff{w.write_u8(0xfe)?;w.write_u32::<LittleEndian>(n as u32)?;Ok(5)}else{w.write_u8(0xff)?;w.write_u64::<LittleEndian>(n)?;Ok(9)}}
pub fn read_var_int<R:Read+ReadBytesExt>(r:&mut R)->Result<u64,IoError>{match r.read_u8()?{0xff=>r.read_u64::<LittleEndian>(),0xfe=>r.read_u32::<LittleEndian>().map(|x|x as u64),0xfd=>r.read_u16::<LittleEndian>().map(|x|x as u64),n=>Ok(n as u64)}}
fn write_var_string_bytes<W:Write+WriteBytesExt>(w:&mut W,b:&[u8])->Result<usize,IoError>{let mut l=write_var_int(w,b.len()as u64)?;w.write_all(b)?;l+=b.len();Ok(l)}
fn read_var_string_bytes<R:Read+ReadBytesExt>(r:&mut R)->Result<Vec<u8>,IoError>{let l=read_var_int(r)?;if l>2*1024*1024{Err(IoError::new(IoErrorKind::InvalidData,"VarBytes too long"))}else{let mut buf=vec![0;l as usize];if l>0{r.read_exact(&mut buf)?;}Ok(buf)}}
fn write_var_string<W:Write+WriteBytesExt>(w:&mut W,s:&str)->Result<usize,IoError>{write_var_string_bytes(w, s.as_bytes())}
fn read_var_string<R:Read+ReadBytesExt>(r:&mut R)->Result<String,IoError>{let bytes=read_var_string_bytes(r)?;String::from_utf8(bytes).map_err(|_|IoError::new(IoErrorKind::InvalidData,"Invalid UTF-8 in VarString"))}
#[derive(Debug,Clone, Serialize)]
pub struct VersionMessage{pub version:i32,pub services:u64,pub timestamp:i64,pub addr_recv:NetAddr,pub addr_from:NetAddr,pub nonce:u64,pub user_agent:String,pub start_height:i32}
impl Encodable for VersionMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;w.write_i32::<LittleEndian>(self.version)?;wr+=4;w.write_u64::<LittleEndian>(self.services)?;wr+=8;w.write_i64::<LittleEndian>(self.timestamp)?;wr+=8;wr+=self.addr_recv.consensus_encode(w)?;wr+=self.addr_from.consensus_encode(w)?;w.write_u64::<LittleEndian>(self.nonce)?;wr+=8;wr+=write_var_string(w,&self.user_agent)?;w.write_i32::<LittleEndian>(self.start_height)?;wr+=4;Ok(wr)}}
impl Decodable for VersionMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{Ok(Self{version:r.read_i32::<LittleEndian>()?,services:r.read_u64::<LittleEndian>()?,timestamp:r.read_i64::<LittleEndian>()?,addr_recv:NetAddr::consensus_decode(r)?,addr_from:NetAddr::consensus_decode(r)?,nonce:r.read_u64::<LittleEndian>()?,user_agent:read_var_string(r)?,start_height:r.read_i32::<LittleEndian>()?})}}
impl VersionMessage{#[allow(clippy::too_many_arguments)]pub fn new(s:u64,ar:NetAddr,af:NetAddr,n:u64,ua:String,sh:i32)->Self{Self{version:PROTOCOL_VERSION,services:s,timestamp:chrono::Utc::now().timestamp(),addr_recv:ar,addr_from:af,nonce:n,user_agent:ua,start_height:sh}}pub fn default_for_peer(pip:std::net::IpAddr,pp:u16,sh:i32,os:u64,nn:u64)->Self{let lip=std::net::IpAddr::V4(Ipv4Addr::new(0,0,0,0));let lp=0;Self::new(os,NetAddr::new(pip,pp,NODE_NETWORK),NetAddr::new(lip,lp,os),nn,format!("/TWINS-Rust:{}/FastSync:1/",env!("CARGO_PKG_VERSION")),sh)}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct VerackMessage;
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct PingMessage{pub nonce:u64}
impl PingMessage{pub fn new(n:u64)->Self{Self{nonce:n}}}
impl Encodable for PingMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{w.write_u64::<LittleEndian>(self.nonce)?;Ok(8)}}
impl Decodable for PingMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{Ok(Self{nonce:r.read_u64::<LittleEndian>()?})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct PongMessage{pub nonce:u64}
impl PongMessage{pub fn new(n:u64)->Self{Self{nonce:n}}}
impl Encodable for PongMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{w.write_u64::<LittleEndian>(self.nonce)?;Ok(8)}}
impl Decodable for PongMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{Ok(Self{nonce:r.read_u64::<LittleEndian>()?})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct GetHeadersMessage{pub version:i32,pub block_locator_hashes:Vec<[u8;32]>,pub hash_stop:[u8;32]}
impl Encodable for GetHeadersMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;w.write_i32::<LittleEndian>(self.version)?;wr+=4;wr+=write_var_int(w,self.block_locator_hashes.len()as u64)?;for h in &self.block_locator_hashes{w.write_all(h)?;wr+=32;}w.write_all(&self.hash_stop)?;wr+=32;Ok(wr)}}
impl Decodable for GetHeadersMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let v=r.read_i32::<LittleEndian>()?;let c=read_var_int(r)?;if c > 2000 { return Err(IoError::new(IoErrorKind::InvalidData, "GetHeaders block_locator_hashes count too large")); } let mut blh=Vec::with_capacity(c as usize);for _ in 0..c{let mut h=[0u8;32];r.read_exact(&mut h)?;blh.push(h);}let mut hs=[0u8;32];r.read_exact(&mut hs)?;Ok(Self{version:v,block_locator_hashes:blh,hash_stop:hs})}}

#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct GetBlocksMessage{pub version:i32,pub block_locator_hashes:Vec<[u8;32]>,pub hash_stop:[u8;32]}
impl Encodable for GetBlocksMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;w.write_i32::<LittleEndian>(self.version)?;wr+=4;wr+=write_var_int(w,self.block_locator_hashes.len()as u64)?;for h in &self.block_locator_hashes{w.write_all(h)?;wr+=32;}w.write_all(&self.hash_stop)?;wr+=32;Ok(wr)}}
impl Decodable for GetBlocksMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let v=r.read_i32::<LittleEndian>()?;let c=read_var_int(r)?;if c > 2000 { return Err(IoError::new(IoErrorKind::InvalidData, "GetBlocks block_locator_hashes count too large")); } let mut blh=Vec::with_capacity(c as usize);for _ in 0..c{let mut h=[0u8;32];r.read_exact(&mut h)?;blh.push(h);}let mut hs=[0u8;32];r.read_exact(&mut hs)?;Ok(Self{version:v,block_locator_hashes:blh,hash_stop:hs})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct BlockHeaderData{pub version:i32,pub prev_block_hash:[u8;32],pub merkle_root:[u8;32],pub timestamp:u32,pub bits:u32,pub nonce:u32,pub accumulator_checkpoint:Option<[u8;32]>}
impl BlockHeaderData{pub fn get_hash(&self)->[u8;32]{let mut cpp_msg=Vec::new();cpp_msg.write_i32::<LittleEndian>(self.version).unwrap();cpp_msg.write_all(&self.prev_block_hash).unwrap();cpp_msg.write_all(&self.merkle_root).unwrap();cpp_msg.write_u32::<LittleEndian>(self.timestamp).unwrap();cpp_msg.write_u32::<LittleEndian>(self.bits).unwrap();cpp_msg.write_u32::<LittleEndian>(self.nonce).unwrap();if self.version>3{if let Some(ch)=&self.accumulator_checkpoint{cpp_msg.write_all(ch).unwrap();}}let h1=Sha256::digest(&cpp_msg);let h2=Sha256::digest(&h1);let mut hb=[0u8;32];hb.copy_from_slice(&h2);hb}}
impl Encodable for BlockHeaderData{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;w.write_i32::<LittleEndian>(self.version)?;wr+=4;w.write_all(&self.prev_block_hash)?;wr+=32;w.write_all(&self.merkle_root)?;wr+=32;w.write_u32::<LittleEndian>(self.timestamp)?;wr+=4;w.write_u32::<LittleEndian>(self.bits)?;wr+=4;w.write_u32::<LittleEndian>(self.nonce)?;wr+=4;if self.version>3{if let Some(ch)=&self.accumulator_checkpoint{w.write_all(ch)?;wr+=32;}else{return Err(IoError::new(IoErrorKind::InvalidInput,"AccOpt missing for v>3 P2P enc"));}}else{if self.accumulator_checkpoint.is_some(){return Err(IoError::new(IoErrorKind::InvalidInput,"AccOpt present for v<=3 P2P enc"));}}Ok(wr)}}
impl Decodable for BlockHeaderData{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let v=r.read_i32::<LittleEndian>()?;let mut pbh=[0u8;32];r.read_exact(&mut pbh)?;let mut mr=[0u8;32];r.read_exact(&mut mr)?;let t=r.read_u32::<LittleEndian>()?;let b=r.read_u32::<LittleEndian>()?;let n=r.read_u32::<LittleEndian>()?;let ac=if v>3{let mut ach=[0u8;32];r.read_exact(&mut ach)?;Some(ach)}else{None};Ok(Self{version:v,prev_block_hash:pbh,merkle_root:mr,timestamp:t,bits:b,nonce:n,accumulator_checkpoint:ac})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct HeadersMessage{pub headers:Vec<BlockHeaderData>}
impl Encodable for HeadersMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;wr+=write_var_int(w,self.headers.len()as u64)?;for h in &self.headers{wr+=h.consensus_encode(w)?;wr+=write_var_int(w,0)?;}Ok(wr)}}
impl Decodable for HeadersMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let c=read_var_int(r)?;if c>MAX_HEADERS_PER_MSG as u64{return Err(IoError::new(IoErrorKind::InvalidData,format!("HdrsMsg cnt {} > max {}",c,MAX_HEADERS_PER_MSG)));}let mut hds=Vec::with_capacity(c as usize);for _ in 0..c{hds.push(BlockHeaderData::consensus_decode(r)?);if read_var_int(r)?!=0{return Err(IoError::new(IoErrorKind::InvalidData,"HdrsMsg item non-zero txn_cnt"));}}Ok(Self{headers:hds})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct InventoryVector{pub inv_type:InventoryType,pub hash:[u8;32]}
impl Encodable for InventoryVector{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{w.write_u32::<LittleEndian>(self.inv_type as u32)?;w.write_all(&self.hash)?;Ok(36)}}
impl Decodable for InventoryVector{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let tv=r.read_u32::<LittleEndian>()?;let it=InventoryType::from_u32(tv).ok_or_else(||IoError::new(IoErrorKind::InvalidData,format!("Unk inv type: {}",tv)))?;let mut h=[0u8;32];r.read_exact(&mut h)?;Ok(Self{inv_type:it,hash:h})}}
#[derive(Debug,Clone,PartialEq,Eq, Serialize)]
pub struct GetDataMessage{pub inventory:Vec<InventoryVector>}
impl Encodable for GetDataMessage{fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=write_var_int(w,self.inventory.len()as u64)?;for i in &self.inventory{wr+=i.consensus_encode(w)?;}Ok(wr)}}
impl Decodable for GetDataMessage{fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let c=read_var_int(r)?;if c>50_000{return Err(IoError::new(IoErrorKind::InvalidData,"GetData inv cnt too large"));}let mut inv=Vec::with_capacity(c as usize);for _ in 0..c{inv.push(InventoryVector::consensus_decode(r)?);}Ok(Self{inventory:inv})}}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TxInRaw {
    pub prev_out_hash: [u8; 32],
    pub prev_out_n: u32,
    pub script_sig: Vec<u8>,
    pub sequence: u32,
}
impl Encodable for TxInRaw {fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {let mut written = 0;writer.write_all(&self.prev_out_hash)?; written += 32;writer.write_u32::<LittleEndian>(self.prev_out_n)?; written += 4;written += write_var_string_bytes(writer, &self.script_sig)?;writer.write_u32::<LittleEndian>(self.sequence)?; written += 4;Ok(written)}}
impl Decodable for TxInRaw {fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {let mut prev_out_hash = [0u8; 32];reader.read_exact(&mut prev_out_hash)?;let prev_out_n = reader.read_u32::<LittleEndian>()?;let script_sig = read_var_string_bytes(reader)?;let sequence = reader.read_u32::<LittleEndian>()?;Ok(TxInRaw { prev_out_hash, prev_out_n, script_sig, sequence })}}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TxOutRaw { pub value: i64, pub script_pubkey: Vec<u8>,}
impl Encodable for TxOutRaw {fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {let mut written = 0;writer.write_i64::<LittleEndian>(self.value)?; written += 8;written += write_var_string_bytes(writer, &self.script_pubkey)?;Ok(written)}}
impl Decodable for TxOutRaw {fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {let value = reader.read_i64::<LittleEndian>()?;let script_pubkey = read_var_string_bytes(reader)?;Ok(TxOutRaw { value, script_pubkey })}}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionData { pub version: i32, pub vin: Vec<TxInRaw>, pub vout: Vec<TxOutRaw>, pub lock_time: u32,}
impl TransactionData {
    pub fn is_coinstake(&self) -> bool { if self.vin.is_empty()||self.vout.len()<2{return false;} self.vout[0].value==0&&self.vout[0].script_pubkey.is_empty() }
    pub fn get_txid(&self) -> [u8; 32] { let mut w=Cursor::new(Vec::new());self.consensus_encode(&mut w).expect("Tx encode failed");let b=w.into_inner();let h1=Sha256::digest(&b);let h2=Sha256::digest(&h1);let mut tid=[0u8;32];tid.copy_from_slice(&h2);tid }
}
impl Encodable for TransactionData {fn consensus_encode<W:Write+WriteBytesExt>(&self,w:&mut W)->Result<usize,IoError>{let mut wr=0;w.write_i32::<LittleEndian>(self.version)?;wr+=4;wr+=write_var_int(w,self.vin.len()as u64)?;for txin in &self.vin{wr+=txin.consensus_encode(w)?;}wr+=write_var_int(w,self.vout.len()as u64)?;for txout in &self.vout{wr+=txout.consensus_encode(w)?;}w.write_u32::<LittleEndian>(self.lock_time)?;wr+=4;Ok(wr)}}
impl Decodable for TransactionData {fn consensus_decode<R:Read+ReadBytesExt>(r:&mut R)->Result<Self,IoError>{let ver=r.read_i32::<LittleEndian>()?;let vc=read_var_int(r)?;if vc>100_000{return Err(IoError::new(IoErrorKind::InvalidData,"tx vin cnt>max"));}let mut v_in=Vec::with_capacity(vc as usize);for _ in 0..vc{v_in.push(TxInRaw::consensus_decode(r)?);}let voc=read_var_int(r)?;if voc>100_000{return Err(IoError::new(IoErrorKind::InvalidData,"tx vout cnt>max"));}let mut v_out=Vec::with_capacity(voc as usize);for _ in 0..voc{v_out.push(TxOutRaw::consensus_decode(r)?);}let lt=r.read_u32::<LittleEndian>()?;Ok(Self{version:ver,vin:v_in,vout:v_out,lock_time:lt})}}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)] 
pub struct BlockMessage { pub header: BlockHeaderData, pub transactions: Vec<TransactionData>, pub block_sig: Option<Vec<u8>>, }
impl BlockMessage {
    pub fn is_likely_proof_of_stake(&self) -> bool {
        if self.transactions.len() > 1 {
            return self.transactions[1].is_coinstake();
        }
        false
    }
    pub fn calculate_merkle_root(&self) -> [u8; 32] {
        if self.transactions.is_empty() { return [0u8; 32]; }
        let txids: Vec<[u8; 32]> = self.transactions.iter().map(|tx| tx.get_txid()).collect();
        if txids.is_empty() { return [0u8; 32]; }
        let mut merkle_tree = txids;
        while merkle_tree.len() > 1 {
            if merkle_tree.len() % 2 != 0 {
                if let Some(last_hash) = merkle_tree.last().cloned() { merkle_tree.push(last_hash); }
            }
            merkle_tree = merkle_tree.chunks_exact(2).map(|pair| {
                let mut concat = Vec::with_capacity(64); concat.extend_from_slice(&pair[0]); concat.extend_from_slice(&pair[1]);
                let hash1 = Sha256::digest(&concat);
                let hash2 = Sha256::digest(&hash1);
                let mut result_hash = [0u8; 32]; result_hash.copy_from_slice(&hash2); result_hash
            }).collect();
        }
        merkle_tree.get(0).cloned().unwrap_or([0u8; 32])
    }
    pub fn verify_pos_signature(&self) -> Result<bool, secp256k1::Error> {
        if !self.is_likely_proof_of_stake() { return Ok(true); }
        match &self.block_sig {
            Some(sig_bytes) => {
                let block_header_hash = self.header.get_hash();
                log::debug!("Attempting PoS block signature verification for block: {}", hex::encode(block_header_hash));
                let staker_pubkey_bytes_opt: Option<Vec<u8>> = 
                    if self.transactions.len() > 1 && self.transactions[1].vout.len() > 1 {
                        let coinstake_tx = &self.transactions[1];
                        let script_pub_key_staker = &coinstake_tx.vout[1].script_pubkey;
                        log::debug!("  PoS Sig Check: Coinstake vout[1] scriptPubKey (len {}): {}", script_pub_key_staker.len(), hex::encode(script_pub_key_staker));
                        if script_pub_key_staker.len() == 35 && script_pub_key_staker[0] == 0x21 && script_pub_key_staker[34] == 0xac { 
                            Some(script_pub_key_staker[1..34].to_vec())
                        } else if script_pub_key_staker.len() == 67 && script_pub_key_staker[0] == 0x41 && script_pub_key_staker[66] == 0xac { 
                            Some(script_pub_key_staker[1..66].to_vec())
                        } else {
                            log::warn!("  PoS Sig Check: Coinstake vout[1] scriptPubKey for block {} not P2PK.", hex::encode(block_header_hash));
                            None
                        }
                    } else {
                        log::warn!("  PoS Sig Check: Coinstake tx or vout[1] not found for pubkey extraction in block {}.", hex::encode(block_header_hash));
                        None
                    };
                if staker_pubkey_bytes_opt.is_none() {
                    log::warn!("PoS Sig Verify: Could not extract staker public key for block {}. Verification failed.", hex::encode(block_header_hash));
                    return Ok(false); 
                }
                let staker_pubkey_bytes = staker_pubkey_bytes_opt.unwrap();
                let secp = Secp256k1::verification_only();
                let message = SecpMessage::from_slice(&block_header_hash)?;
                let signature = Signature::from_der(sig_bytes)?;
                let public_key = PublicKey::from_slice(&staker_pubkey_bytes)?;
                match secp.verify_ecdsa(&message, &signature, &public_key) {
                    Ok(_) => { log::info!("PoS Block {} sig VERIFIED.", hex::encode(block_header_hash)); Ok(true) }
                    Err(e) => { log::warn!("PoS Block {} sig INVALID: {}", hex::encode(block_header_hash), e); Ok(false) }
                }
            }
            None => { log::warn!("PoS Block {} expected signature but none found.", hex::encode(self.header.get_hash())); Ok(false) }
        }
    }
}
impl Decodable for BlockMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let header = BlockHeaderData::consensus_decode(reader)?;
        let tx_count = read_var_int(reader)?;
        if tx_count==0||tx_count>20_000{return Err(IoError::new(IoErrorKind::InvalidData,"Block tx_count OOB"));}
        let mut transactions = Vec::with_capacity(tx_count as usize);
        for _ in 0..tx_count { transactions.push(TransactionData::consensus_decode(reader)?); }
        let mut block_sig = None;
        if transactions.len() > 1 && transactions[1].is_coinstake() {
            match read_var_string_bytes(reader) {
                Ok(sig) if !sig.is_empty() => block_sig = Some(sig),
                Ok(_) => {},
                Err(e) if e.kind() == IoErrorKind::UnexpectedEof => { log::warn!("PoS block {} missing sig (EOF). Header v{}", hex::encode(header.get_hash()), header.version); }
                Err(e) => return Err(e),
            }
        }
        Ok(BlockMessage { header, transactions, block_sig })
    }
}
 
impl Encodable for BlockMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.header.consensus_encode(writer)?;
        written += write_var_int(writer, self.transactions.len() as u64)?;
        for tx in &self.transactions {
            written += tx.consensus_encode(writer)?;
        }
        if self.is_likely_proof_of_stake() { 
            if let Some(ref sig) = self.block_sig {
                written += write_var_string_bytes(writer, sig)?;
            } else {
                written += write_var_int(writer, 0)?; 
            }
        } 
        Ok(written)
    }
}
  
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MasternodePing {
    pub vin: TxInRaw,
    pub block_hash: [u8; 32],
    pub sig_time: i64,
    pub vch_sig: Vec<u8>,
}
impl Encodable for MasternodePing {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.vin.consensus_encode(writer)?;
        writer.write_all(&self.block_hash)?;
        written += 32;
        writer.write_i64::<LittleEndian>(self.sig_time)?;
        written += 8;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        Ok(written)
    }
}
impl Decodable for MasternodePing {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin = TxInRaw::consensus_decode(reader)?;
        let mut block_hash = [0u8; 32];
        reader.read_exact(&mut block_hash)?;
        let sig_time = reader.read_i64::<LittleEndian>()?;
        let vch_sig = read_var_string_bytes(reader)?;
        Ok(MasternodePing {
            vin,
            block_hash,
            sig_time,
            vch_sig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MasternodeBroadcast {
    pub vin: TxInRaw,
    pub addr: NetAddr,
    pub pubkey_collateral_address: Vec<u8>,
    pub pubkey_masternode: Vec<u8>,
    pub sig: Vec<u8>,
    pub sig_time: i64,
    pub protocol_version: i32,
    pub last_ping: MasternodePing,
    pub n_last_dsq: i64,
}
impl Encodable for MasternodeBroadcast {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.vin.consensus_encode(writer)?;
        written += self.addr.consensus_encode(writer)?;
        written += write_var_string_bytes(writer, &self.pubkey_collateral_address)?;
        written += write_var_string_bytes(writer, &self.pubkey_masternode)?;
        written += write_var_string_bytes(writer, &self.sig)?;
        writer.write_i64::<LittleEndian>(self.sig_time)?;
        written += 8;
        writer.write_i32::<LittleEndian>(self.protocol_version)?;
        written += 4;
        written += self.last_ping.consensus_encode(writer)?;
        writer.write_i64::<LittleEndian>(self.n_last_dsq)?;
        written += 8;
        Ok(written)
    }
}
impl Decodable for MasternodeBroadcast {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin = TxInRaw::consensus_decode(reader)?;
        let addr = NetAddr::consensus_decode(reader)?;
        let pubkey_collateral_address = read_var_string_bytes(reader)?;
        let pubkey_masternode = read_var_string_bytes(reader)?;
        let sig = read_var_string_bytes(reader)?;
        let sig_time = reader.read_i64::<LittleEndian>()?;
        let protocol_version = reader.read_i32::<LittleEndian>()?;
        let last_ping = MasternodePing::consensus_decode(reader)?;
        let n_last_dsq = reader.read_i64::<LittleEndian>()?;
        Ok(MasternodeBroadcast {
            vin,
            addr,
            pubkey_collateral_address,
            pubkey_masternode,
            sig,
            sig_time,
            protocol_version,
            last_ping,
            n_last_dsq,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SporkMessage {
    pub n_spork_id: i32,
    pub n_value: i64,
    pub n_time_signed: i64,
    pub vch_sig: Vec<u8>,
}
impl Encodable for SporkMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        writer.write_i32::<LittleEndian>(self.n_spork_id)?;
        written += 4;
        writer.write_i64::<LittleEndian>(self.n_value)?;
        written += 8;
        writer.write_i64::<LittleEndian>(self.n_time_signed)?;
        written += 8;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        Ok(written)
    }
}
impl Decodable for SporkMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let n_spork_id = reader.read_i32::<LittleEndian>()?;
        let n_value = reader.read_i64::<LittleEndian>()?;
        let n_time_signed = reader.read_i64::<LittleEndian>()?;
        let vch_sig = read_var_string_bytes(reader)?;
        Ok(SporkMessage {
            n_spork_id,
            n_value,
            n_time_signed,
            vch_sig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetVote {
    pub vin: TxInRaw,
    pub n_proposal_hash: [u8; 32],
    pub n_vote: i32,
    pub n_time: i64,
    pub vch_sig: Vec<u8>,
}
impl Encodable for BudgetVote {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.vin.consensus_encode(writer)?;
        writer.write_all(&self.n_proposal_hash)?;
        written += 32;
        writer.write_i32::<LittleEndian>(self.n_vote)?;
        written += 4;
        writer.write_i64::<LittleEndian>(self.n_time)?;
        written += 8;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        Ok(written)
    }
}
impl Decodable for BudgetVote {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin = TxInRaw::consensus_decode(reader)?;
        let mut n_proposal_hash = [0u8; 32];
        reader.read_exact(&mut n_proposal_hash)?;
        let n_vote = reader.read_i32::<LittleEndian>()?;
        let n_time = reader.read_i64::<LittleEndian>()?;
        let vch_sig = read_var_string_bytes(reader)?;
        Ok(BudgetVote {
            vin,
            n_proposal_hash,
            n_vote,
            n_time,
            vch_sig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FinalizedBudgetVote {
    pub vin: TxInRaw,
    pub n_budget_hash: [u8; 32],
    pub n_time: i64,
    pub vch_sig: Vec<u8>,
}
impl Encodable for FinalizedBudgetVote {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.vin.consensus_encode(writer)?;
        writer.write_all(&self.n_budget_hash)?;
        written += 32;
        writer.write_i64::<LittleEndian>(self.n_time)?;
        written += 8;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        Ok(written)
    }
}
impl Decodable for FinalizedBudgetVote {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin = TxInRaw::consensus_decode(reader)?;
        let mut n_budget_hash = [0u8; 32];
        reader.read_exact(&mut n_budget_hash)?;
        let n_time = reader.read_i64::<LittleEndian>()?;
        let vch_sig = read_var_string_bytes(reader)?;
        Ok(FinalizedBudgetVote {
            vin,
            n_budget_hash,
            n_time,
            vch_sig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BudgetProposalBroadcast {
    pub str_proposal_name: String,
    pub str_url: String,
    pub n_time: i64,
    pub n_block_start: i32,
    pub n_block_end: i32,
    pub n_amount: i64,
    pub address_script: Vec<u8>,
    pub n_fee_tx_hash: [u8; 32],
}
impl Encodable for BudgetProposalBroadcast {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += write_var_string(writer, &self.str_proposal_name)?;
        written += write_var_string(writer, &self.str_url)?;
        writer.write_i64::<LittleEndian>(self.n_time)?;
        written += 8;
        writer.write_i32::<LittleEndian>(self.n_block_start)?;
        written += 4;
        writer.write_i32::<LittleEndian>(self.n_block_end)?;
        written += 4;
        writer.write_i64::<LittleEndian>(self.n_amount)?;
        written += 8;
        written += write_var_string_bytes(writer, &self.address_script)?;
        writer.write_all(&self.n_fee_tx_hash)?;
        written += 32;
        Ok(written)
    }
}
impl Decodable for BudgetProposalBroadcast {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let str_proposal_name = read_var_string(reader)?;
        if str_proposal_name.len() > 20 { return Err(IoError::new(IoErrorKind::InvalidData, "Proposal name too long")); }
        let str_url = read_var_string(reader)?;
        if str_url.len() > 64 { return Err(IoError::new(IoErrorKind::InvalidData, "Proposal URL too long")); }
        let n_time = reader.read_i64::<LittleEndian>()?;
        let n_block_start = reader.read_i32::<LittleEndian>()?;
        let n_block_end = reader.read_i32::<LittleEndian>()?;
        let n_amount = reader.read_i64::<LittleEndian>()?;
        let address_script = read_var_string_bytes(reader)?;
        let mut n_fee_tx_hash = [0u8; 32];
        reader.read_exact(&mut n_fee_tx_hash)?;
        Ok(BudgetProposalBroadcast {
            str_proposal_name,
            str_url,
            n_time,
            n_block_start,
            n_block_end,
            n_amount,
            address_script,
            n_fee_tx_hash,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TxBudgetPayment {
    pub n_proposal_hash: [u8; 32],
    pub payee_script: Vec<u8>,
    pub n_amount: i64,
}
impl Encodable for TxBudgetPayment {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        writer.write_all(&self.n_proposal_hash)?;
        written += 32;
        written += write_var_string_bytes(writer, &self.payee_script)?;
        writer.write_i64::<LittleEndian>(self.n_amount)?;
        written += 8;
        Ok(written)
    }
}
impl Decodable for TxBudgetPayment {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let mut n_proposal_hash = [0u8; 32];
        reader.read_exact(&mut n_proposal_hash)?;
        let payee_script = read_var_string_bytes(reader)?;
        let n_amount = reader.read_i64::<LittleEndian>()?;
        Ok(TxBudgetPayment {
            n_proposal_hash,
            payee_script,
            n_amount,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FinalizedBudgetBroadcast {
    pub str_budget_name: String,
    pub n_block_start: i32,
    pub vec_budget_payments: Vec<TxBudgetPayment>,
    pub n_fee_tx_hash: [u8; 32],
}
impl Encodable for FinalizedBudgetBroadcast {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += write_var_string(writer, &self.str_budget_name)?;
        writer.write_i32::<LittleEndian>(self.n_block_start)?;
        written += 4;
        written += write_var_int(writer, self.vec_budget_payments.len() as u64)?;
        for payment in &self.vec_budget_payments {
            written += payment.consensus_encode(writer)?;
        }
        writer.write_all(&self.n_fee_tx_hash)?;
        written += 32;
        Ok(written)
    }
}
impl Decodable for FinalizedBudgetBroadcast {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let str_budget_name = read_var_string(reader)?;
        if str_budget_name.len() > 20 { return Err(IoError::new(IoErrorKind::InvalidData, "Budget name too long")); }
        let n_block_start = reader.read_i32::<LittleEndian>()?;
        let payment_count = read_var_int(reader)?;
        if payment_count > 1000 {
            return Err(IoError::new(IoErrorKind::InvalidData, "Too many budget payments"));
        }
        let mut vec_budget_payments = Vec::with_capacity(payment_count as usize);
        for _ in 0..payment_count {
            vec_budget_payments.push(TxBudgetPayment::consensus_decode(reader)?);
        }
        let mut n_fee_tx_hash = [0u8; 32];
        reader.read_exact(&mut n_fee_tx_hash)?;
        Ok(FinalizedBudgetBroadcast {
            str_budget_name,
            n_block_start,
            vec_budget_payments,
            n_fee_tx_hash,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsensusVote {
    pub tx_hash: [u8; 32],
    pub vin_masternode: TxInRaw,
    pub vch_masternode_signature: Vec<u8>,
    pub n_block_height: i32,
}
impl Encodable for ConsensusVote {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        writer.write_all(&self.tx_hash)?;
        written += 32;
        written += self.vin_masternode.consensus_encode(writer)?;
        written += write_var_string_bytes(writer, &self.vch_masternode_signature)?;
        writer.write_i32::<LittleEndian>(self.n_block_height)?;
        written += 4;
        Ok(written)
    }
}
impl Decodable for ConsensusVote {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let mut tx_hash = [0u8; 32];
        reader.read_exact(&mut tx_hash)?;
        let vin_masternode = TxInRaw::consensus_decode(reader)?;
        let vch_masternode_signature = read_var_string_bytes(reader)?;
        let n_block_height = reader.read_i32::<LittleEndian>()?;
        Ok(ConsensusVote {
            tx_hash,
            vin_masternode,
            vch_masternode_signature,
            n_block_height,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TxLockRequest {
    pub tx: TransactionData,
}
impl Encodable for TxLockRequest {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        self.tx.consensus_encode(writer)
    }
}
impl Decodable for TxLockRequest {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let tx = TransactionData::consensus_decode(reader)?;
        Ok(TxLockRequest { tx })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DsegMessage {
    pub vin: TxInRaw,
}
impl Encodable for DsegMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        self.vin.consensus_encode(writer)
    }
}
impl Decodable for DsegMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin = TxInRaw::consensus_decode(reader)?;
        Ok(DsegMessage { vin })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SscMessage {
    pub item_id: i32,
    pub count: i32,
}
impl Encodable for SscMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        writer.write_i32::<LittleEndian>(self.item_id)?;
        writer.write_i32::<LittleEndian>(self.count)?;
        Ok(8)
    }
}
impl Decodable for SscMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let item_id = reader.read_i32::<LittleEndian>()?;
        let count = reader.read_i32::<LittleEndian>()?;
        Ok(SscMessage { item_id, count })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MnGetMessage {
    pub count: i32,
}
impl Encodable for MnGetMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        writer.write_i32::<LittleEndian>(self.count)?;
        Ok(4)
    }
}
impl Decodable for MnGetMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let count = reader.read_i32::<LittleEndian>()?;
        Ok(MnGetMessage { count })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MnvsMessage {
    pub hash: [u8; 32],
}
impl Encodable for MnvsMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        writer.write_all(&self.hash)?;
        Ok(32)
    }
}
impl Decodable for MnvsMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let mut hash = [0u8; 32];
        reader.read_exact(&mut hash)?;
        Ok(MnvsMessage { hash })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DstxMessage {
    pub tx: TransactionData,
    pub vin_masternode: TxInRaw,
    pub vch_sig: Vec<u8>,
    pub sig_time: i64,
}
impl Encodable for DstxMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.tx.consensus_encode(writer)?;
        written += self.vin_masternode.consensus_encode(writer)?;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        writer.write_i64::<LittleEndian>(self.sig_time)?;
        written += 8;
        Ok(written)
    }
}
impl Decodable for DstxMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let tx = TransactionData::consensus_decode(reader)?;
        let vin_masternode = TxInRaw::consensus_decode(reader)?;
        let vch_sig = read_var_string_bytes(reader)?;
        let sig_time = reader.read_i64::<LittleEndian>()?;
        Ok(DstxMessage {
            tx,
            vin_masternode,
            vch_sig,
            sig_time,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MasternodeWinner {
    pub vin_masternode: TxInRaw,
    pub n_block_height: i32,
    pub payee_script: Vec<u8>,
    pub vch_sig: Vec<u8>,
}
impl Encodable for MasternodeWinner {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += self.vin_masternode.consensus_encode(writer)?;
        writer.write_i32::<LittleEndian>(self.n_block_height)?;
        written += 4;
        written += write_var_string_bytes(writer, &self.payee_script)?;
        written += write_var_string_bytes(writer, &self.vch_sig)?;
        Ok(written)
    }
}
impl Decodable for MasternodeWinner {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let vin_masternode = TxInRaw::consensus_decode(reader)?;
        let n_block_height = reader.read_i32::<LittleEndian>()?;
        let payee_script = read_var_string_bytes(reader)?;
        let vch_sig = read_var_string_bytes(reader)?;
        Ok(MasternodeWinner {
            vin_masternode,
            n_block_height,
            payee_script,
            vch_sig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddrMessage {
    pub addresses: Vec<NetAddr>,
}
impl Encodable for AddrMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = write_var_int(writer, self.addresses.len() as u64)?;
        for addr in &self.addresses {
            written += addr.consensus_encode(writer)?;
        }
        Ok(written)
    }
}
impl Decodable for AddrMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let count = read_var_int(reader)?;
        if count > 1000 {
            return Err(IoError::new(IoErrorKind::InvalidData, "Addr message has too many addresses"));
        }
        let mut addresses = Vec::with_capacity(count as usize);
        for _ in 0..count {
            addresses.push(NetAddr::consensus_decode(reader)?);
        }
        Ok(AddrMessage { addresses })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RejectMessage {
    pub message_cmd: String,
    pub code: u8,
    pub reason: String,
    pub data_hash: [u8; 32],
}
impl Encodable for RejectMessage {
    fn consensus_encode<W: Write + WriteBytesExt>(&self, writer: &mut W) -> Result<usize, IoError> {
        let mut written = 0;
        written += write_var_string(writer, &self.message_cmd)?;
        writer.write_u8(self.code)?;
        written += 1;
        written += write_var_string(writer, &self.reason)?;
        writer.write_all(&self.data_hash)?;
        written += 32;
        Ok(written)
    }
}
impl Decodable for RejectMessage {
    fn consensus_decode<R: Read + ReadBytesExt>(reader: &mut R) -> Result<Self, IoError> {
        let message_cmd = read_var_string(reader)?;
        let code = reader.read_u8()?;
        let reason = read_var_string(reader)?;
        let mut data_hash = [0u8; 32];
        reader.read_exact(&mut data_hash)?;
        Ok(RejectMessage {
            message_cmd,
            code,
            reason,
            data_hash,
        })
    }
} 