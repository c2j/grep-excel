import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { SearchMode, SheetMetadataInfo } from "../types/search";

interface Props {
  columns: string[];
  sheets: SheetMetadataInfo[];
  onSearch: (
    text: string,
    mode: SearchMode,
    column: string | null,
    sheet: string | null,
    invert: boolean,
  ) => void;
  loading: boolean;
}

const MODES: { value: SearchMode; key: string }[] = [
  { value: "FullText", key: "fulltext" },
  { value: "ExactMatch", key: "exact" },
  { value: "Wildcard", key: "wildcard" },
  { value: "Regex", key: "regex" },
];

export function SearchBar({ columns, sheets, onSearch, loading }: Props) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [mode, setMode] = useState<SearchMode>("FullText");
  const [column, setColumn] = useState<string>("");
  const [sheet, setSheet] = useState<string>("");
  const [invert, setInvert] = useState(false);
  const [isComposing, setIsComposing] = useState(false);

  const handleSearch = () => {
    if (!text.trim()) return;
    onSearch(
      text,
      mode,
      column || null,
      sheet || null,
      invert,
    );
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !isComposing) handleSearch();
  };

  return (
    <div className="space-y-3">
      <div className="flex gap-2">
        <input
          type="text"
          lang="zh"
          value={text}
          onChange={(e) => {
            if (!isComposing) setText(e.target.value);
          }}
          onCompositionStart={() => setIsComposing(true)}
          onCompositionEnd={(e) => {
            setIsComposing(false);
            setText((e.target as HTMLInputElement).value);
          }}
          onKeyDown={handleKeyDown}
          placeholder={t("search.placeholder")}
          className="flex-1 px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none"
        />
        <button
          onClick={handleSearch}
          disabled={loading || !text.trim()}
          className="px-6 py-2 bg-primary-600 hover:bg-primary-700 disabled:bg-gray-400 text-white font-medium rounded-lg transition-colors whitespace-nowrap"
        >
          {loading ? "..." : t("search.button")}
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div className="flex gap-1 bg-gray-100 p-1 rounded-lg">
          {MODES.map((m) => (
            <button
              key={m.value}
              onClick={() => setMode(m.value)}
              className={`px-3 py-1 text-sm font-medium rounded-md transition-colors ${
                mode === m.value
                  ? "bg-white text-primary-700 shadow-sm"
                  : "text-gray-600 hover:text-gray-800"
              }`}
            >
              {t(`search.mode.${m.key}`)}
            </button>
          ))}
        </div>

        {columns.length > 0 && (
          <select
            value={column}
            onChange={(e) => setColumn(e.target.value)}
            className="px-3 py-1.5 text-sm border border-gray-300 rounded-md bg-white outline-none focus:ring-2 focus:ring-primary-500"
          >
            <option value="">{t("search.columnAll")}</option>
            {columns.map((col) => (
              <option key={col} value={col}>
                {col}
              </option>
            ))}
          </select>
        )}

        {sheets.length > 0 && (
          <select
            value={sheet}
            onChange={(e) => setSheet(e.target.value)}
            className="px-3 py-1.5 text-sm border border-gray-300 rounded-md bg-white outline-none focus:ring-2 focus:ring-primary-500"
          >
            <option value="">{t("search.sheetAll")}</option>
            {sheets.map((s) => (
              <option key={s.sheet_name} value={s.sheet_name}>
                {s.sheet_name}
              </option>
            ))}
          </select>
        )}

        <label className="flex items-center gap-1.5 text-sm text-gray-600 cursor-pointer">
          <input
            type="checkbox"
            checked={invert}
            onChange={(e) => setInvert(e.target.checked)}
            className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
          />
          {t("search.invert")}
        </label>
      </div>
    </div>
  );
}
