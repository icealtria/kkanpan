import { createSignal, For, Show } from "solid-js";
import { View, Text } from "@pocketjs/framework/components";
import { MOCK_STOCKS } from "./mock-data";
import type { StockData, StyleMode } from "./types";

// ============================================================
// Sparkline: bar chart drawn with View elements (no SVG/canvas)
// ============================================================

function Sparkline(props: { prices: number[]; width: number; height: number }) {
  const bars = () => {
    const p = props.prices;
    if (p.length < 2) return [];
    let min = p[0],
      max = p[0];
    for (let i = 1; i < p.length; i++) {
      if (p[i] < min) min = p[i];
      if (p[i] > max) max = p[i];
    }
    const range = max - min || 1;
    const barW = Math.max(1, Math.floor(props.width / p.length));
    return p.map((v, i) => ({
      x: i * barW,
      h: Math.max(1, Math.floor(((v - min) / range) * (props.height - 4))),
      barW,
    }));
  };

  return (
    <View
      class="flex-row items-end"
      style={{ width: props.width, height: props.height }}
    >
      <For each={bars()}>
        {(bar) => (
          <View
            class="bg-slate-800"
            style={{ width: bar.barW, height: bar.h }}
          />
        )}
      </For>
    </View>
  );
}

// ============================================================
// StockCard
// ============================================================

function StockCard(props: { stock: StockData; style: StyleMode }) {
  const priceStr = () =>
    props.stock.price > 0 ? props.stock.price.toFixed(2) : "--";

  const chgStr = () => {
    const c = props.stock.change;
    const p = props.stock.pct;
    const arrow = c > 0 ? "▲" : c < 0 ? "▼" : " ";
    const sign = c >= 0 ? "+" : "";
    return `${arrow} ${sign}${c.toFixed(2)} (${sign}${p.toFixed(2)}%)`;
  };

  const isLarge = () => props.style === "large";

  return (
    <View class="flex-row justify-between items-center border border-slate-300 rounded px-3 py-2">
      {/* Left: name + code */}
      <View class="flex-col flex-1">
        <Text
          class={
            isLarge()
              ? "text-lg text-slate-950 font-bold"
              : "text-sm text-slate-950 font-bold"
          }
        >
          {props.stock.name}
        </Text>
        <Text class="text-xs text-slate-500">{props.stock.code}</Text>
      </View>

      {/* Center: sparkline */}
      <Show when={props.stock.prices.length >= 2}>
        <View class="mx-2">
          <Sparkline
            prices={props.stock.prices}
            width={isLarge() ? 140 : 100}
            height={isLarge() ? 40 : 28}
          />
        </View>
      </Show>

      {/* Right: price + change */}
      <View class="flex-col items-end">
        <Text
          class={
            isLarge()
              ? "text-lg text-slate-950 font-bold"
              : "text-base text-slate-950 font-bold"
          }
        >
          {priceStr()}
        </Text>
        <Text
          class={
            props.stock.change >= 0
              ? isLarge()
                ? "text-sm text-emerald-600 font-bold"
                : "text-xs text-emerald-600"
              : isLarge()
                ? "text-sm text-red-600 font-bold"
                : "text-xs text-red-600"
          }
        >
          {chgStr()}
        </Text>
      </View>
    </View>
  );
}

// ============================================================
// Header
// ============================================================

function Header(props: {
  viewMode: string;
  effectiveGroup: string;
  isAuto: boolean;
  style: StyleMode;
  onStyleToggle: () => void;
  onExit: () => void;
}) {
  const modeTag = () =>
    props.isAuto ? `[AUTO: ${props.effectiveGroup}]` : `[${props.viewMode}]`;

  return (
    <View class="flex-row justify-between items-center px-4 py-2 bg-white">
      <Text class="text-lg text-slate-950 font-bold">KKANPAN</Text>
      <View class="flex-row items-center gap-3">
        <Text class="text-xs text-slate-600">{modeTag()}</Text>
        <View
          class="px-3 py-1 rounded border border-slate-400"
          focusable
          onPress={props.onStyleToggle}
        >
          <Text class="text-xs text-slate-900 font-bold">
            {props.style === "large" ? "L" : "S"}
          </Text>
        </View>
        <View
          class="px-3 py-1 rounded border-2 border-slate-600"
          focusable
          onPress={props.onExit}
        >
          <Text class="text-sm text-slate-900 font-bold">X</Text>
        </View>
      </View>
    </View>
  );
}

// ============================================================
// TabBar
// ============================================================

