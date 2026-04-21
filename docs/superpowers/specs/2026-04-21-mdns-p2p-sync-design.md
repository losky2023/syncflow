# mDNS 局域网设备发现与 P2P 同步设计

**Goal**: 去掉 WebSocket 信号服务器，改为 mDNS 局域网自动发现 + 本地 HTTP 服务交换 SDP，实现零服务器成本的纯 P2P 文件同步。

**Architecture**: 每台设备启动时广播 mDNS 服务并监听局域网中其他设备，发现设备后通过轻量 HTTP 服务交换 WebRTC SDP 信息，建立 Data Channel 后直接传输加密文件。

**Tech Stack**: mdns-sd (mDNS), axum (本地 HTTP), webrtc-rs (WebRTC), Tauri 2.0 (跨平台)

---

## 模块设计

### 1. mDNS 设备发现 (`transport/discovery.rs`)

每台设备启动时注册一个 mDNS 服务，类型为 `_syncflow._tcp.local.`，端口为本机 SDP 交换服务的端口（默认 18080）。服务信息 TXT 记录中包含 `device_id`、`device_name`、`platform`。

同时启动 mDNS 浏览器，监听 `_syncflow._tcp.local.` 类型的新服务。当发现新设备时，记录其 IP、port、device_id 等信息，并通过 broadcast channel 发送 `DiscoveryEvent::PeerDiscovered`。

```rust
pub struct DiscoveryService {
    daemon: mdns_sd::ServiceDaemon,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

pub enum DiscoveryEvent {
    PeerDiscovered { device_id: String, device_name: String, ip: String, port: u16, platform: String },
    PeerLost { device_id: String },
}
```

**关键行为**:
- `register(device_id, device_name, platform, port)`: 注册本设备 mDNS 服务
- `start_browse()`: 启动浏览，返回 receiver
- 设备离线时 mDNS 自动发送 `PeerLost` 事件

### 2. 本地 SDP 交换服务 (`transport/sdp_exchange.rs`)

每台设备启动一个轻量 axum HTTP 服务，监听 `0.0.0.0:18080`，提供两个端点：

- `POST /sdp/offer` — 接收对方的 SDP offer，创建 answer 并返回
- `POST /sdp/answer` — 接收对方的 SDP answer

```rust
#[derive(serde::Deserialize)]
pub struct SdpRequest {
    pub sdp: String,
    pub device_id: String,
}

#[derive(serde::Serialize)]
pub struct SdpResponse {
    pub sdp: String,
}
```

**offer 处理流程**:
1. 收到 offer → 调用 `set_remote_offer(pc, &sdp)`
2. 调用 `create_answer(pc)` 生成 answer
3. 设置 `set_local_description(answer)`
4. 返回 answer SDP

### 3. 新的 TransportLayer (`transport/mod.rs`)

```rust
pub struct TransportLayer {
    peers: Arc<RwLock<HashMap<String, Arc<RTCPeerConnection>>>>,
    data_channels: Arc<RwLock<HashMap<String, Arc<RTCDataChannel>>>>,
    event_tx: broadcast::Sender<TransportEvent>,
    discovery: DiscoveryService,
    local_port: u16,
    device_id: String,
    pc_template: Arc<RTCPeerConnection>,  // 预创建的 PC 模板
}
```

**连接流程**:
1. 启动 mDNS 注册 + 浏览
2. 发现设备 A → 检查是否已连接（去重）
3. 向 A 的 IP:port 发送 `POST /sdp/offer`
4. 收到 A 的 answer → `set_remote_answer(pc, &sdp)`
5. WebRTC ICE 连接建立 → `PeerConnected` 事件
6. 设备离线 → `PeerDisconnected` 事件

**去重逻辑**: 如果已经向某设备发送过 offer（本端为 offerer），则不再重复发起。如果对方先发起（本端收到 offer），则接受。

### 4. 连接建立完整时序

