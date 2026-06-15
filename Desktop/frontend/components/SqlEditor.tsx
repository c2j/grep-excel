import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { SqlResult } from "../types/search";

interface Props {
  onExecute: (sql: string) => Promise<SqlResult | null>;
}

export function SqlEditor({ onExecute }: Props) {
  const { t } = useTranslation();
  const [sql, setSql] = useState("");
  const [result, setResult] = useState<SqlResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleExecute = async () => {
    if (!sql.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await onExecute(sql);
      setResult(res);
    } catch (e) {
      setError(String(e));
      setResult(null);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      handleExecute();
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-700">{t("sql.title")}</h3>
        <button
          onClick={handleExecute}
          disabled={loading || !sql.trim()}
          className="px-4 py-1.5 bg-primary-600 hover:bg-primary-700 disabled:bg-gray-400 text-white text-sm font-medium rounded-md transition-colors"
        >
          {loading ? "..." : t("sql.execute")}
        </button>
      </div>

      <textarea
        value={sql}
        onChange={(e) => setSql(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={t("sql.placeholder")}
        rows={4}
        className="w-full px-4 py-2 font-mono text-sm border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none resize-y"
      />

      {error && (
        <div className="p-3 bg-red-50 border border-red-200 rounded-md text-sm text-red-700">
          {t("sql.error", { error })}
        </div>
      )}

      {result && (
        <div className="overflow-x-auto border border-gray-200 rounded-lg">
          {result.rows.length === 0 ? (
            <div className="text-center py-8 text-gray-400 text-sm">
              {t("sql.noResults")}
            </div>
          ) : (
            <table className="min-w-full text-sm">
              <thead>
                <tr className="bg-gray-50 border-b border-gray-200">
                  {result.columns.map((col, i) => (
                    <th
                      key={i}
                      className="px-3 py-2 text-left font-medium text-gray-600 whitespace-nowrap"
                    >
                      {col}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {result.rows.map((row, rowIdx) => (
                  <tr key={rowIdx} className="hover:bg-gray-50">
                    {row.map((cell, colIdx) => (
                      <td
                        key={colIdx}
                        className="px-3 py-2 text-gray-700 whitespace-nowrap"
                      >
                        {cell}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  );
}
