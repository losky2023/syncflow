# SyncFlow — 多端端到端文件同步系统设计

**日期**: 2026-04-20
**状态**: Approved for Implementation

---

## 1. 概述

SyncFlow 是一套个人文件同步软件，支持 Windows、macOS、iOS 多端之间的端到端文件同步。核心理念：文件只在设备间传输，服务器不存储任何文件内容。

### 目标

- 多平台支持：Windows、macOS、iOS（后续可扩展 Android）
- 端到端加密：文件在本地加密后再传输，服务器无法窥探
- 混合传输：局域网内 WebRTC P2P 直连，远程通过信令服务器中转
- 实时同步：文件系统监听 + 定时兜底
- 冲突手动解决：UI 提示用户选择保留版本

### 非目标

- 团队协作/多人共享（个人场景）
- 在线编辑/协同办公
- 大文件流媒体预览
- WebSocket 备份通道（本期 TODO，后续实现）

---

## 2. 技术栈

### 客户端（全平台统一）

- **框架**: Tauri 2.0（Rust 后端 + Web 前端）
- **UI 框架**: React + TypeScript + Vite
- **核心引擎**: Rust

### 信令服务器

- **语言**: Rust + axum + tokio
- **数据库**: SQLite（轻量部署）或 PostgreSQL（生产级）
- **STUN/TURN**: coturn 独立部署

### 关键依赖

| 模块 | Rust Crate | 用途 |
|------|-----------|------|
| 加密 | `libsodium` (sodiumoxide) | XChaCha20-Poly1305, Argon2id, Ed25519 |
| 文件监听 | `notify` | 跨平台文件系统事件监控 |
| WebRTC | `webrtc-rs` | P2P 数据通道 |
| 本地存储 | `sqlx` + SQLite | 元数据、同步状态、版本历史 |
| 网络通信 | `tokio-tungstenite` | WebSocket 信令连接 |
| 异步运行时 | `tokio` | 全平台异步支持 |

---

## 3. 系统架构

### 3.1 整体架构

```
                    ┌─────────────────────┐
                    │   信令服务器 (axum)   │
                    │  设备注册 | SDP 交换  │
                    │  STUN 配置分发        │
                    └──────────┬──────────┘
                               │ WebSocket
              ┌────────────────┼────────────────┐
              │                │                │
        ┌─────┴─────┐   ┌─────┴─────┐   ┌─────┴─────┐
        │  设备 A    │   │  设备 B    │   │  设备 C    │
        │ Windows PC │   │ iPhone    │   │ macOS     │
        │            │   │            │   │            │
        │ Tauri Core │   │ Tauri Core │   │ Tauri Core │
        │ WebRTC     │   │ WebRTC     │   │ WebRTC     │
        │ 加密引擎    │   │ 加密引擎    │   │ 加密引擎    │
        └────────────┘   └────────────┘   └────────────┘
              │                │                │
              └────────────────┼────────────────┘
                         WebRTC DataChannel
                         (局域网优先，失败则
                          走服务器中转)
```

### 3.2 传输策略

1. **局域网内**: mDNS 发现 → WebRTC P2P 直连
2. **远程**: 信令服务器交换 SDP + ICE → WebRTC（可能经 TURN 中转）
3. **P2P 失败**: 走信令服务器 WebSocket 内存转发（TODO）
4. **所有文件数据**: 全程 XChaCha20-Poly1305 加密

---

## 4. 客户端模块设计

### 4.1 crypto_engine

**职责**: 所有加密/解密操作

```rust
pub struct CryptoEngine {
    // 主密码派生的根密钥
    root_key: SecretBox<[u8; 32]>,
    // 文件加密密钥（每文件独立生成）
    file_key_cache: LruCache<FileHash, Vec<u8>>,
}

impl CryptoEngine {
    // 从主密码派生根密钥 (Argon2id)
    pub fn derive_root_key(password: &str, salt: &[u8]) -> Result<Self>;

    // 生成文件加密密钥并加密文件内容
    pub fn encrypt_file(&self, plaintext: &[u8]) -> Result<EncryptedBlob>;

    // 解密文件内容
    pub fn decrypt_file(&self, encrypted_blob: &EncryptedBlob) -> Result<Vec<u8>>;

    // 计算文件哈希 (BLAKE3)
    pub fn hash_file(&self, content: &[u8]) -> FileHash;

    // 生成设备签名密钥对 (Ed25519)
    pub fn generate_device_keypair(&self) -> Result<DeviceKeypair>;
}
```

