// Stock data types matching the kkanpan Go backend

export interface StockConfig {
  code: string;
  name: string;
  group: string;
  source: "tencent" | "yahoo";
}

export interface StockData {
  code: string;
  name: string;
  group: string;
  price: number;
  change: number;
  pct: number;
  prev: number;
  prices: number[];
}

export interface AutoRule {
  group: string;
  weekdays: number[];
  start: string;
  end: string;
}

export interface AppConfig {
  proxy: string;
  cacheTTL: number;
  autoRules: AutoRule[];
  defaultView: string;
  dimFrontlight: boolean;
}

export type ViewMode = string; // "AUTO", "ALL", or a group name
export type StyleMode = "normal" | "large";
