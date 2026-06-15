import { useTranslation } from "react-i18next";
import type { FileInfo } from "../types/search";

interface Props {
  files: FileInfo[];
  onClear: () => void;
}

export function FileList({ files, onClear }: Props) {
  const { t } = useTranslation();

  if (files.length === 0) {
    return (
      <div className="text-center py-8 text-gray-400 text-sm">
        {t("files.empty")}
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-700">{t("files.title")}</h3>
        <button
          onClick={onClear}
          className="text-xs text-gray-500 hover:text-red-500 transition-colors"
        >
          {t("files.clear")}
        </button>
      </div>
      <div className="space-y-1.5">
        {files.map((file) => (
          <div
            key={file.name}
            className="p-2.5 bg-gray-50 rounded-md border border-gray-200"
          >
            <div className="font-medium text-sm text-gray-800 truncate">
              {file.name}
            </div>
            <div className="flex gap-3 mt-1 text-xs text-gray-500">
              <span>{t("files.sheets", { count: file.sheets.length })}</span>
              <span>{t("files.rows", { count: file.total_rows })}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