**加密方案**:
- 每个文件独立生成随机 nonce + 对称密钥
- XChaCha20-Poly1305 加密文件内容
- Argon2id 从主密码派生根密钥
- Ed25519 用于设备身份签名
- BLAKE3 用于文件内容哈希

### 4.2 sync_engine

**职责**: 文件变更检测、同步队列、冲突处理

```rust
pub struct SyncEngine {
    crypto: Arc<CryptoEngine>,
    storage: Arc<StorageEngine>,
    transport: Arc<TransportLayer>,
    watcher: Option<DirectoryWatcher>,
    sync_queue: SyncQueue,
    version_vectors: HashMap<PathBuf, VersionVector>,
}

impl SyncEngine {
    // 启动文件夹监控
    pub fn start_watching(&mut self, paths: Vec<PathBuf>) -> Result<()>;

    // 处理文件变更事件
    async fn on_file_changed(&self, event: FileEvent) -> Result<()>;

    // 执行同步（发送本地变更 + 接收远端变更）
    pub async fn sync_with_peer(&self, peer_id: DeviceId) -> Result<SyncReport>;

    // 接收远端文件
    async fn receive_file(&self, metadata: FileMetadata, encrypted_data: Vec<u8>) -> Result<()>;

    // 冲突检测（基于版本向量）
    fn detect_conflict(&self, path: &PathBuf, incoming_version: &VersionVector) -> ConflictStatus;
}
```

**同步流程**:
1. `notify` 监听文件系统 → 生成 FileEvent
2. 计算文件 BLAKE3 哈希，比对元数据判断是否真变更
3. 生成同步任务入队
4. 通过 `transport_layer` 发送给对端
5. 对端接收 → 解密 → 写入文件系统
6. 冲突时标记冲突状态，等待用户选择

**版本向量**:
```rust
pub struct VersionVector {
    // device_id -> 该设备上的最新版本号
    versions: HashMap<DeviceId, u64>,
    timestamp: DateTime<Utc>,
}
```

### 4.3 transport_layer

**职责**: WebRTC 连接管理、数据传输

```rust
pub struct TransportLayer {
    signal_client: Arc<SignalClient>,
    // peer_id -> PeerConnection
    peers: HashMap<DeviceId, Arc<PeerConnection>>,
    data_channels: HashMap<DeviceId, Arc<DataChannel>>,
}

impl TransportLayer {
    // 初始化并连接信令服务器
    pub async fn connect(&self, signal_url: &str, token: &str) -> Result<()>;

    // 发起与指定设备的 WebRTC 连接
    pub async fn connect_to_peer(&self, peer_id: DeviceId) -> Result<()>;

    // 通过 DataChannel 发送文件
    pub async fn send_file(&self, peer_id: DeviceId, data: &[u8]) -> Result<()>;

    // 接收远端文件事件流
    pub fn file_events(&self) -> broadcast::Receiver<FileTransferEvent>;

    // 获取在线设备列表
    pub fn online_peers(&self) -> Vec<DeviceId>;
}
```

**WebRTC 连接流程**:
1. 连接信令服务器 WebSocket，发送 `device_online`
2. 收到对端上线通知后，创建 `RTCPeerConnection`
3. 创建 DataChannel，生成 SDP offer
4. 通过信令服务器交换 SDP offer/answer
5. 交换 ICE candidates
6. DataChannel 连通，开始传输

**数据传输协议**:
```
+----------------+----------------+----------------+----------------+
|  Message Type  |  Payload Size  |    Metadata    |  Encrypted     |
|  (1 byte)      |  (4 bytes)     |  (variable)    |  Data          |
+----------------+----------------+----------------+----------------+
```

Message Types:
- `0x01`: FileTransfer — 传输文件数据
- `0x02`: FileMetadata — 传输文件元数据
- `0x03`: FileDelete — 通知删除文件
- `0x04`: SyncRequest — 请求全量同步
- `0x05`: SyncComplete — 同步完成
- `0x06`: ConflictResolution — 冲突解决方案

### 4.4 storage_engine

**职责**: 本地元数据持久化

