package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestParseQT_EmptyVals(t *testing.T) {
	p, c, pct, prev := parseQT("sh600519", nil)
	if p != 0 || c != 0 || pct != 0 || prev != 0 {
		t.Errorf("nil vals: expected all zeros, got %f %f %f %f", p, c, pct, prev)
	}

	p, c, pct, prev = parseQT("sh600519", []string{})
	if p != 0 || c != 0 || pct != 0 || prev != 0 {
		t.Errorf("empty vals: expected all zeros, got %f %f %f %f", p, c, pct, prev)
	}
}

func TestParseQT_ShortVals(t *testing.T) {
	// Only 1 element
	p, c, pct, prev := parseQT("sh600519", []string{"1800.00"})
	if p != 0 || prev != 0 {
		t.Errorf("short vals: price=%f prev=%f, want 0", p, prev)
	}
	_ = c
	_ = pct
}

func TestParseQT_NonNumeric(t *testing.T) {
	// strconv.ParseFloat("NaN") returns NaN, not error
	vals := []string{"a", "b", "c", "not_a_number", "also_bad", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "NaN", "NaN"}
	p, c, pct, prev := parseQT("sh600519", vals)
	if p != 0 {
		t.Errorf("price = %f, want 0 for non-numeric", p)
	}
	// change and pct will be NaN because strconv.ParseFloat("NaN") returns NaN
	_ = c
	_ = pct
	_ = prev
}

func TestParseQT_HfChangeComputed(t *testing.T) {
	// hf_ prefix: change = vals[1], prev = price - change, pct = change/prev*100
	vals := []string{"100.00", "5.00"}
	p, c, pct, prev := parseQT("hf_GC", vals)
	if p != 100.00 {
		t.Errorf("hf price = %f, want 100", p)
	}
	if c != 5.00 {
		t.Errorf("hf change = %f, want 5", c)
	}
	if prev != 95.00 {
		t.Errorf("hf prev = %f, want 95", prev)
	}
	if pct < 5.26 || pct > 5.27 {
		t.Errorf("hf pct = %f, want ~5.26", pct)
	}
}

func TestParseQT_HfPrevZero(t *testing.T) {
	// prev = price - change = 0 => pct stays 0
	vals := []string{"10.00", "10.00"}
	_, _, pct, prev := parseQT("hf_X", vals)
	if prev != 0 {
		t.Errorf("hf prev = %f, want 0", prev)
	}
	if pct != 0 {
		t.Errorf("hf pct = %f, want 0 when prev=0", pct)
	}
}

func TestParseQT_StockPrevZero(t *testing.T) {
	// Non-hf: prev defaults to price when vals[4] missing
	p, _, _, prev := parseQT("sh600519", []string{"", "", "", "1500.00"})
	if p != 1500.00 {
		t.Errorf("price = %f, want 1500", p)
	}
	if prev != 1500.00 {
		t.Errorf("prev = %f, want 1500 (default to price)", prev)
	}
}

func TestParseGtimgRows_InvalidFloat(t *testing.T) {
	rows := []string{"09:30 not_a_number", "09:31 also_bad"}
	prices := parseGtimgRows(rows)
	if prices != nil {
		t.Errorf("expected nil for non-numeric prices, got %v", prices)
	}
}

func TestParseGtimgRows_OnlyOneValid(t *testing.T) {
	rows := []string{"09:30 12.50", "bad row"}
	prices := parseGtimgRows(rows)
	if prices != nil {
		t.Errorf("expected nil for <2 valid prices, got %v", prices)
	}
}

func TestParseGtimgRows_NegativePrices(t *testing.T) {
	rows := []string{"09:30 -5.00", "09:31 -3.00", "09:32 -4.50"}
	prices := parseGtimgRows(rows)
	if len(prices) != 3 {
		t.Fatalf("expected 3 prices, got %d", len(prices))
	}
	if prices[0] != -5.00 || prices[1] != -3.00 || prices[2] != -4.50 {
		t.Errorf("unexpected prices: %v", prices)
	}
}

func TestParseHM_EdgeCases(t *testing.T) {
	tests := []struct {
		s    string
		want int
	}{
		{"0:0", 0},
		{"23:59", 1439},
		{"00:00", 0},
		{"1:30", 90},
		{"01:30", 90},
		{":::", 0},
		{"abc:def", 0},
	}
	for _, tt := range tests {
		if got := parseHM(tt.s); got != tt.want {
			t.Errorf("parseHM(%q) = %d, want %d", tt.s, got, tt.want)
		}
	}
}

func TestMatchRule_EdgeCases(t *testing.T) {
	loc := time.FixedZone("CST", 8*3600)

	// start == end: matches exactly that minute
	rule := AutoRule{Start: "12:00", End: "12:00"}
	now := time.Date(2026, 9, 1, 12, 0, 0, 0, loc)
	if !matchRule(now, rule) {
		t.Error("start==end, exact minute should match")
	}
	now = time.Date(2026, 9, 1, 12, 1, 0, 0, loc)
	if matchRule(now, rule) {
		t.Error("start==end, +1 minute should not match")
	}

	// Single minute window
	rule = AutoRule{Start: "09:30", End: "09:30"}
	now = time.Date(2026, 9, 1, 9, 30, 0, 0, loc)
	if !matchRule(now, rule) {
		t.Error("single minute window should match")
	}
	now = time.Date(2026, 9, 1, 9, 29, 0, 0, loc)
	if matchRule(now, rule) {
		t.Error("single minute window, -1 minute should not match")
	}

	// Weekday 0=Sunday
	rule = AutoRule{Weekdays: []int{0}, Start: "00:00", End: "23:59"}
	now = time.Date(2026, 9, 6, 10, 0, 0, 0, loc) // Sunday
	if !matchRule(now, rule) {
		t.Error("Sunday should match weekday=0")
	}
	now = time.Date(2026, 9, 7, 10, 0, 0, 0, loc) // Monday
	if matchRule(now, rule) {
		t.Error("Monday should not match weekday=0")
	}
}