function TabBar(props: {
  tabs: string[];
  active: string;
  onSelect: (mode: string) => void;
}) {
  return (
    <View class="flex-row gap-2 px-4 py-2">
      <For each={props.tabs}>
        {(tab) => (
          <View
            class={
              props.active === tab
                ? "flex-1 py-2 rounded bg-slate-900"
                : "flex-1 py-2 rounded border border-slate-400"
            }
            focusable
            onPress={() => props.onSelect(tab)}
          >
            <Text
              class={
                props.active === tab
                  ? "text-xs text-white font-bold text-center"
                  : "text-xs text-slate-900 text-center"
              }
            >
              {tab}
            </Text>
          </View>
        )}
      </For>
    </View>
  );
}

// ============================================================
// Footer
// ============================================================

function Footer(props: {
  page: number;
  totalPages: number;
  statusText: string;
}) {
  const indicator = () => {
    if (props.totalPages <= 1) return "";
    let s = `${props.page + 1} / ${props.totalPages}`;
    if (props.page > 0) s = "▲ " + s;
    if (props.page + 1 < props.totalPages) s = s + " ▼";
    return s;
  };

  return (
    <View class="flex-col px-4 py-2">
      <Show when={props.totalPages > 1}>
        <Text class="text-xs text-slate-500 text-center">{indicator()}</Text>
      </Show>
      <View class="h-px bg-slate-300 my-1" />
      <View class="flex-row justify-between">
        <Text class="text-xs text-slate-500">
          Swipe H: tab | Tap: flip
        </Text>
        <Text class="text-xs text-slate-700">{props.statusText}</Text>
      </View>
    </View>
  );
}

// ============================================================
// Main App
// ============================================================

export default function App() {
  const [viewMode, setViewMode] = createSignal<string>("ALL");
  const [style, setStyle] = createSignal<StyleMode>("normal");
  const [page, setPage] = createSignal(0);

  // All unique groups
  const groups = () => {
    const seen = new Set<string>();
    const out: string[] = [];
    for (const s of MOCK_STOCKS) {
      if (!seen.has(s.group)) {
        seen.add(s.group);
        out.push(s.group);
      }
    }
    return out;
  };

  const tabs = () => ["AUTO", ...groups(), "ALL"];

  // Filter stocks by current view mode
  const filteredStocks = (): StockData[] => {
    const mode = viewMode();
    if (mode === "ALL") return MOCK_STOCKS;
    if (mode === "AUTO") {
      // In auto mode, show all for now (real impl uses time-based rules)
      return MOCK_STOCKS;
    }
    return MOCK_STOCKS.filter((s) => s.group === mode);
  };

  // Pagination: 5 cards per page for normal, 3 for large
  const perPage = () => (style() === "large" ? 3 : 5);

  const totalPages = () => Math.max(1, Math.ceil(filteredStocks().length / perPage()));

  const pageStocks = (): StockData[] => {
    const all = filteredStocks();
    const start = page() * perPage();
    return all.slice(start, start + perPage());
  };

  const effectiveGroup = () => {
    const mode = viewMode();
    if (mode !== "AUTO" && mode !== "ALL") return mode;
    return "ALL";
  };

  const statusText = () => {
    const now = new Date();
    const h = String(now.getHours()).padStart(2, "0");
    const m = String(now.getMinutes()).padStart(2, "0");
    return `Updated: ${h}:${m}`;
  };

  const handleStyleToggle = () => {
    setStyle(style() === "large" ? "normal" : "large");
    setPage(0);
  };

  const handleExit = () => {
    // On Kindle, this would restore the home screen
    // In pocketjs, we could use a system op or just log
    console.log("Exit requested");
  };

  return (
    <View class="w-full h-full flex-col bg-white">
      {/* Header */}
      <Header
        viewMode={viewMode()}
        effectiveGroup={effectiveGroup()}
        isAuto={viewMode() === "AUTO"}
        style={style()}
        onStyleToggle={handleStyleToggle}
        onExit={handleExit}
      />

      {/* Tab bar */}
      <TabBar tabs={tabs()} active={viewMode()} onSelect={(m) => { setViewMode(m); setPage(0); }} />

      {/* Divider */}
      <View class="h-px bg-slate-900 mx-4" />

      {/* Stock list */}
      <View class="flex-col flex-1 px-4 py-2 gap-2">
        <For each={pageStocks()}>
          {(stock) => <StockCard stock={stock} style={style()} />}
        </For>
      </View>

      {/* Footer */}
      <Footer
        page={page()}
        totalPages={totalPages()}
        statusText={statusText()}
      />
    </View>
  );
}
