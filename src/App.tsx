import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

type AppSnapshot = {
  totalEntries: number;
  importedKeys: string[];
  citationOrder: string[];
};

type ImportResult = {
  total: number;
  imported: number;
  newCount: number;
  overwrittenCount: number;
  failed: number;
  message: string;
};

type CiteResult = {
  citationText: string;
  citedReferencesText: string;
  newlyAddedCount: number;
};

type ErrorLike = {
  message?: string;
};

function toErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (typeof error === "object" && error !== null) {
    const maybeMessage = (error as ErrorLike).message;
    if (typeof maybeMessage === "string" && maybeMessage.trim().length > 0) {
      return maybeMessage;
    }

    try {
      return JSON.stringify(error);
    } catch {
      return "Unknown error";
    }
  }

  return "Unknown error";
}

function App() {
  const [citationInput, setCitationInput] = useState(
    "10495806,10648348,10980318,10807485",
  );
  const [citationOutput, setCitationOutput] = useState("");
  const [citedReferencesText, setCitedReferencesText] = useState("");
  const [importedKeys, setImportedKeys] = useState<string[]>([]);
  const [statusText, setStatusText] = useState("正在加载本地文献状态...");
  const [isLoading, setIsLoading] = useState(true);
  const [isImporting, setIsImporting] = useState(false);
  const [isCiting, setIsCiting] = useState(false);

  const refreshSnapshot = useCallback(async (): Promise<AppSnapshot> => {
    const [snapshot, referencesText] = await Promise.all([
      invoke<AppSnapshot>("get_app_snapshot"),
      invoke<string>("get_cited_references_text"),
    ]);

    setImportedKeys(snapshot.importedKeys);
    setCitedReferencesText(referencesText);

    return snapshot;
  }, []);

  useEffect(() => {
    let mounted = true;

    void (async () => {
      setIsLoading(true);

      try {
        const snapshot = await refreshSnapshot();
        if (!mounted) {
          return;
        }

        setStatusText(`已加载 ${snapshot.totalEntries} 条本地文献，可开始导入或引用。`);
      } catch (error) {
        if (!mounted) {
          return;
        }

        setStatusText(
          `加载本地状态失败：${toErrorMessage(error)}。请在 Tauri 桌面环境中运行。`,
        );
      } finally {
        if (mounted) {
          setIsLoading(false);
        }
      }
    })();

    return () => {
      mounted = false;
    };
  }, [refreshSnapshot]);

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

  async function handleImportBib(): Promise<void> {
    if (isImporting) {
      return;
    }

    setIsImporting(true);

    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "BibTeX", extensions: ["bib"] }],
      });

      if (selected === null) {
        setStatusText("已取消导入。未选择 bib 文件。");
        return;
      }

      if (Array.isArray(selected)) {
        setStatusText("当前版本仅支持一次导入一个 bib 文件。请选择单个文件。");
        return;
      }

      const importResult = await invoke<ImportResult>("import_bib_file", {
        path: selected,
      });
      const snapshot = await refreshSnapshot();

      setStatusText(
        `${importResult.message} 当前库内共 ${snapshot.totalEntries} 条文献。`,
      );
    } catch (error) {
      setStatusText(`导入失败：${toErrorMessage(error)}`);
    } finally {
      setIsImporting(false);
    }
  }

  async function handleCite(): Promise<void> {
    if (isCiting) {
      return;
    }

    setIsCiting(true);

    try {
      const result = await invoke<CiteResult>("cite_keys", {
        input: citationInput,
      });

      setCitationOutput(result.citationText);
      setCitedReferencesText(result.citedReferencesText);

      if (result.newlyAddedCount > 0) {
        setStatusText(
          `引用完成：新增 ${result.newlyAddedCount} 条引用并返回编号 ${result.citationText}。`,
        );
      } else {
        setStatusText(`引用完成：全部为已有引用，返回编号 ${result.citationText}。`);
      }
    } catch (error) {
      setCitationOutput("");
      setStatusText(`引用失败：${toErrorMessage(error)}`);
    } finally {
      setIsCiting(false);
    }
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <p className="eyebrow">Reference Workspace</p>
        <h1>文献引用管理工具</h1>
        <p className="subtitle">
          导入 bib 文献库，按 key 引用并自动维护编号化参考文献总表。
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
              <button
                type="button"
                className="primary"
                onClick={() => void handleCite()}
                disabled={isCiting || isLoading}
              >
                {isCiting ? "引用中..." : "引用"}
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
              placeholder="引用后将在这里显示按编号排列的参考文献总表。"
            />
          </article>
        </div>

        <aside className="panel right-column">
          <div className="panel-header">
            <h2>已导入文献 Key（{importedKeys.length}）</h2>
            <button
              type="button"
              className="primary ghosted"
              onClick={() => void handleImportBib()}
              disabled={isImporting || isLoading}
            >
              {isImporting ? "导入中..." : "导入 .bib"}
            </button>
          </div>

          <ul className="key-list">
            {importedKeys.length > 0 ? (
              importedKeys.map((key) => <li key={key}>{key}</li>)
            ) : (
              <li className="empty-state">暂无已导入 key，请先导入 bib 文件。</li>
            )}
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
