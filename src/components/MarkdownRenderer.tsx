import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import {
  oneLight,
  oneDark,
} from "react-syntax-highlighter/dist/esm/styles/prism";

interface MarkdownRendererProps {
  content: string;
  className?: string;
  darkMode?: boolean;
}

/**
 * 可复用的 Markdown 渲染组件
 *
 * 支持 GFM（表格、任务列表、删除线）+ 代码块语法高亮
 */
export default function MarkdownRenderer({
  content,
  className = "",
  darkMode = false,
}: MarkdownRendererProps) {
  return (
    <div className={`markdown-body ${className}`}>
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        // 代码块 / 行内代码
        code({ className: codeClassName, children, ...props }) {
          const match = /language-(\w+)/.exec(codeClassName || "");
          const codeString = String(children).replace(/\n$/, "");

          // 判断是否为代码块（通过 className 中的 language- 前缀）
          if (match) {
            return (
              <SyntaxHighlighter
                style={darkMode ? oneDark : oneLight}
                language={match[1]}
                PreTag="div"
                customStyle={{
                  margin: 0,
                  borderRadius: "6px",
                  fontSize: "13px",
                  background: darkMode ? "#2A2A28" : "#F5F4F0",
                }}
              >
                {codeString}
              </SyntaxHighlighter>
            );
          }

          // 行内代码
          return (
            <code
              className="inline-code"
              style={{
                background: darkMode ? "#2A2A28" : "#F5F4F0",
                padding: "2px 6px",
                borderRadius: "4px",
                fontSize: "0.9em",
                fontFamily: "'JetBrains Mono', monospace",
              }}
              {...props}
            >
              {children}
            </code>
          );
        },

        // 代码块容器
        pre({ children }) {
          return (
            <pre
              style={{
                margin: "12px 0",
                borderRadius: "6px",
                overflow: "auto",
              }}
            >
              {children}
            </pre>
          );
        },

        // 链接
        a({ children, href, ...props }) {
          return (
            <a
              href={href}
              style={{ color: "var(--steel)", textDecoration: "underline" }}
              target="_blank"
              rel="noopener noreferrer"
              {...props}
            >
              {children}
            </a>
          );
        },

        // 表格
        table({ children }) {
          return (
            <div style={{ overflowX: "auto", margin: "12px 0" }}>
              <table
                style={{
                  width: "100%",
                  borderCollapse: "collapse",
                  fontSize: "14px",
                }}
              >
                {children}
              </table>
            </div>
          );
        },

        th({ children }) {
          return (
            <th
              style={{
                border: `1px solid ${darkMode ? "#444" : "var(--sand)"}`,
                padding: "8px 12px",
                textAlign: "left",
                fontWeight: 600,
                background: darkMode ? "#2A2A28" : "#F5F4F0",
              }}
            >
              {children}
            </th>
          );
        },

        td({ children }) {
          return (
            <td
              style={{
                border: `1px solid ${darkMode ? "#444" : "var(--sand)"}`,
                padding: "8px 12px",
              }}
            >
              {children}
            </td>
          );
        },

        // 标题层级
        h1({ children }) {
          return (
            <h1
              style={{
                fontSize: "1.5em",
                fontWeight: 700,
                margin: "16px 0 8px",
                lineHeight: 1.3,
              }}
            >
              {children}
            </h1>
          );
        },
        h2({ children }) {
          return (
            <h2
              style={{
                fontSize: "1.3em",
                fontWeight: 600,
                margin: "14px 0 6px",
                lineHeight: 1.3,
              }}
            >
              {children}
            </h2>
          );
        },
        h3({ children }) {
          return (
            <h3
              style={{
                fontSize: "1.15em",
                fontWeight: 600,
                margin: "12px 0 4px",
                lineHeight: 1.3,
              }}
            >
              {children}
            </h3>
          );
        },

        // 段落
        p({ children }) {
          return (
            <p style={{ margin: "8px 0", lineHeight: 1.7 }}>{children}</p>
          );
        },

        // 列表
        ul({ children }) {
          return (
            <ul style={{ margin: "8px 0", paddingLeft: "24px" }}>{children}</ul>
          );
        },
        ol({ children }) {
          return (
            <ol style={{ margin: "8px 0", paddingLeft: "24px" }}>{children}</ol>
          );
        },
        li({ children }) {
          return <li style={{ margin: "4px 0", lineHeight: 1.6 }}>{children}</li>;
        },

        // 引用块
        blockquote({ children }) {
          return (
            <blockquote
              style={{
                margin: "12px 0",
                padding: "8px 16px",
                borderLeft: `3px solid ${darkMode ? "#555" : "var(--sand)"}`,
                color: darkMode ? "#999" : "var(--stone-dark)",
                background: darkMode ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)",
              }}
            >
              {children}
            </blockquote>
          );
        },

        // 分割线
        hr() {
          return (
            <hr
              style={{
                margin: "16px 0",
                border: "none",
                borderTop: `1px solid ${darkMode ? "#333" : "var(--sand)"}`,
              }}
            />
          );
        },
      }}
    >
      {content}
    </ReactMarkdown>
    </div>
  );
}