```rust
pub struct StorageEngine {
    db: SqlitePool,
}

impl StorageEngine {
    // 初始化数据库
    pub async fn init(db_path: &str) -> Result<Self>;

    // 保存/获取文件元数据
    pub async fn save_file_meta(&self, meta: &FileMetadata) -> Result<()>;
    pub async fn get_file_meta(&self, path: &PathBuf) -> Result<Option<FileMetadata>>;

    // 保存/获取同步状态
    pub async fn save_sync_state(&self, peer_id: DeviceId, state: &SyncState) -> Result<()>;
    pub async fn get_sync_state(&self, peer_id: DeviceId) -> Result<Option<SyncState>>;

    // 版本历史
    pub async fn save_version(&self, version: &FileVersion) -> Result<()>;
    pub async fn get_version_history(&self, path: &PathBuf) -> Result<Vec<FileVersion>>;

    // 设备信息
    pub async fn save_device_info(&self, info: &DeviceInfo) -> Result<()>;
    pub async fn get_known_devices(&self) -> Result<Vec<DeviceInfo>>;
}
```

**数据库表结构**:

```sql
-- 文件元数据
CREATE TABLE file_metadata (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT NOT NULL,           -- BLAKE3 哈希
    size BIGINT NOT NULL,
    modified_at TEXT NOT NULL,
    version_vector TEXT NOT NULL, -- JSON 格式的版本向量
    created_at TEXT NOT NULL
);

-- 同步状态
CREATE TABLE sync_state (
    id INTEGER PRIMARY KEY,
    peer_id TEXT NOT NULL,
    last_sync_at TEXT,
    sync_status TEXT NOT NULL,    -- idle, syncing, conflict, error
    pending_changes INTEGER DEFAULT 0
);

-- 版本历史
CREATE TABLE file_versions (
    id INTEGER PRIMARY KEY,
    file_path TEXT NOT NULL,
    hash TEXT NOT NULL,
    version_vector TEXT NOT NULL,
    device_id TEXT NOT NULL,
    is_conflict BOOLEAN DEFAULT FALSE,
    created_at TEXT NOT NULL
);

-- 已知设备
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,
    device_id TEXT UNIQUE NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,       -- windows, macos, ios
    public_key TEXT NOT NULL,     -- Ed25519 公钥
    last_seen_at TEXT
);
```

### 4.5 auth_manager

**职责**: 用户认证、设备注册、会话管理

```rust
pub struct AuthManager {
    crypto: Arc<CryptoEngine>,
    storage: Arc<StorageEngine>,
    current_session: Option<UserSession>,
}

pub struct UserSession {
    user_id: String,
    device_id: String,
    auth_token: String,
    root_key: SecretBox<[u8; 32]>,
}

impl AuthManager {
    // 注册新用户
    pub async fn register(&self, username: &str, password: &str) -> Result<UserSession>;

    // 登录（主密码派生密钥）
    pub async fn login(&self, username: &str, password: &str) -> Result<UserSession>;

    // 注册新设备（需要已有设备授权或验证码）
    pub async fn register_device(&self, session: &UserSession) -> Result<()>;

    // 获取当前会话
    pub fn current_session(&self) -> Option<&UserSession>;
}
```

**认证流程**:
1. 用户输入账号 + 主密码
2. Argon2id 派生密钥对（本地）
3. 发送注册请求到信令服务器（传输派生公钥）
4. 服务器返回 JWT token
5. 本地保存 root_key（加密存储在安全存储中）

---

## 5. 信令服务器设计

### 5.1 API 端点

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/auth/register` | 注册账号 |
| POST | `/api/auth/login` | 登录获取 token |
| POST | `/api/device/register` | 注册新设备 |
| GET | `/api/device/list` | 获取用户设备列表 |
| WS | `/ws/signal` | WebSocket 信令通道 |
| POST | `/api/stun/config` | 获取 STUN/TURN 配置 |

### 5.2 WebSocket 信令消息

```rust
// 客户端 → 服务器
enum ClientMessage {
    DeviceOnline { device_id: String, token: String },
    DeviceOffline { device_id: String },
    SdpOffer { target: String, sdp: String },
    SdpAnswer { target: String, sdp: String },
    IceCandidate { target: String, candidate: String },
    SyncRequest { target: String },
}

// 服务器 → 客户端
enum ServerMessage {
    DeviceOnline { device_id: String, device_info: DeviceInfo },
    DeviceOffline { device_id: String },
    SdpOffer { from: String, sdp: String },
    SdpAnswer { from: String, sdp: String },
    IceCandidate { from: String, candidate: String },
    Error { code: String, message: String },
}
```

### 5.3 数据库设计

```sql
-- 用户表
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,    -- Argon2id 哈希
    public_key TEXT NOT NULL,       -- Ed25519 公钥
    created_at TEXT NOT NULL
);

