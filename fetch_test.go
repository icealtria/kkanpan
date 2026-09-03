package main

import (
	"testing"
)

func TestParseQT(t *testing.T) {
	// Normal stock (non-hf): vals[3]=price, vals[4]=prev, vals[31]=change, vals[32]=pct
	vals := make([]string, 33)
	vals[3] = "12.50"  // price
	vals[4] = "12.20"  // prev
	vals[31] = "0.30"  // change
	vals[32] = "2.46"  // pct
	price, change, pct, prev := parseQT("sh600519", vals)
	if price != 12.50 {
		t.Errorf("price = %f, want 12.50", price)
	}
	if prev != 12.20 {
		t.Errorf("prev = %f, want 12.20", prev)
	}
	if change != 0.30 {
		t.Errorf("change = %f, want 0.30", change)
	}
	if pct != 2.46 {
		t.Errorf("pct = %f, want 2.46", pct)
	}

	// hf_ prefix (fund/forex)
	hfVals := []string{"100.50", "2.50"}
	price, change, pct, prev = parseQT("hf_GC", hfVals)
	if price != 100.50 {
		t.Errorf("hf price = %f, want 100.50", price)
	}
	if change != 2.50 {
		t.Errorf("hf change = %f, want 2.50", change)
	}
	if prev != 98.00 {
		t.Errorf("hf prev = %f, want 98.00", prev)
	}

	// Empty vals
	price, change, pct, prev = parseQT("sh600519", nil)
	if price != 0 || change != 0 || pct != 0 || prev != 0 {
		t.Error("expected all zeros for nil vals")
	}
}

func TestParseGtimgRows(t *testing.T) {
	// Normal rows: format is "time price" where parts[1] is the float
	rows := []string{
		"09:30 12.50",
		"09:31 12.60",
		"09:32 12.55",
	}
	prices := parseGtimgRows(rows)
	if len(prices) != 3 {
		t.Fatalf("expected 3 prices, got %d", len(prices))
	}
	if prices[0] != 12.50 || prices[1] != 12.60 || prices[2] != 12.55 {
		t.Errorf("unexpected prices: %v", prices)
	}

	// Single row -> nil
	single := []string{"2026-09-01 09:30 12.50"}
	prices = parseGtimgRows(single)
	if prices != nil {
		t.Error("expected nil for single row")
	}

	// Empty
	prices = parseGtimgRows(nil)
	if prices != nil {
		t.Error("expected nil for nil rows")
	}

	// Invalid row format
	bad := []string{"invalid", "also invalid"}
	prices = parseGtimgRows(bad)
	if prices != nil {
		t.Error("expected nil for invalid rows")
	}
}
