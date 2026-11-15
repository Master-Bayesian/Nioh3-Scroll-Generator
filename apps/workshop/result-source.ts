/** Semantic result ownership is independent of the localized display label. */
export type ResultOrigin = "none" | "search" | "native_search" | "known_id" |
  "demo_search" | "demo_known_id" | "sample";

export function resultSourceLabel(origin: ResultOrigin): string {
  switch (origin) {
    case "none": return "";
    case "search": return "真实搜索结果";
    case "native_search": return "原生搜索结果";
    case "known_id": return "种子预览";
    case "demo_search": return "已按条件筛选";
    case "demo_known_id": return "已知种子预览";
    case "sample": return "未筛选示例";
  }
}
