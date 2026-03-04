import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

type AppSnapshot = {
  totalEntries: number;
  importedKeys: string[];
  citationOrder: string[];
  citationStartIndex: number;
  nextCitationIndex: number;
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
  const [citationInput, setCitationInput] = useState("");
  const [citationOutput, setCitationOutput] = useState("");
  const [citedReferencesText, setCitedReferencesText] = useState("");
  const [importedKeys, setImportedKeys] = useState<string[]>([]);
  const [statusText, setStatusText] = useState("正在加载本地文献状态...");
  const [isManualOpen, setIsManualOpen] = useState(false);
  const [nextCitationIndexInput, setNextCitationIndexInput] = useState("");
  const [currentNextCitationIndex, setCurrentNextCitationIndex] = useState<
    number | null
  >(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isImporting, setIsImporting] = useState(false);
  const [isCiting, setIsCiting] = useState(false);
  const [isClearingLibrary, setIsClearingLibrary] = useState(false);
  const [isClearingCitations, setIsClearingCitations] = useState(false);
  const [isSettingNextIndex, setIsSettingNextIndex] = useState(false);

  const refreshSnapshot = useCallback(async (): Promise<AppSnapshot> => {
    const [snapshot, referencesText] = await Promise.all([
      invoke<AppSnapshot>("get_app_snapshot"),
      invoke<string>("get_cited_references_text"),
    ]);

    setImportedKeys(snapshot.importedKeys);
    setCitedReferencesText(referencesText);
    setCurrentNextCitationIndex(snapshot.nextCitationIndex);

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

  async function handleClearLibrary(): Promise<void> {
    if (isClearingLibrary) {
      return;
    }

    setIsClearingLibrary(true);

    try {
      await invoke<AppSnapshot>("clear_library");
      const snapshot = await refreshSnapshot();
      setCitationOutput("");
      setNextCitationIndexInput("");
      setStatusText(
        `数据库已清空。当前文献数 ${snapshot.totalEntries}，下一个序号从 ${snapshot.nextCitationIndex} 开始。`,
      );
    } catch (error) {
      setStatusText(`清空数据库失败：${toErrorMessage(error)}`);
    } finally {
      setIsClearingLibrary(false);
    }
  }

  async function handleClearCitations(): Promise<void> {
    if (isClearingCitations) {
      return;
    }

    setIsClearingCitations(true);

    try {
      await invoke<AppSnapshot>("clear_citations");
      const snapshot = await refreshSnapshot();
      setCitationOutput("");
      setNextCitationIndexInput("");
      setStatusText(`已有引用已清空，下一个序号重置为 ${snapshot.nextCitationIndex}。`);
    } catch (error) {
      setStatusText(`清空已有引用失败：${toErrorMessage(error)}`);
    } finally {
      setIsClearingCitations(false);
    }
  }

  async function handleSetNextCitationIndex(): Promise<void> {
    if (isSettingNextIndex) {
      return;
    }

    const normalizedInput = nextCitationIndexInput.trim();
    let nextIndex: number | null = null;

    if (normalizedInput.length > 0) {
      const parsed = Number.parseInt(normalizedInput, 10);
      if (!Number.isInteger(parsed) || parsed < 1) {
        setStatusText("下一个序号必须是大于等于 1 的整数；留空则自动按上一个序号+1。");
        return;
      }

      nextIndex = parsed;
    }

    setIsSettingNextIndex(true);

    try {
      const payload: { nextIndex?: number } = {};
      if (nextIndex !== null) {
        payload.nextIndex = nextIndex;
      }

      const snapshot = await invoke<AppSnapshot>(
        "set_next_citation_index",
        payload,
      );
      setCurrentNextCitationIndex(snapshot.nextCitationIndex);
      setNextCitationIndexInput("");
      setStatusText(
        `下一个引用序号已设置为 ${snapshot.nextCitationIndex}（仅对后续首次引用的新 key 生效）。`,
      );
    } catch (error) {
      setStatusText(`设置下一个序号失败：${toErrorMessage(error)}`);
    } finally {
      setIsSettingNextIndex(false);
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
      const snapshot = await refreshSnapshot();

      setCitationOutput(result.citationText);
      setCitedReferencesText(result.citedReferencesText);

      const isParagraphMode = citationInput.includes("\\cite");

      if (result.newlyAddedCount > 0) {
        setStatusText(
          isParagraphMode
            ? `段落引用替换完成：新增 ${result.newlyAddedCount} 条引用。`
            : `引用完成：新增 ${result.newlyAddedCount} 条引用并返回编号 ${result.citationText}（下一个序号：${snapshot.nextCitationIndex}）。`,
        );
      } else {
        setStatusText(
          isParagraphMode
            ? "段落引用替换完成：全部为已有引用。"
            : `引用完成：全部为已有引用，返回编号 ${result.citationText}。`,
        );
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
        <div className="app-header-top">
          <p className="eyebrow">Reference Workspace</p>
          <button
            type="button"
            className="secondary manual-button"
            onClick={() => setIsManualOpen(true)}
          >
            说明书
          </button>
        </div>
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
              引用框
            </label>
            <textarea
              id="citation-input"
              value={citationInput}
              onChange={(event) => setCitationInput(event.currentTarget.value)}
              placeholder="可输入多个 key，或直接粘贴含 \\cite{} 的段落"
            />

            <label className="field-title" htmlFor="citation-output">
              引用返回框
            </label>
            <textarea
              id="citation-output"
              value={citationOutput}
              readOnly
              placeholder="点击“引用”后返回示例：[1]-[3], [5]"
            />

            <label className="field-title" htmlFor="next-citation-index">
              设置下一个序号
            </label>
            <div className="inline-setting-row">
              <input
                id="next-citation-index"
                className="index-input"
                type="number"
                min={1}
                step={1}
                value={nextCitationIndexInput}
                onChange={(event) => setNextCitationIndexInput(event.currentTarget.value)}
                placeholder="留空自动+1；可手动设为16"
              />
              <button
                type="button"
                className="secondary"
                onClick={() => void handleSetNextCitationIndex()}
                disabled={isSettingNextIndex || isLoading}
              >
                {isSettingNextIndex ? "设置中..." : "设置下一个"}
              </button>
            </div>
            <p className="hint-text">
              当前下一个序号：
              {currentNextCitationIndex ?? "-"}
            </p>

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
              <div className="header-button-group">
                <button
                  type="button"
                  className="secondary"
                  onClick={() => void copyText(citedReferencesText, "已引用文献")}
                >
                  复制
                </button>
                <button
                  type="button"
                  className="danger"
                  onClick={() => void handleClearCitations()}
                  disabled={isClearingCitations || isLoading}
                >
                  {isClearingCitations ? "清空中..." : "清空引用"}
                </button>
              </div>
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
            <div className="header-button-group">
              <button
                type="button"
                className="primary ghosted"
                onClick={() => void handleImportBib()}
                disabled={isImporting || isLoading}
              >
                {isImporting ? "导入中..." : "导入 .bib"}
              </button>
              <button
                type="button"
                className="danger"
                onClick={() => void handleClearLibrary()}
                disabled={isClearingLibrary || isLoading}
              >
                {isClearingLibrary ? "清空中..." : "清空数据库"}
              </button>
            </div>
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

      {isManualOpen ? (
        <div className="manual-backdrop" onClick={() => setIsManualOpen(false)}>
          <section
            className="manual-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="manual-title"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="manual-header">
              <h2 id="manual-title">使用说明书</h2>
              <button
                type="button"
                className="secondary"
                onClick={() => setIsManualOpen(false)}
              >
                关闭
              </button>
            </div>

            <p>
              本工具用于管理 bib 文献并快速生成引用编号。核心流程是：导入 bib、输入引用、自动返回编号并维护已引用文献总表。
            </p>

            <h3>一、导入文献</h3>
            <p>
              点击右侧“导入 .bib”，选择文件后会自动按 key 去重并覆盖。右侧列表会刷新显示全部已导入 key。
            </p>

            <h3>二、引用输入支持</h3>
            <p>支持两种输入方式：</p>
            <ol>
              <li>key 列表模式：`10495806,10648348,10980318`</li>
              <li>
                段落模式：在正文中包含 `\\cite&#123;...&#125;`，程序只替换 cite 命令，不改动其他文字
              </li>
            </ol>

            <h3>三、示例</h3>
            <div className="manual-example">
              <p className="manual-example-title">输入（段落模式）</p>
              <p>
                近年来，物联网（Internet of Things,
                IoT）的应用场景与规模高速扩张，网络连接设备数量快速增长\cite&#123;8016573&#125;，智慧交通、智慧医疗与智能农业等应用场景不断普及\cite&#123;9221208,6425066&#125;。
              </p>
              <p className="manual-example-title">输出</p>
              <p>
                近年来，物联网（Internet of Things,
                IoT）的应用场景与规模高速扩张，网络连接设备数量快速增长[1]，智慧交通、智慧医疗与智能农业等应用场景不断普及[2],[3]。
              </p>
            </div>

            <h3>四、下一个序号设置</h3>
            <p>
              “设置下一个序号”用于控制后续新引用编号：
              如果当前最大编号是[10]，你手动插入了5条外部文献，可把下一个序号设置为16，则程序内后续新引用会从[16]开始。
            </p>
            <p>
              输入框留空时，点击“设置下一个”会自动采用“当前最大编号+1”。
            </p>

            <h3>五、清空按钮说明</h3>
            <ol>
              <li>清空数据库：删除全部文献并清空引用。</li>
              <li>清空引用：仅清空已引用列表，文献库保留。</li>
            </ol>
          </section>
        </div>
      ) : null}
    </main>
  );
}

export default App;
