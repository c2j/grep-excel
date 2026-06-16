export type SearchMode = "FullText" | "ExactMatch" | "Wildcard" | "Regex";

export interface SearchQuery {
  text: string;
  column: string | null;
  mode: SearchMode;
  limit: number;
  sheet: string | null;
  invert: boolean;
}

export interface SearchResult {
  sheet_name: string;
  file_name: string;
  row: string[];
  matched_columns: number[];
  row_index: number;
}

export interface SheetColumnMeta {
  col_names: string[];
  col_widths: number[];
}

export interface SearchStats {
  total_rows_searched: number;
  total_matches: number;
  matches_per_sheet: Record<string, number>;
  search_duration: { secs: number; nanos: number };
  truncated: boolean;
}

export interface SearchResponse {
  results: SearchResult[];
  stats: SearchStats;
  columns_by_sheet: Record<string, SheetColumnMeta>;
}

export interface FileSample {
  sheet_name: string;
  headers: string[];
  rows: string[][];
}

export interface FileInfo {
  name: string;
  sheets: [string, number][];
  total_rows: number;
  sample: FileSample | null;
}

export interface SheetMetadataInfo {
  sheet_name: string;
  row_count: number;
  columns: string[];
}

export interface FileMetadataInfo {
  file_name: string;
  sheet_count: number;
  sheets: SheetMetadataInfo[];
}

export interface SheetDataResult {
  file_name: string;
  sheet_name: string;
  columns: string[];
  rows: string[][];
  row_count: number;
  total_rows: number;
  truncated: boolean;
}

export interface SqlResult {
  columns: string[];
  rows: string[][];
  row_count: number;
  truncated: boolean;
  duration: { secs: number; nanos: number };
}

export interface TableAliasInfo {
  table_name: string;
  alias: string;
  file_name: string;
  sheet_name: string;
  row_count: number;
  columns: string[];
}