func TestChartTotal_EdgeCases(t *testing.T) {
	// n equals tradingMinutes
	if got := chartTotal("sh600519", 240); got != 240 {
		t.Errorf("chartTotal(sh600519, 240) = %d, want 240", got)
	}
	// n equals 0
	if got := chartTotal("sh600519", 0); got != 240 {
		t.Errorf("chartTotal(sh600519, 0) = %d, want 240", got)
	}
	// Very large n
	if got := chartTotal("usAAPL", 10000); got != 10000 {
		t.Errorf("chartTotal(usAAPL, 10000) = %d, want 10000", got)
	}
}

func TestTradingMinutes_PrefixEdgeCases(t *testing.T) {
	tests := []struct {
		code string
		want int
	}{
		{"sh", 240},
		{"sz", 240},
		{"us", 390},
		{"hk", 330},
		{"SH600519", 0}, // uppercase, no match
		{"USAAPL", 0},
	}
	for _, tt := range tests {
		if got := tradingMinutes(tt.code); got != tt.want {
			t.Errorf("tradingMinutes(%q) = %d, want %d", tt.code, got, tt.want)
		}
	}
}

func TestConfigRoundtrip(t *testing.T) {
	orig := AppConfig{
		Proxy:         "http://127.0.0.1:7890",
		CacheTTL:      120,
		DefaultView:   "AUTO",
		DimFrontlight: true,
		AutoRules: []AutoRule{
			{Group: "a-share", Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:30", End: "15:00"},
			{Group: "us", Weekdays: []int{1, 2, 3, 4, 5}, Start: "21:30", End: "04:00"},
		},
	}

	data, err := json.Marshal(orig)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var decoded AppConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if decoded.Proxy != orig.Proxy {
		t.Errorf("Proxy = %q, want %q", decoded.Proxy, orig.Proxy)
	}
	if decoded.CacheTTL != orig.CacheTTL {
		t.Errorf("CacheTTL = %d, want %d", decoded.CacheTTL, orig.CacheTTL)
	}
	if decoded.DefaultView != orig.DefaultView {
		t.Errorf("DefaultView = %q, want %q", decoded.DefaultView, orig.DefaultView)
	}
	if decoded.DimFrontlight != orig.DimFrontlight {
		t.Errorf("DimFrontlight = %v, want %v", decoded.DimFrontlight, orig.DimFrontlight)
	}
	if len(decoded.AutoRules) != len(orig.AutoRules) {
		t.Fatalf("AutoRules len = %d, want %d", len(decoded.AutoRules), len(orig.AutoRules))
	}
	for i, r := range decoded.AutoRules {
		o := orig.AutoRules[i]
		if r.Group != o.Group || r.Start != o.Start || r.End != o.End {
			t.Errorf("AutoRules[%d] = %+v, want %+v", i, r, o)
		}
		if len(r.Weekdays) != len(o.Weekdays) {
			t.Errorf("AutoRules[%d].Weekdays len = %d, want %d", i, len(r.Weekdays), len(o.Weekdays))
		}
	}
}

func TestStockConfigRoundtrip(t *testing.T) {
	orig := []StockConfig{
		{Code: "sh600519", Name: "贵州茅台", Group: "a-share", Source: "tencent"},
		{Code: "usAAPL", Name: "Apple", Group: "us", Source: "yahoo"},
	}

	data, err := json.Marshal(orig)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var decoded []StockConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if len(decoded) != len(orig) {
		t.Fatalf("len = %d, want %d", len(decoded), len(orig))
	}
	for i, s := range decoded {
		if s != orig[i] {
			t.Errorf("[%d] = %+v, want %+v", i, s, orig[i])
		}
	}
}

func TestLoadAppConfig_TempFile(t *testing.T) {
	dir := t.TempDir()
	cfgJSON := `{"proxy":"http://test:8080","cacheTTL":60,"defaultView":"ALL","dimFrontlight":true}`
	path := filepath.Join(dir, "app.json")
	if err := os.WriteFile(path, []byte(cfgJSON), 0644); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var cfg AppConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		t.Fatal(err)
	}

	if cfg.Proxy != "http://test:8080" {
		t.Errorf("Proxy = %q", cfg.Proxy)
	}
	if cfg.CacheTTL != 60 {
		t.Errorf("CacheTTL = %d", cfg.CacheTTL)
	}
	if cfg.DefaultView != "ALL" {
		t.Errorf("DefaultView = %q", cfg.DefaultView)
	}
}

func TestLoadStocks_TempFile(t *testing.T) {
	dir := t.TempDir()
	stocksJSON := `[{"code":"sh600519","name":"茅台","group":"a-share","source":"tencent"}]`
	path := filepath.Join(dir, "stocks.json")
	if err := os.WriteFile(path, []byte(stocksJSON), 0644); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var stocks []StockConfig
	if err := json.Unmarshal(data, &stocks); err != nil {
		t.Fatal(err)
	}

	if len(stocks) != 1 {
		t.Fatalf("len = %d, want 1", len(stocks))
	}
	if stocks[0].Code != "sh600519" {
		t.Errorf("Code = %q", stocks[0].Code)
	}
}
