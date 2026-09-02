package main

import (
	"encoding/json"
	"log"
	"os"
)

type StockConfig struct {
	Code   string `json:"code"`
	Name   string `json:"name"`
	Group  string `json:"group"`
	Source string `json:"source"` // "tencent" | "yahoo"
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

type AutoRule struct {
	Group    string `json:"group"`
	Weekdays []int  `json:"weekdays"` // 0=Sun 1=Mon ... 6=Sat, 空表示每天
	Start    string `json:"start"`    // "09:00"
	End      string `json:"end"`      // "15:30"
}

type AppConfig struct {
	Proxy          string         `json:"proxy"`          // "http://127.0.0.1:7890" 可空
	CacheTTL       int64          `json:"cacheTTL"`       // 秒
	PinnedGroups   []string       `json:"pinnedGroups"`   // AUTO 时常驻的组
	AutoRules      []AutoRule     `json:"autoRules"`      // 按顺序匹配，09:00-15:30 格式
	TradingMinutes map[string]int `json:"tradingMinutes"` // code/分组 -> 分钟数
}

var appConfig AppConfig
var stocksCache []StockConfig

func loadAppConfig() AppConfig {
	paths := []string{"app.json", "/mnt/us/extensions/kkanpan/app.json", "/mnt/us/kkanpan/app.json"}
	for _, p := range paths {
		if data, err := os.ReadFile(p); err == nil {
			var cfg AppConfig
			if err := json.Unmarshal(data, &cfg); err == nil {
				if cfg.CacheTTL == 0 {
					cfg.CacheTTL = 55
				}
				if cfg.TradingMinutes == nil {
					cfg.TradingMinutes = map[string]int{}
				}
				log.Printf("Loaded app config from %s (proxy=%q)", p, cfg.Proxy)
				return cfg
			}
		}
	}
	log.Fatal("app.json not found or invalid")
	return AppConfig{}
}

func loadStocks() []StockConfig {
	if stocksCache != nil {
		return stocksCache
	}
	paths := []string{"stocks.json", "/mnt/us/extensions/kkanpan/stocks.json", "/mnt/us/kkanpan/stocks.json"}
	for _, p := range paths {
		if data, err := os.ReadFile(p); err == nil {
			var cfg []StockConfig
			if err := json.Unmarshal(data, &cfg); err == nil && len(cfg) > 0 {
				log.Printf("Loaded stocks from %s (%d items)", p, len(cfg))
				stocksCache = cfg
				return cfg
			}
		}
	}
	log.Fatal("stocks.json not found or empty")
	return nil
}

func tradingMinutes(code string) int {
	if v, ok := appConfig.TradingMinutes[code]; ok {
		return v
	}
	for _, s := range loadStocks() {
		if s.Code == code {
			if v, ok := appConfig.TradingMinutes[s.Group]; ok {
				return v
			}
			break
		}
	}
	return 0
}
