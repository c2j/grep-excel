import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SearchResult, SearchStats, SheetColumnMeta } from "../types/search";

const ROW_HEIGHT = 36;
const OVERSCAN = 5;
const VIEWPORT_HEIGHT = 600;

interface Props {
  results: SearchResult[];
  stats: SearchStats | null;
  columnsBySheet: Record<string, SheetColumnMeta>;
}

export function ResultsTable({ results, stats, columnsBySheet }: Props) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(VIEWPORT_HEIGHT);

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
    setScrollTop(0);
  }, [results]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const update = () => setViewportH(el.clientHeight);
    update();
    const obs = new ResizeObserver(update);
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  if (results.length === 0) {
    return (
      <div className="text-center py-12 text-gray-400">
        {t("search.noResults")}
      </div>
    );
  }

  const colNames = columnsBySheet[results[0].sheet_name]?.col_names ?? [];
  const colCount = colNames.length + 2;
  const durationMs = stats ? Math.round(stats.search_duration.secs * 1000 + stats.search_duration.nanos / 1e6) : 0;

  const startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const endIndex = Math.min(results.length, Math.ceil((scrollTop + viewportH) / ROW_HEIGHT) + OVERSCAN);
  const visibleResults = results.slice(startIndex, endIndex);
  const topSpacer = startIndex * ROW_HEIGHT;
  const bottomSpacer = Math.max(0, (results.length - endIndex) * ROW_HEIGHT);

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

      <div
        ref={scrollRef}
        onScroll={() => setScrollTop(scrollRef.current?.scrollTop ?? 0)}
        className="overflow-auto border border-gray-200 rounded-lg"
        style={{ maxHeight: `${VIEWPORT_HEIGHT}px` }}
      >
        <table className="min-w-full text-sm">
          <thead>
            <tr className="sticky top-0 z-10 bg-gray-50 border-b border-gray-200">
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
            {topSpacer > 0 && (
              <tr aria-hidden="true" style={{ height: `${topSpacer}px` }}>
                <td colSpan={colCount} />
              </tr>
            )}
            {visibleResults.map((result, idx) => {
              const resultCols = columnsBySheet[result.sheet_name]?.col_names ?? colNames;
              return (
                <tr key={startIndex + idx} className="hover:bg-gray-50">
                  <td className="px-3 py-2 text-gray-700 whitespace-nowrap text-xs">
                    {result.file_name}
                  </td>
                  <td className="px-3 py-2 text-gray-700 whitespace-nowrap text-xs">
                    {result.sheet_name}
                  </td>
                  {resultCols.map((_, colIdx) => {
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
              );
            })}
            {bottomSpacer > 0 && (
              <tr aria-hidden="true" style={{ height: `${bottomSpacer}px` }}>
                <td colSpan={colCount} />
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
