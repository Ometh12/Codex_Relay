import type { RolloutPreview } from "../lib/types";
import { formatBytes, formatRfc3339 } from "../lib/format";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

function formatBytes0(n: number): string {
  if (n === 0) return "0 B";
  return formatBytes(n);
}

function roleZh(role: string): string {
  switch (role) {
    case "user":
      return "用户";
    case "assistant":
      return "助手";
    case "system":
      return "系统";
    case "developer":
      return "开发者";
    case "tool":
      return "工具";
    default:
      return role || "未知";
  }
}

export default function RolloutPreviewView(props: {
  preview: RolloutPreview;
  renderMarkdown?: boolean;
}) {
  const p = props.preview;
  const counts = Object.entries(p.message_counts ?? {});
  const previewCounts = Object.entries(p.message_counts_preview ?? {});
  const renderMarkdown = Boolean(props.renderMarkdown);
  const scope = p.stats_scope === "full" ? "全量" : "扫描窗口";

  function roleClass(role: string): string {
    switch (role) {
      case "user":
        return "role-user";
      case "assistant":
        return "role-assistant";
      case "system":
        return "role-system";
      case "developer":
        return "role-developer";
      case "tool":
        return "role-tool";
      default:
        return "role-other";
    }
  }

  return (
    <div className="preview">
      {p.warning ? <div className="previewWarn">提示：{p.warning}</div> : null}

      <div className="kv previewMeta">
        <div>来源</div>
        <div className="mono">{p.source}</div>
        <div>会话ID</div>
        <div className="mono">{p.session_id ?? "-"}</div>
        <div>统计范围</div>
        <div className="mono">{scope}</div>
        <div>读取范围</div>
        <div className="mono">
          {formatBytes0(p.scanned_bytes)}（偏移 {formatBytes0(p.scanned_offset)}）
        </div>
        <div>工具调用（{scope}）</div>
        <div className="mono">
          {p.tool_calls} / 输出 {p.tool_call_outputs}
        </div>
        <div>消息统计（{scope}）</div>
        <div className="previewCounts">
          {counts.length === 0 ? (
            <span className="muted">-</span>
          ) : (
            counts.map(([role, n]) => (
              <span className="pill" key={role}>
                {roleZh(role)} {n}
              </span>
            ))
          )}
        </div>
        <div>消息统计（预览显示）</div>
        <div className="previewCounts">
          {previewCounts.length === 0 ? (
            <span className="muted">-</span>
          ) : (
            previewCounts.map(([role, n]) => (
              <span className="pill" key={role}>
                {roleZh(role)} {n}
              </span>
            ))
          )}
        </div>
      </div>

      {p.messages.length === 0 ? (
        <div className="muted">未解析到可预览的消息。</div>
      ) : (
        <div className="previewMessages">
          {p.messages.map((m, idx) => (
            <div
              className={`previewMessage ${roleClass(m.role)}`}
              key={`${idx}-${m.timestamp ?? ""}`}
            >
              <div className="previewMessageHeader">
                <span className="pill">{roleZh(m.role)}</span>
                <span className="muted small">
                  {m.timestamp ? formatRfc3339(m.timestamp) : "-"}
                </span>
                {m.content_types?.length ? (
                  <span className="muted small">{m.content_types.join(", ")}</span>
                ) : null}
              </div>
              {renderMarkdown ? (
                <div className="previewText previewMarkdown">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {m.text}
                  </ReactMarkdown>
                </div>
              ) : (
                <pre className="previewText">{m.text}</pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
