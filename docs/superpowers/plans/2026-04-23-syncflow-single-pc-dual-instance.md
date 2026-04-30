# SyncFlow 单机双实例验证

Date: 2026-04-23

## 目的

在只有一台 PC 的情况下，启动两个独立 SyncFlow 实例，用不同数据目录、不同设备名、不同 SDP 端口和不同 Vite 端口模拟两台设备。

## 实例 A

在第一个 PowerShell 窗口运行：

```powershell
cd D:\workspace\wjtb\syncflow\packages\client
$env:SYNCFLOW_PROFILE="a"
$env:SYNCFLOW_DEVICE_NAME="PC-A"
$env:SYNCFLOW_SDP_PORT="18080"
npx tauri dev
```

## 实例 B

在第二个 PowerShell 窗口运行：

```powershell
cd D:\workspace\wjtb\syncflow\packages\client
npm run dev:tauri:peer
```

## 验证步骤

1. 两个窗口分别登录。
2. 实例 A 添加本地目录，例如 `D:\syncflow-demo-a`。
3. 实例 A 在空间卡片点击“邀请”，复制邀请码。
4. 实例 B 粘贴邀请码并点击“加入”，它会使用独立数据目录下的 joined space。
5. 两边都点击空间卡片的“启动”。
6. 等待顶部“发现”或“已连接”计数变化，确认两端至少进入已发现状态。
7. 在实例 A 的目录中新建 `hello-from-a.txt`，内容写入 `hello v1`。
8. 观察实例 B 空间卡片显示的目录中是否出现该文件，并在文件树中打开确认内容为 `hello v1`。
9. 在实例 A 中把 `hello-from-a.txt` 内容改成 `hello v2`。
10. 观察实例 B 预览内容是否更新为 `hello v2`。
11. 在实例 A 中删除 `hello-from-a.txt`。
12. 观察实例 B 文件树中该文件是否消失。

## 建议记录

- 实例 A 本地目录：`D:\syncflow-demo-a`
- 实例 B 自动加入目录：`%LOCALAPPDATA%\syncflow-b\joined-spaces\<space-name>-<sync-key-prefix>`
- 建议同时打开两个实例的开发者日志，记录 `peer connected`、`data received`、`Sent file`、`Received file`、`Sent delete`、`Received delete`。

## 观察点

- 顶部“发现”和“已连接”计数是否变化。
- 顶部“传输”是否出现 `peer connected` 或 `data received`。
- 两个实例的空间卡片是否显示相同 Key 前缀。
- 两个实例是否都进入 `watching` 状态。
- 修改文件后，实例 B 的文本预览是否刷新到新内容。
- 删除文件后，实例 B 的文件树是否去掉对应节点。

## 失败时优先排查

- 两边是否都已经点击空间卡片的“启动”。
- 两个实例顶部是否至少出现“发现”计数变化；如果没有，先看 mDNS/局域网发现。
- 两个实例空间卡片显示的 Key 前缀是否一致；如果不一致，说明不是同一个逻辑空间。
- 顶部“传输”是否出现 `peer connected`；如果没有，优先看 SDP 端口和自动建连。
- 新建/修改能同步但删除不同步时，重点看日志里是否出现 `Sent delete` 和 `Received delete`。

## 清理数据

两个实例的数据目录分别是：

- `%LOCALAPPDATA%\syncflow-a`
- `%LOCALAPPDATA%\syncflow-b`

如需重测，可关闭两个实例后删除这两个目录。
