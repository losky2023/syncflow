import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DeviceInfo {
  device_id: string;
  device_name: string;
  platform: string;
  is_online: boolean;
}

interface FolderInfo {
  path: string;
  status: string;
  file_count: number;
}

function App() {
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [folders, setFolders] = useState<FolderInfo[]>([]);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);

  useEffect(() => {
    if (isLoggedIn) {
      loadFolders();
      loadDevices();
    }
  }, [isLoggedIn]);

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault();
    try {
      const result = await invoke("login", { username, password });
      if ((result as any).success) {
        setIsLoggedIn(true);
      }
    } catch (err) {
      console.error("Login failed:", err);
    }
  }

  async function loadFolders() {
    try {
      const result = await invoke("get_synced_folders");
      setFolders(result as FolderInfo[]);
    } catch (err) {
      console.error("Failed to load folders:", err);
    }
  }

  async function loadDevices() {
    try {
      const result = await invoke("get_device_info");
      setDevices(result as DeviceInfo[]);
    } catch (err) {
      console.error("Failed to load devices:", err);
    }
  }

  if (!isLoggedIn) {
    return (
      <div style={{ maxWidth: 400, margin: "100px auto", padding: 20 }}>
        <h1>SyncFlow</h1>
        <form onSubmit={handleLogin}>
          <div style={{ marginBottom: 12 }}>
            <label>Username</label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              style={{ width: "100%", padding: 8, marginTop: 4 }}
            />
          </div>
          <div style={{ marginBottom: 12 }}>
            <label>Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              style={{ width: "100%", padding: 8, marginTop: 4 }}
            />
          </div>
          <button type="submit" style={{ width: "100%", padding: 10 }}>
            Login
          </button>
        </form>
      </div>
    );
  }

  return (
    <div style={{ padding: 20 }}>
      <h1>SyncFlow</h1>
      <h2>Synced Folders</h2>
      {folders.length === 0 ? (
        <p>No synced folders yet. Add a folder to get started.</p>
      ) : (
        <ul>
          {folders.map((f, i) => (
            <li key={i}>
              {f.path} — <span>{f.status}</span> ({f.file_count} files)
            </li>
          ))}
        </ul>
      )}

      <h2>Devices</h2>
      {devices.length === 0 ? (
        <p>No other devices connected.</p>
      ) : (
        <ul>
          {devices.map((d, i) => (
            <li key={i}>
              {d.device_name} ({d.platform}) —{" "}
              {d.is_online ? "Online" : "Offline"}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export default App;