-- 设备表
CREATE TABLE devices (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    device_id TEXT UNIQUE NOT NULL,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    public_key TEXT NOT NULL,
    last_seen_at TEXT,
    is_online BOOLEAN DEFAULT FALSE
);
```

---

## 6. UI 设计要点

### 6.1 主要页面

- **登录/注册页**: 账号 + 主密码输入
- **主界面**: 同步文件夹列表 + 同步状态指示器
- **文件夹选择页**: 选择要同步的本地文件夹
- **设备管理页**: 查看已授权设备、添加新设备
- **冲突解决页**: 弹窗显示冲突文件，用户选择保留版本
- **设置页**: 同步间隔、加密设置等

### 6.2 状态指示

- 每个同步文件夹旁显示状态图标：✓ 已同步、↻ 同步中、⚠ 冲突、✗ 错误
- 在线设备显示绿色指示器

---

## 7. 安全模型

### 7.1 威胁模型

| 威胁 | 防护措施 |
|------|---------|
| 服务器被入侵 | 服务器不持有密钥，加密文件对攻击者无用 |
| 网络窃听 | WebRTC DTLS 加密 + XChaCha20-Poly1305 文件加密 |
| 主密码泄露 | Argon2id 抗暴力破解，支持密钥文件增强 |
| 设备丢失 | 远程撤销设备授权 |

### 7.2 密钥层次结构

```
主密码 (用户记忆)
    │
    └── Argon2id ──→ 根密钥 (32 bytes)
                        │
                        ├── HKDF ──→ 文件加密密钥 (每文件随机生成，用根密钥加密存储)
                        ├── HKDF ──→ 设备签名密钥 (Ed25519)
                        └── HKDF ──→ 传输密钥 (WebRTC 之外的额外加密层)
```

---

## 8. 项目结构

```
syncflow/
├── Cargo.toml                      # Rust workspace
├── packages/
│   ├── core/                       # 核心同步引擎 (Rust library)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── crypto/             # crypto_engine
│   │       ├── sync/               # sync_engine
│   │       ├── transport/          # transport_layer
│   │       ├── storage/            # storage_engine
│   │       └── auth/               # auth_manager
│   ├── client/                     # Tauri 客户端应用
│   │   ├── src-tauri/
│   │   │   ├── Cargo.toml
│   │   │   ├── tauri.conf.json
│   │   │   ├── build.rs
│   │   │   └── src/
│   │   │       ├── main.rs         # Tauri 入口
│   │   │       └── commands.rs     # Tauri commands
│   │   ├── src/                    # Web 前端
│   │   │   ├── main.tsx
│   │   │   ├── App.tsx
│   │   │   └── components/
│   │   ├── package.json
│   │   └── vite.config.ts
│   └── server/                     # 信令服务器
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── auth.rs
│           ├── device.rs
│           ├── signal.rs
│           └── stun.rs
├── deploy/
│   ├── docker-compose.yml          # 部署信令服务器 + coturn
│   └── coturn/
│       └── turnserver.conf
└── docs/
    └── superpowers/specs/
        └── 2026-04-20-syncflow-design.md
```

---

## 9. 实施阶段

### Phase 1: 核心基础设施
- [ ] 项目骨架搭建（workspace、Cargo.toml）
- [ ] crypto_engine 实现（加密/解密/密钥派生）
- [ ] storage_engine 实现（SQLite 元数据存储）
- [ ] auth_manager 实现（注册/登录/设备管理）

### Phase 2: 信令服务器
- [ ] 信令服务器基础框架（axum + tokio）
- [ ] 用户注册/登录 API
- [ ] 设备管理 API
- [ ] WebSocket 信令通道
- [ ] STUN 配置端点

### Phase 3: WebRTC 传输
- [ ] transport_layer 实现（webrtc-rs）
- [ ] 信令客户端集成
- [ ] SDP 交换与 ICE 连接
- [ ] DataChannel 数据传输协议

### Phase 4: 同步引擎
- [ ] sync_engine 实现（文件监听 + 同步队列）
- [ ] 版本向量与冲突检测
- [ ] 实时同步 + 定时兜底

### Phase 5: Tauri UI
- [ ] Tauri 2.0 项目搭建
- [ ] 登录/注册 UI
- [ ] 主界面（文件夹列表 + 状态）
- [ ] 文件夹选择器
- [ ] 设备管理 UI
- [ ] 冲突解决 UI

### Phase 6: TODO（后续迭代）
- [ ] WebSocket 备份通道（P2P 失败时兜底）
- [ ] Android 支持
- [ ] 增量同步优化（分块传输）
- [ ] 大文件断点续传
