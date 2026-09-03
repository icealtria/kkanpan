package main

import "testing"

func TestTradingMinutes(t *testing.T) {
	tests := []struct {
		code string
		want int
	}{
		{"sh600519", 240},
		{"sz000001", 240},
		{"usAAPL", 390},
		{"hk00700", 330},
		{"unknown", 0},
		{"", 0},
	}
	for _, tt := range tests {
		if got := tradingMinutes(tt.code); got != tt.want {
			t.Errorf("tradingMinutes(%q) = %d, want %d", tt.code, got, tt.want)
		}
	}
}

func TestChartTotal(t *testing.T) {
	tests := []struct {
		code string
		n    int
		want int
	}{
		{"sh600519", 100, 240}, // tradingMinutes > n, return tradingMinutes
		{"sh600519", 300, 300}, // tradingMinutes < n, return n
		{"usAAPL", 200, 390},   // tradingMinutes > n
		{"usAAPL", 500, 500},   // tradingMinutes < n
		{"unknown", 100, 100},  // no prefix match, return n
	}
	for _, tt := range tests {
		if got := chartTotal(tt.code, tt.n); got != tt.want {
			t.Errorf("chartTotal(%q, %d) = %d, want %d", tt.code, tt.n, got, tt.want)
		}
	}
}
