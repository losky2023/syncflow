import type { ConflictDetail, ConflictInfo, FileDetails } from "../../types/workbench";

interface DetailsPaneProps {
  details: FileDetails | null;
  error: string | null;
  conflicts: ConflictInfo[];
  conflictError: string | null;
  selectedConflictId: number | null;
  conflictDetail: ConflictDetail | null;
  conflictDetailError: string | null;
  conflictActionError: string | null;
  conflictActionLoading: "keep-local" | "keep-remote" | "dismiss" | null;
  onSelectConflict: (conflictId: number) => void;
  onResolveKeepLocal: (conflictId: number) => void;
  onResolveKeepRemote: (conflictId: number) => void;
  onDismissConflict: (conflictId: number) => void;
}

export function DetailsPane({
  details,
  error,
  conflicts,
  conflictError,
  selectedConflictId,
  conflictDetail,
  conflictDetailError,
  conflictActionError,
  conflictActionLoading,
  onSelectConflict,
  onResolveKeepLocal,
  onResolveKeepRemote,
  onDismissConflict,
}: DetailsPaneProps) {
  return (
    <aside className="panel details-panel">
      <div className="section-header details-header-compact">
        <div>
          <h2>详情</h2>
          <p>{details?.spaceName ?? "选中文件或冲突后，在这里查看详细信息。"}</p>
        </div>
      </div>

      {error ? <div className="error-banner error-banner-compact">{error}</div> : null}
      {!error && !details ? (
        <div className="empty-card details-empty empty-card-compact">未选中文件。</div>
      ) : null}

      <div className="details-section-list">
        {details ? (
          <>
            <section className="details-section-card">
              <h3>基本信息</h3>
              <dl className="details-grid">
                <dt>名称</dt>
                <dd>{details.name}</dd>
                <dt>类型</dt>
                <dd>{details.nodeType === "directory" ? "文件夹" : "文件"}</dd>
                <dt>扩展名</dt>
                <dd>{details.extension ?? "-"}</dd>
              </dl>
            </section>

            <section className="details-section-card">
              <h3>文件信息</h3>
              <dl className="details-grid">
                <dt>大小</dt>
                <dd>{details.size} bytes</dd>
                <dt>修改时间</dt>
                <dd>{details.modifiedAt ?? "-"}</dd>
              </dl>
            </section>

            <section className="details-section-card">
              <h3>位置信息</h3>
              <dl className="details-grid">
                <dt>空间</dt>
                <dd>{details.spaceName}</dd>
                <dt>相对路径</dt>
                <dd className="path-value">{details.relativePath}</dd>
              </dl>
            </section>
          </>
        ) : null}

        <section className="details-section-card conflict-section">
          <h3>冲突</h3>
          {conflictError ? <div className="error-banner error-banner-compact">{conflictError}</div> : null}
          {!conflictError && conflicts.length === 0 ? (
            <div className="empty-card empty-card-compact">当前空间没有冲突。</div>
          ) : null}

          {!conflictError && conflicts.length > 0 ? (
            <>
              <div className="conflict-list">
                {conflicts.map((conflict) => (
                  <button
                    key={conflict.id}
                    type="button"
                    className={
                      conflict.id === selectedConflictId
                        ? "conflict-card conflict-card-selected"
                        : "conflict-card"
                    }
                    onClick={() => onSelectConflict(conflict.id)}
                  >
                    <strong title={conflict.relativePath}>{conflict.relativePath}</strong>
                    <span>远端设备: {conflict.remoteDevice}</span>
                    <span>检测时间: {conflict.detectedAt}</span>
                    <small title={conflict.localVersion}>本地版本: {conflict.localVersion}</small>
                    <small title={conflict.remoteVersion}>远端版本: {conflict.remoteVersion}</small>
                  </button>
                ))}
              </div>

              {conflictActionError ? (
                <div className="error-banner error-banner-compact">{conflictActionError}</div>
              ) : null}
              {conflictDetailError ? (
                <div className="error-banner error-banner-compact">{conflictDetailError}</div>
              ) : null}

              {!conflictDetailError && !conflictDetail ? (
                <div className="empty-card empty-card-compact">选择一个冲突查看详情。</div>
              ) : null}

              {conflictDetail ? (
                <div className="conflict-detail">
                  <section className="details-section-card conflict-meta-card">
                    <h3>冲突详情</h3>
                    <dl className="details-grid">
                      <dt>空间</dt>
                      <dd>{conflictDetail.spaceName}</dd>
                      <dt>路径</dt>
                      <dd className="path-value">{conflictDetail.relativePath}</dd>
                      <dt>远端设备</dt>
                      <dd>{conflictDetail.remoteDevice}</dd>
                      <dt>检测时间</dt>
                      <dd>{conflictDetail.detectedAt}</dd>
                    </dl>
                  </section>

                  <div className="conflict-actions">
                    <button
                      type="button"
                      className="secondary-button secondary-button-compact"
                      disabled={!conflictDetail.canKeepLocal || conflictActionLoading !== null}
                      onClick={() => onResolveKeepLocal(conflictDetail.id)}
                    >
                      {conflictActionLoading === "keep-local" ? "处理中..." : "保留本地"}
                    </button>
                    <button
                      type="button"
                      className="secondary-button secondary-button-compact"
                      disabled={!conflictDetail.canKeepRemote || conflictActionLoading !== null}
                      onClick={() => onResolveKeepRemote(conflictDetail.id)}
                    >
                      {conflictActionLoading === "keep-remote" ? "处理中..." : "保留远端"}
                    </button>
                    <button
                      type="button"
                      className="secondary-button secondary-button-compact danger-button-compact"
                      disabled={conflictActionLoading !== null}
                      onClick={() => onDismissConflict(conflictDetail.id)}
                    >
                      {conflictActionLoading === "dismiss" ? "处理中..." : "忽略冲突"}
                    </button>
                  </div>

                  {!conflictDetail.canCompareText && conflictDetail.missingRemoteSnapshotReason ? (
                    <div className="empty-card empty-card-compact">
                      {conflictDetail.missingRemoteSnapshotReason}
                    </div>
                  ) : null}

                  {conflictDetail.canCompareText ? (
                    <section className="details-section-card conflict-compare-card">
                      <h3>文本对比</h3>
                      <div className="conflict-compare-grid">
                        <div className="conflict-compare-pane">
                          <div className="conflict-compare-header">
                            <strong>本地</strong>
                            <span>
                              {conflictDetail.localTextTruncated ? "已截断" : "完整"}
                            </span>
                          </div>
                          <pre className="conflict-text-block">
                            {conflictDetail.localTextContent ?? ""}
                          </pre>
                        </div>
                        <div className="conflict-compare-pane">
                          <div className="conflict-compare-header">
                            <strong>远端</strong>
                            <span>
                              {conflictDetail.remoteTextTruncated ? "已截断" : "完整"}
                            </span>
                          </div>
                          <pre className="conflict-text-block">
                            {conflictDetail.remoteTextContent ?? ""}
                          </pre>
                        </div>
                      </div>
                    </section>
                  ) : null}
                </div>
              ) : null}
            </>
          ) : null}
        </section>
      </div>
    </aside>
  );
}
