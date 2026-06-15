import { useTranslation } from "react-i18next";

interface Props {
  onImport: () => void;
  loading: boolean;
}

export function LanguageToggle() {
  const { i18n } = useTranslation();

  const toggle = () => {
    const next = i18n.language === "zh" ? "en" : "zh";
    i18n.changeLanguage(next);
    localStorage.setItem("lang", next);
  };

  return (
    <button
      onClick={toggle}
      className="px-3 py-1.5 text-sm font-medium text-gray-600 hover:text-primary-600 hover:bg-primary-50 rounded-md transition-colors"
    >
      {i18n.language === "zh" ? "EN" : "中文"}
    </button>
  );
}

export function FileImporter({ onImport, loading }: Props) {
  const { t } = useTranslation();

  return (
    <button
      onClick={onImport}
      disabled={loading}
      className="w-full px-4 py-3 bg-primary-600 hover:bg-primary-700 disabled:bg-gray-400 text-white font-medium rounded-lg transition-colors flex items-center justify-center gap-2"
    >
      {loading ? (
        <span className="animate-pulse">...</span>
      ) : (
        <>
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
            />
          </svg>
          {t("import.button")}
        </>
      )}
    </button>
  );
}
