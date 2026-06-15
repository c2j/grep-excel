import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import "./i18n";
import {
  importFileDialog,
  search as searchApi,
  executeSql,
  listFiles,
  getMetadata,
  clearData,
} from "./api/commands";
import type {
  FileInfo,
  FileMetadataInfo,
  SearchResult,
  SearchStats,
  SearchMode,
  SqlResult,
} from "./types/search";
import { FileImporter, LanguageToggle } from "./components/FileImporter";
import { FileList } from "./components/FileList";
import { SearchBar } from "./components/SearchBar";
import { ResultsTable } from "./components/ResultsTable";
import { SqlEditor } from "./components/SqlEditor";

type Tab = "search" | "sql";

function App() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("search");
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [metadata, setMetadata] = useState<FileMetadataInfo | null>(null);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [stats, setStats] = useState<SearchStats | null>(null);
  const [importing, setImporting] = useState(false);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshFiles = useCallback(async () => {
    try {
      const list = await listFiles();
      setFiles(list);
      if (list.length > 0) {
        const meta = await getMetadata(list[0].name);
        setMetadata(meta);
      } else {
        setMetadata(null);
      }
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const handleImport = useCallback(async () => {
    setImporting(true);
    setError(null);
    try {
      const info = await importFileDialog();
      if (info) {
        await refreshFiles();
        setResults([]);
        setStats(null);
      }
    } catch (e) {
      setError(t("import.failed", { error: String(e) }));
    } finally {
      setImporting(false);
    }
  }, [refreshFiles, t]);

  const handleSearch = useCallback(
    async (
      text: string,
      mode: SearchMode,
      column: string | null,
      sheet: string | null,
      invert: boolean,
    ) => {
      setSearching(true);
      setError(null);
      try {
        const resp = await searchApi({
          text,
          mode,
          column,
          sheet,
          invert,
          limit: 1000,
        });
        setResults(resp.results);
        setStats(resp.stats);
      } catch (e) {
        setError(String(e));
        setResults([]);
        setStats(null);
      } finally {
        setSearching(false);
      }
    },
    [],
  );

  const handleSql = useCallback(async (sql: string): Promise<SqlResult | null> => {
    return executeSql(sql);
  }, []);

  const handleClear = useCallback(async () => {
    try {
      await clearData();
      setFiles([]);
      setMetadata(null);
      setResults([]);
      setStats(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const columns = metadata?.sheets.flatMap((s) => s.columns) ?? [];
  const uniqueColumns = [...new Set(columns)];
  const sheets = metadata?.sheets ?? [];

  return (
    <div className="h-screen flex flex-col bg-gray-100">
      <header className="flex items-center justify-between px-6 py-3 bg-white border-b border-gray-200">
        <h1 className="text-lg font-bold text-gray-800">{t("app.title")}</h1>
        <LanguageToggle />
      </header>

      {error && (
        <div className="mx-6 mt-3 p-3 bg-red-50 border border-red-200 rounded-md text-sm text-red-700 flex items-center justify-between">
          <span>{error}</span>
          <button
            onClick={() => setError(null)}
            className="text-red-400 hover:text-red-600 ml-2"
          >
            ×
          </button>
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        <aside className="w-64 p-4 bg-white border-r border-gray-200 overflow-y-auto">
          <FileImporter onImport={handleImport} loading={importing} />
          <div className="mt-4">
            <FileList files={files} onClear={handleClear} />
          </div>
        </aside>

        <main className="flex-1 flex flex-col overflow-hidden">
          <div className="flex border-b border-gray-200 bg-white">
            {(["search", "sql"] as Tab[]).map((tb) => (
              <button
                key={tb}
                onClick={() => setTab(tb)}
                className={`px-6 py-2.5 text-sm font-medium border-b-2 transition-colors ${
                  tab === tb
                    ? "border-primary-600 text-primary-700"
                    : "border-transparent text-gray-500 hover:text-gray-700"
                }`}
              >
                {t(`tabs.${tb}`)}
              </button>
            ))}
          </div>

          <div className="flex-1 overflow-y-auto p-6">
            {tab === "search" && (
              <div className="space-y-4">
                <SearchBar
                  columns={uniqueColumns}
                  sheets={sheets}
                  onSearch={handleSearch}
                  loading={searching}
                />
                <ResultsTable results={results} stats={stats} />
              </div>
            )}
            {tab === "sql" && <SqlEditor onExecute={handleSql} />}
          </div>
        </main>
      </div>
    </div>
  );
}

export default App;
