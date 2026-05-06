import { useState } from "react";
import { Workbench } from "./app/Workbench";
import { login, startSync, stopSync } from "./lib/tauriClient";
import "./styles/workbench.css";

interface SyncStatus {
  syncRunning: boolean;
  deviceName: string;
  deviceId: string;
}

function App() {
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [loginError, setLoginError] = useState("");
  const [syncStatus, setSyncStatus] = useState<SyncStatus>({
    syncRunning: false,
    deviceName: "",
    deviceId: "",
  });

  async function handleLogin(event: React.FormEvent) {
    event.preventDefault();
    setLoginError("");

    try {
      const result = await login(username, password);
      if (!result.success) {
        setLoginError(result.error ?? "登录失败");
        return;
      }

      setIsLoggedIn(true);
      setSyncStatus({
        syncRunning: false,
        deviceName: result.device_name,
        deviceId: result.device_id,
      });

      await startSync(password, result.device_name);
      setSyncStatus((current) => ({ ...current, syncRunning: true }));
    } catch (error) {
      setLoginError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleStopSync() {
    try {
      await stopSync();
      setSyncStatus((current) => ({ ...current, syncRunning: false }));
    } catch (error) {
      console.error("Failed to stop sync:", error);
    }
  }

  if (isLoggedIn) {
    return <Workbench syncStatus={syncStatus} onStopSync={handleStopSync} />;
  }

  return (
    <div className="workbench-shell">
      <div className="panel" style={{ maxWidth: 420, margin: "90px auto", padding: 24 }}>
        <h1 style={{ margin: "0 0 8px" }}>SyncFlow</h1>
        <p style={{ color: "#6b7280", fontSize: 14 }}>
          输入任意用户名和密码即可登录（本地认证，无需服务器）
        </p>
        {loginError ? <div className="error-banner">{loginError}</div> : null}
        <form onSubmit={handleLogin} style={{ marginTop: 16 }}>
          <div style={{ marginBottom: 12 }}>
            <label>用户名</label>
            <input
              type="text"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              style={{ width: "100%", boxSizing: "border-box", padding: 10, marginTop: 4 }}
            />
          </div>
          <div style={{ marginBottom: 12 }}>
            <label>密码</label>
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              style={{ width: "100%", boxSizing: "border-box", padding: 10, marginTop: 4 }}
            />
          </div>
          <button className="primary-button" type="submit" style={{ width: "100%" }}>
            登录
          </button>
        </form>
      </div>
    </div>
  );
}

export default App;
