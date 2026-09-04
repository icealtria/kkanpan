package main

import (
	"strings"
	"testing"
)

func TestSparklinePoints(t *testing.T) {
	// Less than 2 prices
	pts, minVal, maxVal, rng := sparklinePoints([]float64{1.0}, 100, 50)
	if pts != nil {
		t.Error("expected nil for single price")
	}
	if rng != 1 {
		t.Errorf("expected rng=1 for single price, got %f", rng)
	}

	// Normal case
	pts, minVal, maxVal, rng = sparklinePoints([]float64{10, 20, 30, 40, 50}, 100, 50)
	if len(pts) != 5 {
		t.Fatalf("expected 5 points, got %d", len(pts))
	}
	if minVal != 10 || maxVal != 50 {
		t.Errorf("min/max = %f/%f, want 10/50", minVal, maxVal)
	}
	if rng != 40 {
		t.Errorf("rng = %f, want 40", rng)
	}

	// All same prices
	pts, _, _, rng = sparklinePoints([]float64{5, 5, 5}, 100, 50)
	if len(pts) != 3 {
		t.Fatalf("expected 3 points, got %d", len(pts))
	}
	if rng != 1 {
		t.Errorf("rng should be 1 for constant prices, got %f", rng)
	}

	// Two prices
	pts, minVal, maxVal, _ = sparklinePoints([]float64{100, 200}, 200, 80)
	if len(pts) != 2 {
		t.Fatalf("expected 2 points, got %d", len(pts))
	}
	if minVal != 100 || maxVal != 200 {
		t.Errorf("min/max = %f/%f, want 100/200", minVal, maxVal)
	}
}

func TestStockStrings(t *testing.T) {
	tests := []struct {
		price, change, pct float64
		wantPrice          string
		wantChgContains    string
	}{
		{12.50, 0.30, 2.46, "12.50", "▲ +0.30"},
		{12.50, -0.30, -2.46, "12.50", "▼ -0.30"},
		{12.50, 0, 0, "12.50", "  +0.00"},
		{0, 0, 0, "--", "  +0.00"},
	}
	for _, tt := range tests {
		priceStr, chgStr := stockStrings(tt.price, tt.change, tt.pct)
		if priceStr != tt.wantPrice {
			t.Errorf("stockStrings(%.2f, %.2f, %.2f) price = %q, want %q",
				tt.price, tt.change, tt.pct, priceStr, tt.wantPrice)
		}
		if !strings.Contains(chgStr, tt.wantChgContains) {
			t.Errorf("stockStrings(%.2f, %.2f, %.2f) chg = %q, want to contain %q",
				tt.price, tt.change, tt.pct, chgStr, tt.wantChgContains)
		}
	}
}