```
Device A (192.168.1.10:18080)          Device B (192.168.1.20:18080)
        │                                         │
        ├─ mDNS broadcast ───────────────────────►│
        │◄──────── mDNS discover ─────────────────┤
        │                                         │
        │ (A 的 mDNS 浏览器发现 B)                 │
        │                                         │
        ├─ POST http://192.168.1.20:18080/sdp/offer
        │  body: {sdp: "...", device_id: "A"}    │
        │                                         │
        │                            (B 收到 offer)│
        │                            set_remote_offer│
        │                            create_answer │
        │                            set_local_description│
        │                                         │
        │◄─ 200 {sdp: "answer..."} ──────────────┤
        │                                         │
        │ (A 收到 answer)                          │
        │ set_remote_answer                        │
        │ ICE 交换候选                             │
        │                                         │
        ├──────── WebRTC Data Channel 连接成功 ───►│
        │◄──────── 开始同步文件 ───────────────────┤
```

### 5. 需要删除的模块

- `transport/signal_client.rs` — 完全删除
- `transport/tests.rs` — 更新测试
- `packages/server/` — 标记为废弃（不删除，留作参考）

### 6. Cargo.toml 变更

**packages/core/Cargo.toml**:
- 删除: `tokio-tungstenite`, `url`
- 新增: `mdns-sd = "0.12"`, `axum = "0.7"`, `tower-http = "0.5"` (features: trace)
- 新增 workspace dependency: `hyper = "1"` (axum 需要)

**Workspace Cargo.toml**:
- 删除: `tokio-tungstenite`（如没有其他包使用）

### 7. Client 端改动

**`main.rs`**:
- 初始化 `TransportLayer` 时不再传 `signal_url`
- 新增 `start_sync()` 启动 mDNS + SDP 服务 + 文件监听
- 新增 `stop_sync()` 停止所有后台任务

**`commands.rs`**:
- `login` 改为初始化本地 session（不调用远程服务器）
- 新增 `start_sync()` — 启动同步引擎
- 新增 `stop_sync()` — 停止同步引擎
- 新增 `get_discovered_devices()` — 返回 mDNS 发现的设备列表
- `add_synced_folder` 注册到文件监听器

**TauriState**:
```rust
struct TauriState {
    storage: Arc<Mutex<StorageEngine>>,
    sync_engine: Arc<Mutex<Option<SyncEngine>>>,
    discovery_events: broadcast::Sender<DiscoveryEvent>,
}
```

### 8. 本地认证

去掉服务端 auth，改为纯本地方案：
1. 首次启动时生成 device_id (UUID v4) + Ed25519 keypair
2. 用户设置密码 → Argon2id 派生 root_key
3. 保存到本地 keychain/文件（secrecy::SecretBox）
4. 后续启动用密码解锁

## 数据流

```
用户选择文件夹
    │
    ▼
FileWatcher (notify-debouncer-mini)
    │ 文件变化事件
    ▼
SyncEngine.handle_file_event
    │ 读取文件 → 更新 version vector → 保存 metadata → 入队
    ▼
SyncQueue
    │ 取出上传任务
    ▼
TransportLayer.send_data (WebRTC Data Channel)
    │ 加密文件内容 → 元数据 + 加密内容
    ▼
Peer 接收 → 冲突检测 → 解密 → 写入磁盘
```

## 错误处理

| 场景 | 处理 |
|------|------|
| mDNS 无响应 | 5 秒超时，重试 3 次 |
| SDP 交换失败 | 记录日志，下次 mDNS 发现时重试 |
| WebRTC 连接失败 | 关闭连接，清理状态，等待下次发现 |
| HTTP 服务端口冲突 | 尝试 18080-18090 范围 |
| 设备突然离线 | mDNS PeerLost 事件 → 清理连接 |

## 测试计划

1. `discovery.rs` — mDNS 注册和发现（用 mock 或真实 mDNS）
2. `sdp_exchange.rs` — HTTP 端点测试（axum test client）
3. `mod.rs` — TransportLayer 集成测试（两个实例互相连接）
4. 端到端 — 两台设备通过 localhost 回环测试（同一进程创建两个 TransportLayer）
