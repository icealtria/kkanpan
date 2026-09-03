package main

import (
	"image"
	"testing"
	"time"
)

func BenchmarkParseQT(b *testing.B) {
	vals := make([]string, 33)
	vals[3] = "1800.00"
	vals[4] = "1790.00"
	vals[31] = "10.00"
	vals[32] = "0.56"
	for i := 0; i < b.N; i++ {
		parseQT("sh600519", vals)
	}
}

func BenchmarkParseQT_Hf(b *testing.B) {
	vals := []string{"2000.50", "10.00"}
	for i := 0; i < b.N; i++ {
		parseQT("hf_GC", vals)
	}
}

func BenchmarkParseGtimgRows(b *testing.B) {
	rows := make([]string, 240)
	for i := range rows {
		rows[i] = "09:30 12.50"
	}
	for i := 0; i < b.N; i++ {
		parseGtimgRows(rows)
	}
}

func BenchmarkParseHM(b *testing.B) {
	for i := 0; i < b.N; i++ {
		parseHM("09:30")
	}
}

func BenchmarkSparklinePoints(b *testing.B) {
	prices := make([]float64, 240)
	for i := range prices {
		prices[i] = 1800.0 + float64(i)*0.5
	}
	for i := 0; i < b.N; i++ {
		sparklinePoints(prices, "sh600519", 480, 70)
	}
}

func BenchmarkStockStrings(b *testing.B) {
	for i := 0; i < b.N; i++ {
		stockStrings(1800.50, 10.30, 0.57, false)
	}
}

func BenchmarkStockStrings_SVG(b *testing.B) {
	for i := 0; i < b.N; i++ {
		stockStrings(1800.50, 10.30, 0.57, true)
	}
}

func BenchmarkMatchRule(b *testing.B) {
	rule := AutoRule{Weekdays: []int{1, 2, 3, 4, 5}, Start: "09:30", End: "15:00"}
	loc := time.FixedZone("CST", 8*3600)
	now := time.Date(2026, 9, 1, 10, 30, 0, 0, loc)
	for i := 0; i < b.N; i++ {
		matchRule(now, rule)
	}
}

func BenchmarkDrawSparklineGraph(b *testing.B) {
	img := image.NewGray(image.Rect(0, 0, 1072, 1448))
	item := StockData{
		Code:  "sh600519",
		Price: 1800.50, Prev: 1790.00,
		Prices: func() []float64 {
			p := make([]float64, 240)
			for i := range p {
				p[i] = 1800.0 + float64(i)*0.2
			}
			return p
		}(),
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		drawSparklineGraph(img, item, 240, 12, 480, 70)
	}
}
