import { useTranslation } from "react-i18next";
import type { SearchResult, SearchStats } from "../types/search";

interface Props {
  results: SearchResult[];
  stats: SearchStats | null;
}

export function ResultsTable({ results, stats }: Props) {
  const { t } = useTranslation();

  if (results.length === 0) {
    return (
      <div className="text-center py-12 text-gray-400">
        {t("search.noResults")}
      </div>
    );
  }

  const colNames = results[0]?.col_names ?? [];
  const durationMs = stats ? Math.round(stats.search_duration.secs * 1000 + stats.search_duration.nanos / 1e6) : 0;

  return (
    <div className="space-y-3">
      {stats && (
        <div className="flex items-center gap-4 text-sm text-gray-600">
          <span className="font-medium text-primary-700">
            {t("search.results", { count: stats.total_matches })}
          </span>
          <span>{t("search.rowsSearched", { count: stats.total_rows_searched })}</span>
          <span>{t("search.duration", { ms: durationMs })}</span>
        </div>
      )}

      <div className="overflow-x-auto border border-gray-200 rounded-lg">
        <table className="min-w-full text-sm">
          <thead>
            <tr className="bg-gray-50 border-b border-gray-200">
              <th className="px-3 py-2 text-left font-medium text-gray-600 whitespace-nowrap">
                {t("files.title").replace("已导入", "").replace("Imported ", "")}
              </th>
              <th className="px-3 py-2 text-left font-medium text-gray-600 whitespace-nowrap">
                {t("search.sheet")}
              </th>
              {colNames.map((col, i) => (
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
            {results.map((result, idx) => (
              <tr key={idx} className="hover:bg-gray-50">
                <td className="px-3 py-2 text-gray-700 whitespace-nowrap text-xs">
                  {result.file_name}
                </td>
                <td className="px-3 py-2 text-gray-700 whitespace-nowrap text-xs">
                  {result.sheet_name}
                </td>
                {result.col_names.map((_, colIdx) => {
                  const value = result.row[colIdx] ?? "";
                  const isMatched = result.matched_columns.includes(colIdx);
                  return (
                    <td
                      key={colIdx}
                      className={`px-3 py-2 whitespace-nowrap ${
                        isMatched
                          ? "bg-yellow-100 text-gray-900 font-medium"
                          : "text-gray-700"
                      }`}
                    >
                      {value}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
