package main

import (
	"encoding/json"
	"log"
	"os"
	"strings"
)

// StockConfig / StockData 保持不变，供 config.json 覆盖
type StockConfig struct {
	Code  string `json:"code"`
	Name  string `json:"name"`
	Group string `json:"group"`
}

type StockData struct {
	Code   string    `json:"code"`
	Name   string    `json:"name"`
	Group  string    `json:"group"`
	Price  float64   `json:"price"`
	Change float64   `json:"change"`
	Pct    float64   `json:"pct"`
	Prev   float64   `json:"prev"`
	Prices []float64 `json:"prices,omitempty"`
	SVG    string    `json:"svg,omitempty"`
}

var (
	defaultConfigs = []StockConfig{
		{Code: "sh000001", Name: "上证指数", Group: "A股"},
		{Code: "sh600519", Name: "贵州茅台", Group: "A股"},
		{Code: "sz399006", Name: "创业板指", Group: "A股"},
		{Code: "usAAPL", Name: "苹果", Group: "美股"},
		{Code: "usNVDA", Name: "英伟达", Group: "美股"},
		{Code: "usTSLA", Name: "特斯拉", Group: "美股"},
		{Code: "hf_GC", Name: "黄金", Group: "期货"},
		{Code: "hf_CL", Name: "原油", Group: "期货"},
		{Code: "usVIX", Name: "VIX", Group: "期货"},
	}
	yahooMap = map[string]string{
		"hf_GC": "GC=F",
		"hf_CL": "CL=F",
		"usVIX": "^VIX",
	}
)

func loadConfig() []StockConfig {
	paths := []string{"config.json", "/mnt/us/extensions/kkanpan/config.json", "/mnt/us/kkanpan/config.json"}
	for _, p := range paths {
		if data, err := os.ReadFile(p); err == nil {
			var cfg []StockConfig
			if err := json.Unmarshal(data, &cfg); err == nil && len(cfg) > 0 {
				log.Printf("Loaded config from %s (%d items)", p, len(cfg))
				return cfg
			}
		}
	}
	return defaultConfigs
}

func codeToYahoo(code string) string {
	if y, ok := yahooMap[code]; ok {
		return y
	}
	if strings.HasPrefix(code, "us") {
		return code[2:]
	}
	if strings.HasPrefix(code, "sh") {
		return code[2:] + ".SS"
	}
	if strings.HasPrefix(code, "sz") {
		return code[2:] + ".SZ"
	}
	return code
}

func tradingMinutes(code string) int {
	if strings.HasPrefix(code, "sh") || strings.HasPrefix(code, "sz") {
		return 240 // 09:30-11:30 +13:00-15:00
	}
	if strings.HasPrefix(code, "us") {
		return 390 // 09:30-16:00 ET
	}
	if strings.HasPrefix(code, "hf_") {
		return 1440 // 0:00-24:00
	}
	return 0
}
