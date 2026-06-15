import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import type {
  SearchQuery,
  SearchResponse,
  FileInfo,
  FileMetadataInfo,
  SheetDataResult,
  SqlResult,
  TableAliasInfo,
} from "../types/search";

export async function importFileDialog(): Promise<FileInfo | null> {
  const selected = await open({
    multiple: false,
    filters: [
      { name: "Spreadsheets", extensions: ["xlsx", "xls", "xlsm", "xlsb", "ods", "csv"] },
    ],
  });
  if (!selected || typeof selected !== "string") {
    return null;
  }
  return invoke<FileInfo>("import_file", { path: selected });
}

export async function importFile(path: string): Promise<FileInfo> {
  return invoke<FileInfo>("import_file", { path });
}

export async function search(query: SearchQuery): Promise<SearchResponse> {
  return invoke<SearchResponse>("search", { query });
}

export async function executeSql(sql: string, limit?: number): Promise<SqlResult> {
  return invoke<SqlResult>("execute_sql", { sql, limit });
}

export async function listFiles(): Promise<FileInfo[]> {
  return invoke<FileInfo[]>("list_files");
}

export async function listTableAliases(): Promise<TableAliasInfo[]> {
  return invoke<TableAliasInfo[]>("list_table_aliases");
}

export async function getMetadata(fileName: string): Promise<FileMetadataInfo> {
  return invoke<FileMetadataInfo>("get_metadata", { fileName });
}

export async function getSheetSample(
  fileName: string,
  sheetName: string,
  sampleSize?: number,
): Promise<SheetDataResult> {
  return invoke<SheetDataResult>("get_sheet_sample", {
    fileName,
    sheetName,
    sampleSize,
  });
}

export async function getSheetData(
  fileName: string,
  sheetName: string,
  startRow?: number,
  endRow?: number,
  columns?: string[],
): Promise<SheetDataResult> {
  return invoke<SheetDataResult>("get_sheet_data", {
    fileName,
    sheetName,
    startRow,
    endRow,
    columns,
  });
}

export async function updateCell(
  fileName: string,
  sheetName: string,
  row: number,
  column: string,
  value: string,
): Promise<void> {
  await invoke("update_cell", { fileName, sheetName, row, column, value });
}

export async function clearData(): Promise<void> {
  await invoke("clear_data");
}
