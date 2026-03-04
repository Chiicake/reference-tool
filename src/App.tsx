import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AppSnapshot = {
  totalEntries: number;
  importedKeys: string[];
  citationOrder: string[];
};

const demoImportedKeys = [
  "9750059",
  "10495806",
  "10648348",
  "10980318",
  "10807485",
  "9473521",
  "9354193",
  "1474.1-1999",
];

const demoReferences = [
  "[1] Wang X, Liu L, Tang T, Sun W. Enhancing Communication-Based Train Control Systems Through Train-to-Train Communications[J]. IEEE Transactions on Intelligent Transportation Systems, 2019, 20(4): 1544-1561.",
  "[2] IEEE Std 1474.1-1999, IEEE Standard for Communication Based Train Control Performance Requirements and Functional Requirements[S]. New York: IEEE, 1999: 1-36. DOI: 10.1109/IEEESTD.1999.90611.",
  "[3] Liu X, Yu Y, Li F, Durrani T S. Throughput Maximization for RIS-UAV Relaying Communications[J]. IEEE Transactions on Intelligent Transportation Systems, 2022, 23(10): 19569-19574. DOI: 10.1109/TITS.2022.3161698.",
];

const demoIndexByKey = new Map<string, number>([
  ["9354193", 1],
  ["1474.1-1999", 2],
  ["9750059", 3],
  ["10495806", 4],
  ["10648348", 5],
  ["10980318", 6],
  ["10807485", 7],
  ["9473521", 8],
]);

function parseKeys(rawInput: string): string[] {
  return rawInput
    .split(/[\s,，]+/)
    .map((token) => token.trim())
    .filter((token) => token.length > 0);
}

function compressIndexes(indexes: number[]): string {
  const sorted = [...new Set(indexes)].sort((a, b) => a - b);
  if (sorted.length === 0) {
    return "";
  }

  const sections: string[] = [];
  let start = sorted[0];
  let previous = sorted[0];

  for (let i = 1; i < sorted.length; i += 1) {
    const current = sorted[i];
    if (current === previous + 1) {
      previous = current;
      continue;
    }

    sections.push(start === previous ? `[${start}]` : `[${start}]-[${previous}]`);
    start = current;
    previous = current;
  }

  sections.push(start === previous ? `[${start}]` : `[${start}]-[${previous}]`);
  return sections.join(", ");
}

function App() {
  const [citationInput, setCitationInput] = useState(
    "10495806,10648348,10980318,10807485",
  );
  const [citationOutput, setCitationOutput] = useState("");
  const [statusText, setStatusText] = useState(
    "正在加载本地存储状态...",
  );
  const citedReferencesText = useMemo(() => demoReferences.join("\n\n"), []);

  useEffect(() => {
    let active = true;

    void invoke<AppSnapshot>("get_app_snapshot")
      .then((snapshot) => {
        if (!active) {
          return;
        }

        setStatusText(
          `Step 2 已接入持久化状态层：已加载 ${snapshot.totalEntries} 条本地文献。`,
        );
      })
      .catch(() => {
        if (!active) {
          return;
        }

        setStatusText(
          "Step 2 已完成存储层开发；当前预览环境未连接 Tauri 后端，显示演示数据。",
        );
      });

    return () => {
      active = false;
    };
  }, []);

  async function copyText(content: string, label: string): Promise<void> {
    if (!content.trim()) {
      setStatusText(`${label}为空，暂无可复制内容。`);
      return;
    }

    try {
      await navigator.clipboard.writeText(content);
      setStatusText(`${label}已复制到剪贴板。`);
    } catch {
      setStatusText(`复制${label}失败，请检查系统剪贴板权限。`);
    }
  }

  function handleDemoCite(): void {
    const keys = parseKeys(citationInput);
    if (keys.length === 0) {
      setCitationOutput("");
      setStatusText("请输入至少一个引用 key。示例：10495806,10648348");
      return;
    }

    const missing = keys.filter((key) => !demoIndexByKey.has(key));
    if (missing.length > 0) {
      setCitationOutput("");
      setStatusText(`以下 key 暂未导入：${missing.join(", ")}`);
      return;
    }

    const indexes = keys
      .map((key) => demoIndexByKey.get(key))
      .filter((value): value is number => value !== undefined);

    setCitationOutput(compressIndexes(indexes));
    setStatusText(
      "当前为界面演示数据，后续步骤会替换为导入 bib 后的真实编号逻辑。",
    );
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <p className="eyebrow">Reference Workspace</p>
        <h1>文献引用管理工具</h1>
        <p className="subtitle">
          轻量 Tauri 桌面应用骨架，支持 bib 导入、按 key 引用与参考文献总表输出。
        </p>
      </header>

      <section className="workspace-grid">
        <div className="left-column">
          <article className="panel input-panel">
            <div className="panel-header">
              <h2>引用操作区</h2>
            </div>

            <label className="field-title" htmlFor="citation-input">
              引用框（支持多个 key）
            </label>
            <textarea
              id="citation-input"
              value={citationInput}
              onChange={(event) => setCitationInput(event.currentTarget.value)}
              placeholder="示例：10495806,10648348,10980318,10807485"
            />

            <label className="field-title" htmlFor="citation-output">
              引用返回框（只读）
            </label>
            <textarea
              id="citation-output"
              value={citationOutput}
              readOnly
              placeholder="点击“引用”后返回示例：[1]-[3], [5]"
            />

            <div className="action-row action-row-right">
              <button
                type="button"
                className="secondary"
                onClick={() => void copyText(citationOutput, "引用返回结果")}
              >
                复制返回
              </button>
              <button type="button" className="primary" onClick={handleDemoCite}>
                引用
              </button>
            </div>
          </article>

          <article className="panel cited-panel">
            <div className="panel-header">
              <h2>已有引用文献</h2>
              <button
                type="button"
                className="secondary"
                onClick={() => void copyText(citedReferencesText, "已引用文献")}
              >
                复制
              </button>
            </div>
            <textarea
              className="cited-output"
              readOnly
              value={citedReferencesText}
            />
          </article>
        </div>

        <aside className="panel right-column">
          <div className="panel-header">
            <h2>已导入文献 Key</h2>
            <button type="button" className="primary ghosted">
              导入 .bib
            </button>
          </div>

          <ul className="key-list">
            {demoImportedKeys.map((key) => (
              <li key={key}>{key}</li>
            ))}
          </ul>
        </aside>
      </section>

      <p className="status-bar" role="status">
        {statusText}
      </p>
    </main>
  );
}

export default App;
